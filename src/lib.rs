#[cfg(feature = "json")]
extern crate serde_derive;
#[cfg(feature = "json")]
extern crate serde_json;

mod file;
mod hasher;
#[cfg(feature = "json")]
mod json;
mod lazyfile;
mod metadata;
mod scanner;
mod ui;

pub use crate::file::FileContent;
#[cfg(feature = "json")]
pub use crate::json::JsonOutput;
pub use crate::scanner::RunMode;
pub use crate::scanner::Scanner;
pub use crate::ui::UI as TextUserInterface;
