//! 编译时扫描 `cui/` 目录，嵌入所有 `.cui` 文件。
//! 生成 `bundled_cui.rs`，供 `CuiDirectory::scan_root()` 使用。

use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let cui_dir = Path::new(&manifest_dir).join("cui");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("bundled_cui.rs");

    let mut out = fs::File::create(&dest_path).unwrap();

    writeln!(out, "vec![").unwrap();

    if cui_dir.exists() {
        let entries = fs::read_dir(&cui_dir).unwrap();
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "cui") {
                let fname = path.file_name().unwrap().to_str().unwrap().to_string();
                writeln!(
                    out,
                    "    ({:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/cui/{}\"))),",
                    fname, fname
                )
                .unwrap();
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }

    writeln!(out, "]").unwrap();

    println!("cargo:rerun-if-changed={}", cui_dir.display());
}
