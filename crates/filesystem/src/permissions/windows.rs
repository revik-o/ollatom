use std::io;
use std::path::Path;

#[cfg(windows)]
pub(super) fn restrict_windows_directory(directory_path: &Path) -> io::Result<()> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

    let mut directory_open_options = std::fs::OpenOptions::new();
    directory_open_options
        .access_mode(
            windows_file_access::FILE_READ_ATTRIBUTES
                | windows_file_access::READ_CONTROL
                | windows_file_access::WRITE_DAC,
        )
        .share_mode(windows_file_access::FILE_SHARE_READ | windows_file_access::FILE_SHARE_WRITE)
        .custom_flags(
            windows_file_attribute::BACKUP_SEMANTICS | windows_file_attribute::OPEN_REPARSE_POINT,
        );
    let mut directory = directory_open_options.open(directory_path)?;
    let directory_metadata = directory.metadata()?;
    if !directory_metadata.is_dir()
        || directory_metadata.file_attributes() & windows_file_attribute::REPARSE_POINT != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private directory must be a directory and must not be a reparse point",
        ));
    }

    set_and_verify_windows_acl(&mut directory, true)
}

#[cfg(windows)]
pub(super) fn set_and_verify_windows_acl<FileHandle>(
    file: &mut FileHandle,
    is_directory: bool,
) -> io::Result<()>
where
    FileHandle: std::os::windows::io::AsRawHandle,
{
    use windows_permissions::constants::{
        AccessRights, AceFlags, AceType, SeObjectType, SecurityInformation,
    };

    use windows_permissions::wrappers::{GetSecurityInfo, SetSecurityInfo};
    use windows_permissions::{LocalBox, SecurityDescriptor};

    let current_user_sid = windows_permissions::utilities::current_process_sid()?;
    let original_security_descriptor = GetSecurityInfo(
        &*file,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Owner | SecurityInformation::Dacl,
    )?;

    if original_security_descriptor.owner() != Some(&current_user_sid) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private file system entry is not owned by the current user",
        ));
    }

    let system_sid: LocalBox<windows_permissions::Sid> = "S-1-5-18".parse()?;
    let inheritance_flags = if is_directory { "OICI" } else { "" };
    let system_access_control_entry = format!("(A;{inheritance_flags};FA;;;{system_sid})");

    let user_access_control_entry = if current_user_sid == system_sid {
        String::new()
    } else {
        format!("(A;{inheritance_flags};FA;;;{current_user_sid})")
    };

    let security_descriptor_sddl =
        format!("D:P{system_access_control_entry}{user_access_control_entry}");
    let private_security_descriptor: LocalBox<SecurityDescriptor> =
        security_descriptor_sddl.parse()?;
    let private_discretionary_access_control_list =
        private_security_descriptor.dacl().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "owner-only security descriptor is missing its access list",
            )
        })?;

    SetSecurityInfo(
        file,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Dacl | SecurityInformation::ProtectedDacl,
        None,
        None,
        Some(private_discretionary_access_control_list),
        None,
    )?;

    let secured_security_descriptor = GetSecurityInfo(
        &*file,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Owner | SecurityInformation::Dacl,
    )?;

    if secured_security_descriptor.owner() != Some(&current_user_sid) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private file system entry owner changed while applying its access list",
        ));
    }

    let secured_security_descriptor_sddl = secured_security_descriptor.as_sddl()?;

    if !secured_security_descriptor_sddl
        .to_string_lossy()
        .contains("D:P")
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private file system entry still inherits access permissions",
        ));
    }

    let discretionary_access_control_list =
        secured_security_descriptor.dacl().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "private file system entry has no access list",
            )
        })?;

    let expected_ace_count = (current_user_sid == system_sid).then_some(1).unwrap_or(2);

    if discretionary_access_control_list.len() != expected_ace_count {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private file system entry has access grants beyond the current user and system",
        ));
    }

    let expected_inheritance_flags = is_directory
        .then_some(AceFlags::ContainerInherit | AceFlags::ObjectInherit)
        .unwrap_or(AceFlags::empty());

    let mut system_access_granted = false;
    let mut user_access_granted = false;

    for ace_index in 0..discretionary_access_control_list.len() {
        let access_control_entry = discretionary_access_control_list
            .get_ace(ace_index)
            .ok_or_else(|| io::Error::other("private access list entry is unavailable"))?;
        if access_control_entry.ace_type() != AceType::ACCESS_ALLOWED_ACE_TYPE
            || access_control_entry.mask().bits() != AccessRights::FileAllAccess.bits()
            || access_control_entry.flags().bits() != expected_inheritance_flags.bits()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "private file system entry has an unexpected access grant",
            ));
        }
        match access_control_entry.sid() {
            Some(access_sid) if access_sid == &*system_sid && !system_access_granted => {
                system_access_granted = true;
            }
            Some(access_sid)
                if access_sid == &*current_user_sid
                    && !user_access_granted
                    && current_user_sid != system_sid =>
            {
                user_access_granted = true;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "private file system entry grants access to an unexpected principal",
                ));
            }
        }
    }
    if !system_access_granted || (current_user_sid != system_sid && !user_access_granted) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private file system entry is missing current-user or system access",
        ));
    }

    Ok(())
}
#[cfg(windows)]
pub(crate) mod windows_file_access {
    pub const FILE_GENERIC_WRITE: u32 = 0x0012_0116;
    pub const FILE_READ_ATTRIBUTES: u32 = 0x0000_0080;
    pub const READ_CONTROL: u32 = 0x0002_0000;
    pub const WRITE_DAC: u32 = 0x0004_0000;
    pub const FILE_SHARE_READ: u32 = 0x0000_0001;
    pub const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    pub const FILE_SHARE_NONE: u32 = 0;
}

#[cfg(windows)]
pub(crate) mod windows_file_attribute {
    pub const REPARSE_POINT: u32 = 0x0000_0400;
    pub const BACKUP_SEMANTICS: u32 = 0x0200_0000;
    pub const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
}
