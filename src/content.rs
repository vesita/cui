//! 内容加载 —— 内置资源、提示词文件、系统指令的加载和缓存。

pub(crate) mod bundled;

#[cfg(feature = "prompts")]
pub(crate) mod prompt;

#[cfg(feature = "instructions")]
pub mod instructions;
