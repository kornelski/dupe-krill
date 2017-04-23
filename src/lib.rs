extern crate sha1;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

mod dupe;
mod file;
mod hasher;
mod lazyfile;
mod metadata;
mod ui;
mod json;

pub use dupe::Scanner;
pub use file::FileContent;
pub use ui::UI as TextUserInterface;
pub use json::JsonOutput as JsonOutput;
