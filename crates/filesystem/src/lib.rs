mod error;
mod file;
mod permissions;
mod workspace;

pub use error::FilesystemError;
pub use file::{FilePointer, create_file, create_folder};
pub use permissions::{
    open_or_create_owner_only_file, restrict_directory_to_current_user,
    restrict_open_file_to_current_user,
};
pub use workspace::{
    AtomicCommitHandle, AtomicCommitIdentifier, AtomicCommitStatus, AtomicMetadataPolicy,
    DirectoryIdentity, PreparedAtomicWrite, WorkspaceCapability, WorkspaceDirectory, WorkspaceFile,
    WorkspaceRelativePath,
};
