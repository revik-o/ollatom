use std::{collections::BTreeSet, path::PathBuf, time::Duration};

#[derive(Clone, Debug)]
#[must_use]
pub struct BasicToolConfiguration {
    pub(super) root_directory: PathBuf,
    pub(super) maximum_file_size: u64,
    pub(super) command_timeout: Duration,
    pub(super) excluded_directory_names: BTreeSet<String>,
}

impl BasicToolConfiguration {
    pub fn new(root_directory: impl Into<PathBuf>) -> Self {
        Self {
            root_directory: root_directory.into(),
            maximum_file_size: 4 * 1024 * 1024,
            command_timeout: Duration::from_mins(15),
            excluded_directory_names: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn root_directory(&self) -> &std::path::Path {
        &self.root_directory
    }

    pub fn maximum_file_size(mut self, maximum_file_size: u64) -> Self {
        self.maximum_file_size = maximum_file_size;
        self
    }

    pub fn command_timeout(mut self, command_timeout: Duration) -> Self {
        self.command_timeout = command_timeout;
        self
    }

    pub fn exclude_directory(mut self, directory_name: impl Into<String>) -> Self {
        self.excluded_directory_names.insert(directory_name.into());
        self
    }

    pub(super) fn excluded_directory_names(&self) -> &BTreeSet<String> {
        &self.excluded_directory_names
    }

    pub(super) fn validate(&self) -> Result<(), crate::LlmError> {
        if !self.root_directory.is_absolute() {
            return Err(crate::LlmError::InvalidRequest(
                "basic tool root must be an absolute path".into(),
            ));
        }

        if self.maximum_file_size == 0 {
            return Err(crate::LlmError::InvalidRequest(
                "basic tool maximum file size must be greater than zero".into(),
            ));
        }

        if self.command_timeout.is_zero() {
            return Err(crate::LlmError::InvalidRequest(
                "basic tool command timeout must be greater than zero".into(),
            ));
        }

        if self
            .excluded_directory_names
            .iter()
            .any(|directory_name| !is_directory_name(directory_name))
        {
            return Err(crate::LlmError::InvalidRequest(
                "excluded directories must be non-empty names without path separators".into(),
            ));
        }

        Ok(())
    }
}

fn is_directory_name(directory_name: &str) -> bool {
    let directory_path = std::path::Path::new(directory_name);
    let mut components = directory_path.components();

    matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none()
}
