use super::*;

#[cfg(unix)]
#[tokio::test]
async fn retained_workspace_file_rejects_parent_replacement() {
    use std::os::unix::fs::symlink;

    let temporary_directory = tempdir().unwrap();
    let outside_directory = tempdir().unwrap();
    std::fs::create_dir(temporary_directory.path().join("nested")).unwrap();
    std::fs::write(temporary_directory.path().join("nested/file"), b"inside").unwrap();
    std::fs::write(outside_directory.path().join("file"), b"outside").unwrap();

    let workspace = WorkspaceDirectory::open(temporary_directory.path()).await.unwrap();
    let retained_file = workspace.file("nested/file").unwrap();

    std::fs::rename(
        temporary_directory.path().join("nested"),
        temporary_directory.path().join("replaced"),
    )
    .unwrap();
    symlink(outside_directory.path(), temporary_directory.path().join("nested")).unwrap();

    assert!(retained_file.read_regular_file_bytes_bounded(100).await.is_err());
    assert_eq!(
        std::fs::read(outside_directory.path().join("file")).unwrap(),
        b"outside"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn bounded_read_and_writes_reject_destination_symlinks() {
    use std::os::unix::fs::symlink;

    let temporary_directory = tempdir().unwrap();
    let target_path = temporary_directory.path().join("target");
    let link_path = temporary_directory.path().join("link");
    std::fs::write(&target_path, b"target").unwrap();
    symlink(&target_path, &link_path).unwrap();
    let file = filesystem::FilePointer::from_path(&link_path);

    assert!(file.read_bytes_bounded(100).await.is_err());
    assert!(matches!(
        file.write_text("changed").await,
        Err(FilesystemError::SymbolicLinkNotAllowed { .. })
    ));
    assert!(matches!(
        file.write_text_atomically("changed").await,
        Err(FilesystemError::SymbolicLinkNotAllowed { .. })
    ));
    assert_eq!(std::fs::read(target_path).unwrap(), b"target");
}

#[tokio::test]
async fn workspace_rename_never_replaces_destination() {
    let temporary_directory = tempdir().unwrap();
    std::fs::write(temporary_directory.path().join("source"), b"source").unwrap();
    std::fs::write(temporary_directory.path().join("destination"), b"destination").unwrap();
    let workspace = WorkspaceDirectory::open(temporary_directory.path()).await.unwrap();
    let source = workspace.file("source").unwrap();
    let destination = workspace.file("destination").unwrap();

    assert!(matches!(
        source.rename_without_replacement(&destination).await,
        Err(FilesystemError::DestinationAlreadyExists { .. })
    ));
    assert_eq!(
        std::fs::read(temporary_directory.path().join("source")).unwrap(),
        b"source"
    );
    assert_eq!(
        std::fs::read(temporary_directory.path().join("destination")).unwrap(),
        b"destination"
    );
}

#[tokio::test]
async fn workspace_rename_moves_regular_file_to_absent_destination() {
    let temporary_directory = tempdir().unwrap();
    std::fs::write(temporary_directory.path().join("source"), b"source").unwrap();
    let workspace = WorkspaceDirectory::open(temporary_directory.path()).await.unwrap();

    workspace
        .file("source")
        .unwrap()
        .rename_without_replacement(&workspace.file("destination").unwrap())
        .await
        .unwrap();

    assert!(!temporary_directory.path().join("source").exists());
    assert_eq!(
        std::fs::read(temporary_directory.path().join("destination")).unwrap(),
        b"source"
    );
}

#[tokio::test]
async fn dropping_prepared_atomic_write_keeps_destination_unchanged() {
    let temporary_directory = tempdir().unwrap();
    std::fs::write(temporary_directory.path().join("file"), b"old").unwrap();
    let workspace = WorkspaceDirectory::open(temporary_directory.path()).await.unwrap();
    let file = workspace.file("file").unwrap();
    let prepared = file
        .prepare_atomic_write(b"new".to_vec(), AtomicMetadataPolicy::ContentOnly)
        .await
        .unwrap();

    drop(prepared);
    assert_eq!(std::fs::read(temporary_directory.path().join("file")).unwrap(), b"old");
}

#[tokio::test]
async fn started_atomic_commit_has_definitive_completion() {
    let temporary_directory = tempdir().unwrap();
    std::fs::write(temporary_directory.path().join("file"), b"old").unwrap();
    let workspace = WorkspaceDirectory::open(temporary_directory.path()).await.unwrap();
    let file = workspace.file("file").unwrap();
    let handle = file
        .prepare_atomic_write(b"new".to_vec(), AtomicMetadataPolicy::ContentOnly)
        .await
        .unwrap()
        .start_commit()
        .unwrap();

    assert_eq!(handle.wait().await, AtomicCommitStatus::Succeeded);
    assert_eq!(std::fs::read(temporary_directory.path().join("file")).unwrap(), b"new");
}
