use super::*;

#[tokio::test]
async fn creates_folders_and_files_and_reads_written_content() {
    let temporary_directory = tempdir().unwrap();
    let documents_directory_path = create_folder("documents", temporary_directory.path()).await.unwrap();
    let file_pointer = create_file("example.txt", &documents_directory_path).await.unwrap();

    file_pointer
        .write_text("first")
        .await
        .unwrap()
        .append_text(" second")
        .await
        .unwrap();

    assert_eq!(file_pointer.read_text().await.unwrap(), "first second");
    assert_eq!(file_pointer.read_bytes().await.unwrap(), b"first second".to_vec());
}

#[tokio::test]
async fn opening_an_existing_file_does_not_truncate_it() {
    let temporary_directory = tempdir().unwrap();
    create_file("example.txt", temporary_directory.path())
        .await
        .unwrap()
        .write_text("existing content")
        .await
        .unwrap();

    let reopened_file_pointer = create_file("example.txt", temporary_directory.path()).await.unwrap();

    assert_eq!(reopened_file_pointer.read_text().await.unwrap(), "existing content");
}

#[cfg(unix)]
#[tokio::test]
async fn created_files_have_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temporary_directory = tempdir().unwrap();
    let file_path = temporary_directory.path().join("private.txt");

    create_file("private.txt", temporary_directory.path()).await.unwrap();

    assert_eq!(
        std::fs::metadata(file_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[tokio::test]
async fn create_file_rejects_symlinks_without_modifying_the_target() {
    use std::os::unix::fs::symlink;

    let temporary_directory = tempdir().unwrap();
    let target_path = temporary_directory.path().join("target.txt");
    let link_path = temporary_directory.path().join("link.txt");
    std::fs::write(&target_path, "preserve this file").unwrap();
    symlink(&target_path, &link_path).unwrap();

    assert!(create_file("link.txt", temporary_directory.path()).await.is_err());
    assert_eq!(std::fs::read_to_string(&target_path).unwrap(), "preserve this file");
    assert!(std::fs::symlink_metadata(link_path).unwrap().file_type().is_symlink());
}

#[tokio::test]
async fn rejects_entry_names_that_escape_the_parent_directory() {
    let temporary_directory = tempdir().unwrap();
    let result = create_file("../outside.txt", temporary_directory.path()).await;

    assert!(matches!(
        result,
        Err(FilesystemError::InvalidFilesystemEntryName { .. })
    ));
}

#[tokio::test]
async fn invalid_file_name_does_not_create_parent_directory() {
    let temporary_directory = tempdir().unwrap();
    let missing_parent = temporary_directory.path().join("missing");

    assert!(create_file("../outside", &missing_parent).await.is_err());
    assert!(!missing_parent.exists());
}

#[tokio::test]
async fn atomically_replaces_file_contents() {
    let temporary_directory = tempdir().unwrap();
    let file_pointer = create_file("example.txt", temporary_directory.path()).await.unwrap();

    file_pointer
        .write_text("old contents")
        .await
        .unwrap()
        .write_text_atomically("new contents")
        .await
        .unwrap();

    assert_eq!(file_pointer.read_text().await.unwrap(), "new contents");
}

#[tokio::test]
async fn atomically_replaces_binary_file_contents() {
    let temporary_directory = tempdir().unwrap();
    let file_pointer = create_file("example.bin", temporary_directory.path()).await.unwrap();

    file_pointer
        .write_bytes(b"old bytes")
        .await
        .unwrap()
        .write_bytes_atomically(b"new bytes")
        .await
        .unwrap();

    assert_eq!(file_pointer.read_bytes().await.unwrap(), b"new bytes".to_vec());
}

#[tokio::test]
async fn bounded_read_rejects_content_beyond_limit() {
    let temporary_directory = tempdir().unwrap();
    let file_pointer = create_file("example.txt", temporary_directory.path()).await.unwrap();
    file_pointer.write_text("12345").await.unwrap();

    assert!(matches!(
        file_pointer.read_bytes_bounded(4).await,
        Err(FilesystemError::FileSizeLimitExceeded { .. })
    ));
}

#[tokio::test]
async fn workspace_rejects_absolute_and_parent_paths() {
    let temporary_directory = tempdir().unwrap();
    let workspace = WorkspaceDirectory::open(temporary_directory.path()).await.unwrap();

    assert!(matches!(
        workspace.file("../outside"),
        Err(FilesystemError::InvalidWorkspaceRelativePath { .. })
    ));
    assert!(matches!(
        workspace.file(temporary_directory.path().join("absolute")),
        Err(FilesystemError::InvalidWorkspaceRelativePath { .. })
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn workspace_read_rejects_symlink_components() {
    use std::os::unix::fs::symlink;

    let temporary_directory = tempdir().unwrap();
    let outside_directory = tempdir().unwrap();
    std::fs::write(outside_directory.path().join("secret"), b"secret").unwrap();
    symlink(outside_directory.path(), temporary_directory.path().join("link")).unwrap();
    let workspace = WorkspaceDirectory::open(temporary_directory.path()).await.unwrap();
    let file = workspace.file("link/secret").unwrap();

    assert!(file.read_regular_file_bytes_bounded(100).await.is_err());
}
