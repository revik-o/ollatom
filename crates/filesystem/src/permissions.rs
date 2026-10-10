mod directory;
mod file;

#[cfg(windows)]
mod windows;

pub use directory::restrict_directory_to_current_user;
pub(crate) use file::restrict_capability_file_to_current_user;
pub use file::{open_or_create_owner_only_file, restrict_open_file_to_current_user};

#[cfg(windows)]
pub(crate) use windows::{windows_file_access, windows_file_attribute};
