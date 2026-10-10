use crate::yaml_configuration::YamlConfigurationError;
use crate::yaml_configuration::store::commit::MAX_UNACKNOWLEDGED_CONFIGURATION_COMMITS;
use crate::yaml_configuration::store::*;
use std::path::Path;

#[test]
fn commit_registry_keeps_pending_results_and_releases_acknowledged_results() {
    let registry = ConfigurationCommitRegistry::default();
    let file_path = Path::new("settings.yaml");
    let (first_identifier, first_sender, _first_receiver) = registry.register(file_path).unwrap();
    for _ in 1..MAX_UNACKNOWLEDGED_CONFIGURATION_COMMITS {
        registry.register(file_path).unwrap();
    }

    assert!(matches!(
        registry.register(file_path),
        Err(YamlConfigurationError::ConfigurationCommitRegistryFull { .. })
    ));
    assert_eq!(
        registry.acknowledge(first_identifier, file_path).unwrap(),
        Some(ConfigurationCommitStatus::Pending)
    );
    assert!(matches!(
        registry.register(file_path),
        Err(YamlConfigurationError::ConfigurationCommitRegistryFull { .. })
    ));

    first_sender.send_replace(ConfigurationCommitStatus::Succeeded);
    assert_eq!(
        registry.acknowledge(first_identifier, file_path).unwrap(),
        Some(ConfigurationCommitStatus::Succeeded)
    );
    assert!(registry.register(file_path).is_ok());
}
