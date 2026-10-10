use crate::permissions::*;
use std::fs::File;
use std::path::Path;

const FILE_SHARE_DELETE: u32 = 0x0000_0004;

use std::os::windows::fs::OpenOptionsExt;
use windows_permissions::constants::{
    AccessRights, AceFlags, AceType, SeObjectType, SecurityInformation,
};
use windows_permissions::wrappers::GetSecurityInfo;
use windows_permissions::{LocalBox, Sid};

#[test]
fn private_directory_and_file_acl_grant_only_user_and_system() {
    let temporary_directory = tempfile::tempdir().unwrap();
    let private_directory = temporary_directory.path().join("private");
    std::fs::create_dir(&private_directory).unwrap();
    restrict_directory_to_current_user(&private_directory).unwrap();

    let mut directory_options = std::fs::OpenOptions::new();
    directory_options
        .read(true)
        .access_mode(windows_file_access::READ_CONTROL)
        .share_mode(
            windows_file_access::FILE_SHARE_READ
                | windows_file_access::FILE_SHARE_WRITE
                | FILE_SHARE_DELETE,
        )
        .custom_flags(
            windows_file_attribute::BACKUP_SEMANTICS | windows_file_attribute::OPEN_REPARSE_POINT,
        );
    let directory = directory_options.open(&private_directory).unwrap();
    assert_private_acl(&directory, true);

    let private_file_path = private_directory.join("private.txt");
    let private_file = open_or_create_owner_only_file(&private_file_path).unwrap();
    assert_private_acl(&private_file, false);
}

#[tokio::test]
async fn created_and_atomically_replaced_files_keep_private_acl() {
    let temporary_directory = tempfile::tempdir().unwrap();
    let file_pointer = crate::file::create_file("private.txt", temporary_directory.path())
        .await
        .unwrap();
    let file_path = temporary_directory.path().join("private.txt");

    let created_file = open_file_for_acl_verification(&file_path);
    assert_private_acl(&created_file, false);
    drop(created_file);

    file_pointer
        .write_text_atomically("private content")
        .await
        .unwrap();

    let atomically_replaced_file = open_file_for_acl_verification(&file_path);
    assert_private_acl(&atomically_replaced_file, false);
}

fn open_file_for_acl_verification(file_path: &Path) -> File {
    use std::os::windows::fs::OpenOptionsExt;

    let mut file_options = std::fs::OpenOptions::new();
    file_options
        .read(true)
        .access_mode(windows_file_access::READ_CONTROL)
        .share_mode(
            windows_file_access::FILE_SHARE_READ
                | windows_file_access::FILE_SHARE_WRITE
                | FILE_SHARE_DELETE,
        );
    file_options.open(file_path).unwrap()
}

fn assert_private_acl(file: &File, is_directory: bool) {
    let current_user_sid = windows_permissions::utilities::current_process_sid().unwrap();
    let system_sid: LocalBox<Sid> = "S-1-5-18".parse().unwrap();
    let descriptor = GetSecurityInfo(
        file,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Owner | SecurityInformation::Dacl,
    )
    .unwrap();
    assert_eq!(descriptor.owner(), Some(&*current_user_sid));
    assert!(
        descriptor
            .as_sddl()
            .unwrap()
            .to_string_lossy()
            .contains("D:P")
    );

    let discretionary_access_control_list = descriptor.dacl().unwrap();
    let expected_ace_count = if current_user_sid == system_sid { 1 } else { 2 };
    assert_eq!(discretionary_access_control_list.len(), expected_ace_count);
    let expected_flags = if is_directory {
        AceFlags::ContainerInherit | AceFlags::ObjectInherit
    } else {
        AceFlags::empty()
    };
    let mut system_access_granted = false;
    let mut user_access_granted = false;
    for ace_index in 0..discretionary_access_control_list.len() {
        let access_control_entry = discretionary_access_control_list
            .get_ace(ace_index)
            .unwrap();
        assert_eq!(
            access_control_entry.ace_type(),
            AceType::ACCESS_ALLOWED_ACE_TYPE
        );
        assert_eq!(
            access_control_entry.mask().bits(),
            AccessRights::FileAllAccess.bits()
        );
        assert_eq!(access_control_entry.flags().bits(), expected_flags.bits());
        match access_control_entry.sid() {
            Some(access_sid) if access_sid == &*system_sid => system_access_granted = true,
            Some(access_sid) if access_sid == &*current_user_sid => {
                user_access_granted = true;
            }
            _ => panic!("private ACL contains an unexpected access principal"),
        }
    }

    assert!(system_access_granted);
    assert!(user_access_granted || current_user_sid == system_sid);
}
