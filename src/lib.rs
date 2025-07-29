mod file;
mod hasher;
#[cfg(feature = "json")]
mod json;
mod lazyfile;
mod metadata;
mod reflink;
mod scanner;
mod ui;

pub use crate::file::FileContent;
#[cfg(feature = "json")]
pub use crate::json::JsonOutput;
pub use crate::reflink::{LinkType, reflink, reflink_or_hardlink};
pub use crate::scanner::RunMode;
pub use crate::scanner::Scanner;
pub use crate::ui::UI as TextUserInterface;
