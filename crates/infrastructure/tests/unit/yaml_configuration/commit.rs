use crate::yaml_configuration::YamlConfigurationError;
use crate::yaml_configuration::store::commit::identifiers::allocate_configuration_commit_identifier;
use std::path::Path;
use std::sync::atomic::AtomicU64;

#[test]
fn commit_identifiers_stop_before_wrapping_to_zero() {
    let next_identifier = AtomicU64::new(u64::MAX);
    let file_path = Path::new("settings.yaml");

    assert_eq!(
        allocate_configuration_commit_identifier(&next_identifier, file_path)
            .unwrap()
            .as_u64(),
        u64::MAX
    );
    assert!(matches!(
        allocate_configuration_commit_identifier(&next_identifier, file_path),
        Err(YamlConfigurationError::ConfigurationCommitIdentifierExhausted { .. })
    ));
}
