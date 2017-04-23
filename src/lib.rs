extern crate sha1;

#[cfg(feature = "json")]
extern crate serde;
#[cfg(feature = "json")]
extern crate serde_json;
#[cfg(feature = "json")]
#[macro_use]
extern crate serde_derive;

mod dupe;
mod file;
mod hasher;
mod lazyfile;
mod metadata;
mod ui;
#[cfg(feature = "json")]
mod json;

pub use dupe::Scanner;
pub use dupe::RunMode;
pub use file::FileContent;
pub use ui::UI as TextUserInterface;
#[cfg(feature = "json")]
pub use json::JsonOutput as JsonOutput;
