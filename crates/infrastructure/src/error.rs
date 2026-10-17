use std::error::Error;
use std::fmt::{Display, Formatter};

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InfrastructureErrorKind {
    InvalidDatabaseFilePath,
    DatabaseParentDirectoryDoesNotExist,
    DatabasePathPointsToDirectory,
    DatabaseConnectionFailed,
    DatabaseConfigurationFailed,
    DatabaseMigrationFailed,
    DatabaseOperationFailed,
    DatabaseConstraintViolation,
    CorruptDatabaseValue,
    EntityValidationFailed,
    InvalidProjectPath,
    InvalidEntityReference,
    InvalidMessageOperation,
    InvalidLlmActionStatusTransition,
    TransactionWasMarkedAsFailed,
    DatabaseActionAndRollbackFailed,
}

#[derive(Debug)]
pub struct InfrastructureError {
    kind: InfrastructureErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl InfrastructureError {
    pub fn kind(&self) -> InfrastructureErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn new(kind: InfrastructureErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn from_database_error(
        kind: InfrastructureErrorKind,
        operation: &str,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            message: format!("{operation}: {source}"),
            source: Some(Box::new(source)),
        }
    }
}

impl Display for InfrastructureError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for InfrastructureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|source| source as &(dyn Error + 'static))
    }
}

pub type InfrastructureResult<ReturnValue> = Result<ReturnValue, InfrastructureError>;
