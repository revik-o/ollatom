use filesystem::{FilesystemError, create_file, create_folder};
use tempfile::tempdir;

#[tokio::test]
async fn creates_folders_and_files_and_reads_written_content() {
    let temporary_directory = tempdir().unwrap();
    let documents_directory_path = create_folder("documents", temporary_directory.path())
        .await
        .unwrap();
    let file_pointer = create_file("example.txt", &documents_directory_path)
        .await
        .unwrap();

    file_pointer
        .write_text("first")
        .await
        .unwrap()
        .append_text(" second")
        .await
        .unwrap();

    assert_eq!(file_pointer.read_text().await.unwrap(), "first second");
    assert_eq!(
        file_pointer.read_bytes().await.unwrap(),
        b"first second".to_vec()
    );
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

    let reopened_file_pointer = create_file("example.txt", temporary_directory.path())
        .await
        .unwrap();

    assert_eq!(
        reopened_file_pointer.read_text().await.unwrap(),
        "existing content"
    );
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
async fn atomically_replaces_file_contents() {
    let temporary_directory = tempdir().unwrap();
    let file_pointer = create_file("example.txt", temporary_directory.path())
        .await
        .unwrap();

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
    let file_pointer = create_file("example.bin", temporary_directory.path())
        .await
        .unwrap();

    file_pointer
        .write_bytes(b"old bytes")
        .await
        .unwrap()
        .write_bytes_atomically(b"new bytes")
        .await
        .unwrap();

    assert_eq!(
        file_pointer.read_bytes().await.unwrap(),
        b"new bytes".to_vec()
    );
}
