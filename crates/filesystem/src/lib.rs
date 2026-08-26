mod error;
mod file;

pub use error::FilesystemError;
pub use file::{FilePointer, create_file, create_folder};
