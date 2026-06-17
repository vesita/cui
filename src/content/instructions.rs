use anyhow::{Context, Result};
use globset::GlobBuilder;
use ignore::WalkBuilder;
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const INSTRUCTION_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    ".github/copilot-instructions.md",
    "CONTEXT.md",
];

fn fmt_instruction_source(source: &str, content: &str) -> String {
    let prefix_path =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../prompt/escdir/system/instruction_source_prefix.cui");
    crate::CuiFileComponent::from_file(prefix_path)
        .map(|c| {
            c.body().replace("{source}", source).replace("{content}", content)
        })
        .unwrap_or_default()
}

/// 从文件路径向上遍历查找指令文件。
pub fn resolve_nearby_instructions(
    workspace_root: &Path,
    config_dir: &Path,
    file_path: &Path,
) -> Result<Vec<(PathBuf, String)>> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    let target = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.to_path_buf());
    let root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());

    let system_paths = system_paths(workspace_root, config_dir, &[])?;
    let system_set: HashSet<_> = system_paths
        .iter()
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
        .collect();

    let mut current = target.parent().unwrap_or(&target);
    while current.starts_with(&root) {
        for file_name in INSTRUCTION_FILES {
            let candidate = current.join(file_name);
            let canonical = candidate
                .canonicalize()
                .unwrap_or_else(|_| candidate.clone());

            if canonical == target {
                continue;
            }
            if system_set.contains(&canonical) {
                continue;
            }
            if seen.contains(&canonical) {
                continue;
            }

            if candidate.exists()
                && let Ok(content) = fs::read_to_string(&candidate)
                && !content.trim().is_empty()
            {
                seen.insert(canonical.clone());
                results.push((
                    canonical.clone(),
                    fmt_instruction_source(&canonical.display().to_string(), &content),
                ));
            }
        }

        if current == root {
            break;
        }
        current = current.parent().unwrap_or(current);
    }

    Ok(results)
}

pub fn system_paths(
    workspace_root: &Path,
    config_dir: &Path,
    instructions: &[String],
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    let mut push_unique = |path: PathBuf| {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if seen.insert(canonical.clone()) {
            paths.push(canonical);
        }
    };

    if let Some(project_path) = find_project_instruction(workspace_root)? {
        push_unique(project_path);
    }

    let global_path = config_dir.join("AGENTS.md");
    if global_path.exists() {
        push_unique(global_path);
    }

    for raw in instructions {
        if raw.starts_with("http://") || raw.starts_with("https://") {
            continue;
        }

        let resolved = resolve_instruction_paths(workspace_root, raw)?;
        for path in resolved {
            push_unique(path);
        }
    }

    Ok(paths)
}

pub fn system_prompt_and_sources(
    workspace_root: &Path,
    config_dir: &Path,
    instructions: &[String],
) -> Result<(String, Vec<String>)> {
    let mut sections = Vec::new();
    let mut sources = Vec::new();
    let paths = system_paths(workspace_root, config_dir, instructions)?;

    for path in paths {
        if let Ok(content) = fs::read_to_string(&path)
            && !content.trim().is_empty()
        {
            sections.push(fmt_instruction_source(
                &path.display().to_string(),
                &content,
            ));
            sources.push(path.display().to_string());
        }
    }

    for url in instructions
        .iter()
        .filter(|item| item.starts_with("http://") || item.starts_with("https://"))
    {
        if let Ok(content) = fetch_remote(url)
            && !content.trim().is_empty()
        {
            sections.push(fmt_instruction_source(url, &content));
            sources.push(url.clone());
        }
    }

    Ok((sections.join("\n\n"), sources))
}

/// 构建系统提示词，使用内容缓存避免重复文件 I/O。
pub fn system_prompt_and_sources_with_cache(
    workspace_root: &Path,
    config_dir: &Path,
    instructions: &[String],
    cache: &HashMap<String, String>,
) -> Result<(String, Vec<String>, HashMap<String, String>)> {
    let mut sections = Vec::new();
    let mut sources = Vec::new();
    let mut new_cache = cache.clone();
    let paths = system_paths(workspace_root, config_dir, instructions)?;

    for path in paths {
        let path_str = path.display().to_string();
        if let Some(cached_content) = cache.get(&path_str) {
            tracing::info!(
                "system_prompt_and_sources_with_cache: HIT  path={}",
                path_str,
            );
            if !cached_content.trim().is_empty() {
                sections.push(fmt_instruction_source(
                    &path.display().to_string(),
                    cached_content,
                ));
                sources.push(path_str);
            }
        } else {
            tracing::info!(
                "system_prompt_and_sources_with_cache: MISS path={} cache_keys={:?}",
                path_str,
                cache.keys().collect::<Vec<_>>(),
            );
            if let Ok(content) = fs::read_to_string(&path)
                && !content.trim().is_empty()
            {
                new_cache.insert(path_str.clone(), content.clone());
                sections.push(fmt_instruction_source(
                    &path.display().to_string(),
                    &content,
                ));
                sources.push(path_str);
            }
        }
    }

    for url in instructions
        .iter()
        .filter(|item| item.starts_with("http://") || item.starts_with("https://"))
    {
        if let Some(cached_content) = cache.get(url) {
            if !cached_content.trim().is_empty() {
                sections.push(fmt_instruction_source(url, cached_content));
                sources.push(url.clone());
            }
        } else {
            if let Ok(content) = fetch_remote(url)
                && !content.trim().is_empty()
            {
                new_cache.insert(url.clone(), content.clone());
                sections.push(fmt_instruction_source(url, &content));
                sources.push(url.clone());
            }
        }
    }

    Ok((sections.join("\n\n"), sources, new_cache))
}

pub fn system_prompt(
    workspace_root: &Path,
    config_dir: &Path,
    instructions: &[String],
) -> Result<String> {
    Ok(system_prompt_and_sources(workspace_root, config_dir, instructions)?.0)
}

fn find_project_instruction(workspace_root: &Path) -> Result<Option<PathBuf>> {
    for ancestor in workspace_root.ancestors() {
        for file_name in INSTRUCTION_FILES {
            let candidate = ancestor.join(file_name);
            if candidate.exists() {
                return Ok(Some(candidate));
            }
        }
    }
    Ok(None)
}

fn resolve_instruction_paths(workspace_root: &Path, raw: &str) -> Result<Vec<PathBuf>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }

    let raw = if let Some(stripped) = raw.strip_prefix("~/") {
        dirs::home_dir()
            .map(|dir| dir.join(stripped))
            .unwrap_or_else(|| PathBuf::from(raw))
    } else {
        PathBuf::from(raw)
    };

    if raw.is_absolute() {
        if contains_glob(&raw) {
            return glob_absolute(&raw);
        }

        if raw.exists() {
            return Ok(vec![raw]);
        }

        return Ok(Vec::new());
    }

    if contains_glob(&raw) {
        return glob_relative(workspace_root, &raw);
    }

    let candidate = workspace_root.join(&raw);
    if candidate.exists() {
        return Ok(vec![candidate]);
    }

    Ok(Vec::new())
}

fn contains_glob(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.contains('*') || text.contains('?') || text.contains('[')
}

fn glob_relative(workspace_root: &Path, pattern: &Path) -> Result<Vec<PathBuf>> {
    let matcher = GlobBuilder::new(&pattern.to_string_lossy())
        .literal_separator(false)
        .build()
        .context("invalid glob pattern")?
        .compile_matcher();

    let mut results = Vec::new();
    let walker = WalkBuilder::new(workspace_root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .follow_links(false)
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if !entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false)
        {
            continue;
        }

        let path = entry.path();
        if let Ok(rel) = path.strip_prefix(workspace_root) {
            let candidate = rel.to_string_lossy();
            if matcher.is_match(&*candidate) {
                results.push(path.to_path_buf());
            }
        }
    }

    results.sort();
    Ok(results)
}

fn glob_absolute(pattern: &Path) -> Result<Vec<PathBuf>> {
    let matcher = GlobBuilder::new(&pattern.to_string_lossy())
        .literal_separator(false)
        .build()
        .context("invalid glob pattern")?
        .compile_matcher();

    let mut results = Vec::new();
    let root = Path::new("/");
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .follow_links(false)
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if !entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false)
        {
            continue;
        }

        let path = entry.path();
        let candidate = path.to_string_lossy();
        if matcher.is_match(&*candidate) {
            results.push(path.to_path_buf());
        }
    }

    results.sort();
    Ok(results)
}

fn fetch_remote(url: &str) -> Result<String> {
    validate_url(url)?;

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("failed to build http client")?;

    let response = client
        .get(url)
        .send()
        .context("failed to fetch remote instruction")?;

    let status = response.status();
    if !status.is_success() {
        return Ok(String::new());
    }

    response
        .text()
        .context("failed to read remote instruction body")
}

/// Validate URL to prevent SSRF attacks — blocks private/loopback IPs.
fn validate_url(url: &str) -> Result<()> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    let rest = rest.ok_or_else(|| anyhow::anyhow!("only http/https URLs are allowed"))?;
    let host = rest.split('/').next().unwrap_or(rest);
    let host = host.split(':').next().unwrap_or(host);

    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        if ip.is_loopback()
            || ip.is_private()
            || ip.is_link_local()
            || ip.is_broadcast()
            || ip == std::net::Ipv4Addr::new(169, 254, 169, 254)
        {
            anyhow::bail!("requests to private/loopback IPs are blocked");
        }
    } else if let Ok(ip) = host.parse::<std::net::Ipv6Addr>()
        && (ip.is_loopback()
            || (ip.segments()[0] & 0xfe00) == 0xfc00
            || (ip.segments()[0] & 0xffc0) == 0xfe80)
    {
        anyhow::bail!("requests to private/loopback IPv6 are blocked");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Result<Self> {
            let path = std::env::temp_dir().join(format!("{}-{}", prefix, uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).context("failed to create temp dir")?;
            Ok(Self { path })
        }

        fn path(&self) -> &PathBuf {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn make_temp_dir() -> Result<TempDir> {
        TempDir::new("cui-instructions")
    }

    #[test]
    fn system_paths_finds_project_agent_file() -> Result<()> {
        let workspace = make_temp_dir()?;
        let ws_path = workspace.path().clone();
        fs::write(ws_path.join("AGENTS.md"), "# Root")?;

        let paths = system_paths(&ws_path, &ws_path, &[])?;
        assert_eq!(paths, vec![ws_path.join("AGENTS.md").canonicalize()?]);
        Ok(())
    }

    #[test]
    fn system_paths_prefers_project_over_global() -> Result<()> {
        let workspace = make_temp_dir()?;
        let global = make_temp_dir()?;
        let ws_path = workspace.path().clone();
        let gl_path = global.path().clone();

        fs::write(ws_path.join("AGENTS.md"), "# Root")?;
        fs::write(gl_path.join("AGENTS.md"), "# Global")?;

        let paths = system_paths(&ws_path, &gl_path, &[])?;
        assert_eq!(
            paths,
            vec![
                ws_path.join("AGENTS.md").canonicalize()?,
                gl_path.join("AGENTS.md").canonicalize()?,
            ],
        );
        Ok(())
    }

    #[test]
    fn system_prompt_loads_config_instructions() -> Result<()> {
        let workspace = make_temp_dir()?;
        let global = make_temp_dir()?;
        let ws_path = workspace.path();
        let gl_path = global.path();
        let extra = ws_path.join("docs");
        fs::create_dir_all(&extra)?;
        fs::write(extra.join("style.md"), "# Style")?;

        let prompt = system_prompt(ws_path, gl_path, &["docs/style.md".to_string()])?;
        assert!(prompt.contains("指令来源："));
        assert!(prompt.contains("# Style"));
        Ok(())
    }

    #[test]
    fn system_paths_finds_github_copilot_instructions() -> Result<()> {
        let workspace = make_temp_dir()?;
        let ws_path = workspace.path();
        fs::create_dir_all(ws_path.join(".github"))?;
        fs::write(
            ws_path.join(".github").join("copilot-instructions.md"),
            "# Copilot",
        )?;

        let paths = system_paths(ws_path, ws_path, &[])?;
        assert_eq!(
            paths,
            vec![
                ws_path
                    .join(".github")
                    .join("copilot-instructions.md")
                    .canonicalize()?
            ]
        );
        Ok(())
    }

    #[test]
    fn resolve_nearby_instructions_finds_github_copilot_instructions() -> Result<()> {
        let workspace = make_temp_dir()?;
        let config_dir = TempDir::new("cui-test-config")?;
        let ws_path = workspace.path();
        let cf_path = config_dir.path();
        let subdir = ws_path.join("subdir").join("nested");
        fs::create_dir_all(&subdir)?;
        fs::create_dir_all(subdir.join(".github"))?;
        fs::write(
            subdir.join(".github").join("copilot-instructions.md"),
            "# Copilot",
        )?;
        fs::write(subdir.join("file.rs"), "let x = 1;")?;

        let results = resolve_nearby_instructions(ws_path, cf_path, &subdir.join("file.rs"))?;

        let expected_path = subdir
            .join(".github")
            .join("copilot-instructions.md")
            .canonicalize()?;
        let expected_content = format!("指令来源：{}\n{}", expected_path.display(), "# Copilot");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, expected_path);
        assert_eq!(results[0].1, expected_content);
        Ok(())
    }

    #[test]
    fn resolve_nearby_instructions_finds_subdirectory_agents() -> Result<()> {
        let workspace = make_temp_dir()?;
        let config_dir = TempDir::new("cui-test-config")?;
        let ws_path = workspace.path();
        let cf_path = config_dir.path();
        let subdir = ws_path.join("subdir").join("nested");
        fs::create_dir_all(&subdir)?;
        fs::write(ws_path.join("subdir").join("AGENTS.md"), "# Subdir")?;
        fs::write(subdir.join("file.rs"), "let x = 1;")?;

        let results = resolve_nearby_instructions(ws_path, cf_path, &subdir.join("file.rs"))?;
        let expected_path = ws_path.join("subdir").join("AGENTS.md").canonicalize()?;
        let expected_content = format!("指令来源：{}\n{}", expected_path.display(), "# Subdir");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, expected_path);
        assert_eq!(results[0].1, expected_content);
        Ok(())
    }
}
