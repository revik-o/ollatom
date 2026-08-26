use std::fmt::{Display, Formatter};

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
    EntityValidationFailed,
    InvalidEntityReference,
    InvalidMessageOperation,
    InvalidLlmActionStatusTransition,
    TransactionWasMarkedAsFailed,
    DatabaseActionAndRollbackFailed,
    InvalidSqlBuilderOperation,
}

#[derive(Debug)]
pub struct InfrastructureError {
    kind: InfrastructureErrorKind,
    message: String,
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
        }
    }

    pub(crate) fn from_database_error(
        kind: InfrastructureErrorKind,
        operation: &str,
        source: impl Display,
    ) -> Self {
        Self::new(kind, format!("{operation}: {source}"))
    }
}

impl Display for InfrastructureError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for InfrastructureError {}

pub type InfrastructureResult<ReturnValue> = Result<ReturnValue, InfrastructureError>;
