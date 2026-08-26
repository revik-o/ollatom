use std::fs;
use std::path::Path;

#[test]
fn infrastructure_source_files_follow_the_required_structure() {
    let source_directory_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    verify_source_directory(&source_directory_path);
    let migration_directory_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    verify_migration_directory(&migration_directory_path);
    verify_role_specific_message_columns(&migration_directory_path);
    verify_sqlx_is_private_to_infrastructure();
    verify_no_source_test_modules();
}

fn verify_no_source_test_modules() {
    let workspace_directory_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for source_parent_directory in ["apps", "crates"] {
        verify_directory_has_no_source_test_modules(
            &workspace_directory_path.join(source_parent_directory),
        );
    }
}

fn verify_directory_has_no_source_test_modules(directory_path: &Path) {
    for directory_entry in fs::read_dir(directory_path).unwrap() {
        let entry_path = directory_entry.unwrap().path();
        if entry_path
            .file_name()
            .is_some_and(|file_name| file_name == "tests")
        {
            continue;
        }
        if entry_path.is_dir() {
            verify_directory_has_no_source_test_modules(&entry_path);
            continue;
        }
        if entry_path
            .extension()
            .is_none_or(|extension| extension != "rs")
        {
            continue;
        }
        let source_text = fs::read_to_string(&entry_path).unwrap();
        assert!(
            !source_text.contains("#[cfg(test)]") && !source_text.contains("mod tests"),
            "{} contains a source test module",
            entry_path.display()
        );
    }
}

fn verify_role_specific_message_columns(directory_path: &Path) {
    let migration_text =
        fs::read_to_string(directory_path.join("0001_initial_schema.sql")).unwrap();
    for column_name in [
        "user_revision_group_id",
        "user_revision_number",
        "llm_reply_to_user_message_id",
        "llm_response_round_number",
        "llm_message_state",
    ] {
        assert!(migration_text.contains(column_name));
    }
}

fn verify_sqlx_is_private_to_infrastructure() {
    let workspace_directory_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    verify_directory_has_no_sqlx_references(
        &workspace_directory_path.join("apps"),
        &workspace_directory_path.join("crates/infrastructure"),
    );
    verify_directory_has_no_sqlx_references(
        &workspace_directory_path.join("crates"),
        &workspace_directory_path.join("crates/infrastructure"),
    );
}

fn verify_directory_has_no_sqlx_references(directory_path: &Path, infrastructure_path: &Path) {
    if !directory_path.exists() {
        return;
    }
    for directory_entry in fs::read_dir(directory_path).unwrap() {
        let entry_path = directory_entry.unwrap().path();
        if entry_path == *infrastructure_path {
            continue;
        }
        if entry_path.is_dir() {
            verify_directory_has_no_sqlx_references(&entry_path, infrastructure_path);
            continue;
        }
        let extension = entry_path
            .extension()
            .and_then(|extension| extension.to_str());
        if !matches!(extension, Some("rs") | Some("toml")) {
            continue;
        }
        let source_text = fs::read_to_string(&entry_path).unwrap();
        assert!(
            !source_text.contains("sqlx"),
            "{} references SQLx outside infrastructure",
            entry_path.display()
        );
    }
}

fn verify_migration_directory(directory_path: &Path) {
    for directory_entry in fs::read_dir(directory_path).unwrap() {
        let migration_path = directory_entry.unwrap().path();
        let migration_text = fs::read_to_string(&migration_path).unwrap();
        assert!(
            !migration_text.contains("--") && !migration_text.contains("/*"),
            "{} contains a migration comment",
            migration_path.display()
        );
    }
}

fn verify_source_directory(directory_path: &Path) {
    for directory_entry in fs::read_dir(directory_path).unwrap() {
        let directory_entry = directory_entry.unwrap();
        let entry_path = directory_entry.path();
        if entry_path.is_dir() {
            verify_source_directory(&entry_path);
            continue;
        }
        if entry_path
            .extension()
            .is_none_or(|extension| extension != "rs")
        {
            continue;
        }
        let source_text = fs::read_to_string(&entry_path).unwrap();
        assert!(
            source_text.lines().count() <= 250,
            "{} exceeds 250 lines",
            entry_path.display()
        );
        assert!(
            !source_text.contains("#[cfg(test)]"),
            "{} contains a source test module",
            entry_path.display()
        );
        assert!(
            !source_text
                .lines()
                .any(|source_line| source_line.trim_start().starts_with("//")
                    || source_line.contains("/*")),
            "{} contains a source comment",
            entry_path.display()
        );
    }
}
