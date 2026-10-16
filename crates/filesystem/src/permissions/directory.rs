use std::io;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

pub fn restrict_directory_to_current_user(directory_path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        restrict_unix_directory(directory_path)
    }
    #[cfg(windows)]
    {
        super::windows::restrict_windows_directory(directory_path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = directory_path;

        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "owner-only directory permissions are unavailable on this platform",
        ))
    }
}

#[cfg(unix)]
fn restrict_unix_directory(directory_path: &Path) -> io::Result<()> {
    let mut directory_open_options = std::fs::OpenOptions::new();

    directory_open_options
        .read(true)
        .custom_flags(nix::fcntl::OFlag::O_DIRECTORY.bits() | nix::fcntl::OFlag::O_NOFOLLOW.bits());

    let directory = directory_open_options.open(directory_path)?;
    let directory_metadata = directory.metadata()?;

    if !directory_metadata.is_dir() || directory_metadata.uid() != nix::unistd::geteuid().as_raw() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private directory must be a current-user-owned directory",
        ));
    }

    directory.set_permissions(std::fs::Permissions::from_mode(0o700))?;

    let secured_directory_metadata = directory.metadata()?;

    if secured_directory_metadata.mode() & 0o7777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "directory does not have owner-only permissions",
        ));
    }

    Ok(())
}
