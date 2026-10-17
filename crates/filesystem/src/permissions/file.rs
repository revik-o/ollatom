#[cfg(windows)]
use super::windows::set_and_verify_windows_acl;
#[cfg(windows)]
use super::{windows_file_access, windows_file_attribute};
use std::fs::File;
use std::io;
use std::path::Path;

pub fn restrict_open_file_to_current_user(file: &mut File) -> io::Result<()> {
    #[cfg(unix)]
    {
        restrict_unix_std_file(file)
    }
    #[cfg(windows)]
    {
        restrict_windows_std_file(file)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err(unsupported_file_permissions())
    }
}

pub fn open_or_create_owner_only_file(path: &Path) -> io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(0o600).custom_flags(nix::fcntl::OFlag::O_NOFOLLOW.bits());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;

        options
            .access_mode(
                windows_file_access::FILE_GENERIC_WRITE
                    | windows_file_access::FILE_READ_ATTRIBUTES
                    | windows_file_access::READ_CONTROL
                    | windows_file_access::WRITE_DAC,
            )
            .share_mode(windows_file_access::FILE_SHARE_READ | windows_file_access::FILE_SHARE_WRITE)
            .custom_flags(windows_file_attribute::OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;

    restrict_open_file_to_current_user(&mut file)?;

    Ok(file)
}

pub(crate) fn restrict_capability_file_to_current_user(file: &mut cap_std::fs::File) -> io::Result<()> {
    #[cfg(unix)]
    {
        restrict_unix_capability_file(file)
    }
    #[cfg(windows)]
    {
        restrict_windows_capability_file(file)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err(unsupported_file_permissions())
    }
}
#[cfg(not(any(unix, windows)))]
fn unsupported_file_permissions() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "owner-only file permissions are unavailable on this platform",
    )
}

#[cfg(unix)]
fn restrict_unix_std_file(file: &mut File) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = file.metadata()?;

    validate_unix_private_file(metadata.is_file(), metadata.uid(), metadata.nlink())?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;

    verify_unix_private_file_mode(file.metadata()?.mode())
}

#[cfg(unix)]
fn restrict_unix_capability_file(file: &mut cap_std::fs::File) -> io::Result<()> {
    use cap_std::fs::{MetadataExt, PermissionsExt};

    let metadata = file.metadata()?;

    validate_unix_private_file(metadata.is_file(), metadata.uid(), metadata.nlink())?;
    file.set_permissions(cap_std::fs::Permissions::from_mode(0o600))?;

    verify_unix_private_file_mode(file.metadata()?.mode())
}

#[cfg(unix)]
fn validate_unix_private_file(is_regular_file: bool, owner_id: u32, link_count: u64) -> io::Result<()> {
    if !is_regular_file || owner_id != nix::unistd::geteuid().as_raw() || link_count != 1 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private file must be a regular, current-user-owned file without hard links",
        ));
    }

    Ok(())
}

#[cfg(unix)]
fn verify_unix_private_file_mode(mode: u32) -> io::Result<()> {
    if mode & 0o7777 != 0o600 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "file does not have owner-only permissions",
        ));
    }

    Ok(())
}
#[cfg(windows)]
fn restrict_windows_std_file(file: &mut File) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;

    let file_metadata = file.metadata()?;
    if !file_metadata.is_file() || file_metadata.file_attributes() & windows_file_attribute::REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private file must be a regular file and must not be a reparse point",
        ));
    }

    set_and_verify_windows_acl(file, false)
}

#[cfg(windows)]
fn restrict_windows_capability_file(file: &mut cap_std::fs::File) -> io::Result<()> {
    use cap_std::fs::MetadataExt;

    let file_metadata = file.metadata()?;

    if !file_metadata.is_file() || file_metadata.file_attributes() & windows_file_attribute::REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private file must be a regular file and must not be a reparse point",
        ));
    }

    set_and_verify_windows_acl(file, false)
}
