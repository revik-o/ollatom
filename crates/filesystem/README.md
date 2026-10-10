# Filesystem

The filesystem crate provides asynchronous utilities for capability-scoped files and directories, bounded reads, path validation, and atomic writes.

Every file and directory creation validates entry names to ensure they consist of exactly one normal path component, preventing accidental directory traversal or invalid paths.

## Public API Overview

### 1. Folder Creation

```rust
use filesystem::create_folder;
use std::path::Path;

let folder_path = create_folder("my_folder", Path::new("/path/to/parent")).await?;
```
**Description:** Creates a new folder inside the specified parent directory. The `folder_name` is strictly validated to ensure it contains exactly one path component. If the parent directories do not exist, they are created automatically (`fs::create_dir_all`). Returns the `PathBuf` of the created folder.

### 2. File Creation

```rust
use filesystem::create_file;
use std::path::Path;

let file_pointer = create_file("my_file.txt", Path::new("/path/to/folder")).await?;
```
**Description:** Creates a new file inside the specified directory and returns a `FilePointer`. The `file_name` is strictly validated. The parent directory is created if it does not exist. If the file already exists, it is opened without truncating its contents.

### 3. File Operations (`FilePointer`)

```rust
use filesystem::create_file;
use std::path::Path;

let file = create_file("settings.json", Path::new("/path/to/config")).await?;

// Read operations
let text = file.read_text().await?;
let bytes = file.read_bytes().await?;

// Standard write and append
file.write_text("Hello World").await?;
file.write_bytes(b"Raw Data").await?;
file.append_text("\nAppended text").await?;
file.append_bytes(b"\nMore bytes").await?;

// Atomic write operations
file.write_text_atomically("Safe content").await?;
file.write_bytes_atomically(b"Safe bytes").await?;

// Access underlying path
let path = file.path();
```
**Description:** `FilePointer` provides a compatibility API for ambient paths. Its writes, appends, bounded reads, and atomic replacements delegate to the descriptor-relative workspace implementation and reject destination symbolic links.

For security-sensitive operations, open a `WorkspaceDirectory` once and derive `WorkspaceFile` values from validated relative paths. Workspace operations retain directory handles, reject absolute paths, parent traversal, and symbolic-link traversal, and never resolve an approved operation from an ambient absolute path again.

`read_regular_file_bytes_bounded` and `read_regular_file_text_bounded` open a file once without following links, require a regular file, and read at most one byte beyond the configured limit. `rename_without_replacement` never destroys an existing destination and supports regular files.

Atomic writes have separate prepare and commit phases. Dropping `PreparedAtomicWrite` removes its private temporary file. After `start_commit`, the operation continues independently of the waiting task and `AtomicCommitHandle::wait` reports its definitive result. `ContentOnly` replaces content, `PreserveSupported` preserves the metadata supported by this implementation, and `RequirePreserved` fails closed where the complete security metadata set cannot be preserved and verified.

### 4. Errors (`FilesystemError`)

```rust
use filesystem::FilesystemError;

// Errors include:
// - FilesystemError::InvalidFilesystemEntryName
// - FilesystemError::InputOutputOperation
// - FilesystemError::FilesystemOperationTaskFailed
```
**Description:** The crate defines a custom `thiserror` based `FilesystemError` enum to encapsulate path validation errors (`InvalidFilesystemEntryName`), standard I/O errors (`InputOutputOperation`), and errors occurring during blocking tasks like atomic writes (`FilesystemOperationTaskFailed`).
