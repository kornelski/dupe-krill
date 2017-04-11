extern crate sha1;

mod dupe;
mod file;
mod hasher;
mod lazyfile;
mod metadata;
mod ui;

pub use dupe::Scanner;
pub use file::FileContent;
pub use ui::UI as TextUserInterface;
