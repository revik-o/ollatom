use std::{fs, path::Path};

const MAXIMUM_RUST_SOURCE_FILE_LINES: usize = 270;

#[derive(Clone, Copy)]
enum InlineTestPolicy {
    Allow,
    Forbid,
}

#[test]
fn every_llm_rust_file_respects_the_source_line_limit() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    validate_source_tree(&crate_root.join("src"), InlineTestPolicy::Forbid);
    validate_source_tree(&crate_root.join("tests"), InlineTestPolicy::Allow);
}

fn validate_source_tree(directory: &Path, inline_test_policy: InlineTestPolicy) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();

        if path.is_dir() {
            validate_source_tree(&path, inline_test_policy);
            continue;
        }

        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }

        validate_rust_source_file(&path, inline_test_policy);
    }
}

fn validate_rust_source_file(path: &Path, inline_test_policy: InlineTestPolicy) {
    let source = fs::read_to_string(path).unwrap();
    assert!(
        source.lines().count() <= MAXIMUM_RUST_SOURCE_FILE_LINES,
        "{} exceeds {MAXIMUM_RUST_SOURCE_FILE_LINES} lines",
        path.display()
    );

    if matches!(inline_test_policy, InlineTestPolicy::Forbid) {
        assert!(
            !source.lines().any(is_inline_test_declaration),
            "{} contains inline tests",
            path.display()
        );
    }
}

fn is_inline_test_declaration(source_line: &str) -> bool {
    let source_line = source_line.trim();
    let is_test_configuration = source_line == "#[cfg(test)]";
    let is_test_module_declaration = source_line == "mod tests;";
    let is_inline_test_module = source_line.starts_with("mod tests {");

    is_test_configuration || is_test_module_declaration || is_inline_test_module
}
