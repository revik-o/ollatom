use super::super::super::YamlConfigurationError;
use super::super::ConfigurationCommitIdentifier;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONFIGURATION_COMMIT_IDENTIFIER: AtomicU64 = AtomicU64::new(1);

impl ConfigurationCommitIdentifier {
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}
pub(crate) fn next_configuration_commit_identifier(
    file_path: &Path,
) -> Result<ConfigurationCommitIdentifier, YamlConfigurationError> {
    allocate_configuration_commit_identifier(&NEXT_CONFIGURATION_COMMIT_IDENTIFIER, file_path)
}

pub(crate) fn allocate_configuration_commit_identifier(
    next_identifier: &AtomicU64,
    file_path: &Path,
) -> Result<ConfigurationCommitIdentifier, YamlConfigurationError> {
    next_identifier
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current_identifier| {
            if current_identifier == 0 {
                None
            } else if current_identifier == u64::MAX {
                Some(0)
            } else {
                current_identifier.checked_add(1)
            }
        })
        .map(ConfigurationCommitIdentifier)
        .map_err(|_| YamlConfigurationError::ConfigurationCommitIdentifierExhausted {
            path: file_path.to_owned(),
        })
}
