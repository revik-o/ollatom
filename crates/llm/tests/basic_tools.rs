use llm::{BasicToolConfiguration, register_basic_tools};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temporary_root() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("llm-basic-tools-{suffix}"))
}

#[test]
fn basic_tool_registry_contains_general_file_command_and_search_tools() {
    let root = temporary_root();
    let registry = register_basic_tools(BasicToolConfiguration::new(&root)).unwrap();
    let names = registry
        .definitions()
        .into_iter()
        .map(|definition| definition.name)
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "create_directory",
            "delete_path",
            "execute_command",
            "list_files",
            "read_file",
            "rename_path",
            "web_search",
            "write_file",
        ]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn basic_tool_registry_requires_an_absolute_root() {
    assert!(register_basic_tools(BasicToolConfiguration::new("relative-root")).is_err());
}
