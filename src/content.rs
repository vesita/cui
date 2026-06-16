//! 内容加载 —— 内置资源、提示词文件、系统指令的加载和缓存。

pub mod bundled;

#[cfg(feature = "prompts")]
pub mod prompt;

#[cfg(feature = "instructions")]
pub mod instructions;
