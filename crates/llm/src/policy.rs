use crate::{ApprovalDecision, ApprovalRequest, BoxFuture, LlmError};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AllowedPermissions(u8);
impl AllowedPermissions {
    pub const NONE: Self = Self(0);
    pub const ALL_FILESYSTEM_ACCESS: Self = Self(1);
    pub const ALL_USER_COMMANDS: Self = Self(2);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for AllowedPermissions {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for AllowedPermissions {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

pub const ALL_FILESYSTEM_ACCESS: AllowedPermissions = AllowedPermissions::ALL_FILESYSTEM_ACCESS;
pub const ALL_USER_COMMANDS: AllowedPermissions = AllowedPermissions::ALL_USER_COMMANDS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilesystemAccess {
    Read,
    Create,
    Modify,
    Rename,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedFolder {
    pub path: PathBuf,
    pub allow_delete: bool,
}

impl TrustedFolder {
    pub fn standard(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            allow_delete: false,
        }
    }

    pub fn full_access(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            allow_delete: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CommandPattern {
    source: String,
    anchored: Regex,
}

impl CommandPattern {
    pub fn new(source: impl Into<String>) -> Result<Self, LlmError> {
        let source = source.into();
        let anchored = Regex::new(&format!(r"\A(?:{source})\z")).map_err(|error| {
            LlmError::InvalidRequest(format!("invalid trusted command regex: {error}"))
        })?;

        Ok(Self { source, anchored })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn matches(&self, command: &str) -> bool {
        self.anchored.is_match(command)
    }

    pub fn matches_command(&self, program: &str, arguments: &[String]) -> bool {
        self.matches(&crate::policy_matching::normalize_command(
            program, arguments,
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequiredCapability {
    Filesystem {
        path: PathBuf,
        access: FilesystemAccess,
    },
    Command {
        program: String,
        arguments: Vec<String>,
    },
    Network {
        host: String,
    },
    UserInteraction,
    InvokeSubagent {
        profile: String,
    },
}

pub type ApprovalHandler =
    Arc<dyn Fn(ApprovalRequest) -> BoxFuture<'static, ApprovalDecision> + Send + Sync>;

pub trait ToolAuthorizer: Send + Sync {
    fn authorize(
        &self,
        request: ApprovalRequest,
    ) -> BoxFuture<'_, Result<ApprovalDecision, LlmError>>;
}

#[must_use]
#[derive(Clone, Default)]
pub struct RunPolicy {
    trusted_folders: Vec<TrustedFolder>,
    trusted_commands: Vec<CommandPattern>,
    permissions: AllowedPermissions,
    deny_untrusted: bool,
    stream_approvals: bool,
    approval: Option<ApprovalHandler>,
}

impl RunPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trust_folder(mut self, folder: TrustedFolder) -> Self {
        self.add_trusted_folder(folder);
        self
    }

    pub fn trust_command(mut self, pattern: impl Into<String>) -> Result<Self, LlmError> {
        self.add_trusted_command(pattern)?;
        Ok(self)
    }

    pub fn allow(mut self, permissions: AllowedPermissions) -> Self {
        self.allow_permissions(permissions);
        self
    }

    pub fn deny_untrusted(mut self) -> Self {
        self.set_deny_untrusted();
        self
    }

    pub fn stream_approval_requests(mut self) -> Self {
        self.set_stream_approvals();
        self
    }

    pub fn approval_handler(mut self, handler: ApprovalHandler) -> Self {
        self.set_approval_handler(handler);
        self
    }

    pub fn trusted_folders(&self) -> &[TrustedFolder] {
        &self.trusted_folders
    }

    pub fn trusted_commands(&self) -> &[CommandPattern] {
        &self.trusted_commands
    }

    pub fn permissions(&self) -> AllowedPermissions {
        self.permissions
    }

    pub fn denies_untrusted(&self) -> bool {
        self.deny_untrusted
    }

    pub fn streams_approvals(&self) -> bool {
        self.stream_approvals
    }

    pub(crate) fn approval(&self) -> Option<&ApprovalHandler> {
        self.approval.as_ref()
    }

    pub(crate) fn add_trusted_folder(&mut self, folder: TrustedFolder) {
        self.trusted_folders.push(folder);
    }

    pub(crate) fn add_trusted_command(
        &mut self,
        pattern: impl Into<String>,
    ) -> Result<(), LlmError> {
        self.trusted_commands.push(CommandPattern::new(pattern)?);

        Ok(())
    }
    pub(crate) fn allow_permissions(&mut self, permissions: AllowedPermissions) {
        self.permissions |= permissions;
    }

    pub(crate) fn set_deny_untrusted(&mut self) {
        self.deny_untrusted = true;
    }

    pub(crate) fn set_stream_approvals(&mut self) {
        self.stream_approvals = true;
    }

    pub(crate) fn set_approval_handler(&mut self, handler: ApprovalHandler) {
        self.approval = Some(handler);
    }

    pub(crate) fn permits(&self, capability: &RequiredCapability) -> bool {
        match capability {
            RequiredCapability::Filesystem { path, access } => {
                let has_unrestricted_filesystem_access =
                    self.permissions.contains(ALL_FILESYSTEM_ACCESS);

                if has_unrestricted_filesystem_access {
                    return true;
                }

                self.has_trusted_folder_access(path, *access)
            }

            RequiredCapability::Command { program, arguments } => {
                let has_unrestricted_command_access = self.permissions.contains(ALL_USER_COMMANDS);
                if has_unrestricted_command_access {
                    return true;
                }

                self.has_trusted_command_access(program, arguments)
            }
            RequiredCapability::UserInteraction => true,
            RequiredCapability::Network { .. } | RequiredCapability::InvokeSubagent { .. } => false,
        }
    }

    fn has_trusted_folder_access(&self, path: &std::path::Path, access: FilesystemAccess) -> bool {
        self.trusted_folders.iter().any(|trusted_folder| {
            crate::policy_matching::folder_permits(trusted_folder, path, access)
        })
    }

    fn has_trusted_command_access(&self, program: &str, arguments: &[String]) -> bool {
        let normalized_command = crate::policy_matching::normalize_command(program, arguments);

        self.trusted_commands
            .iter()
            .any(|pattern| pattern.matches(&normalized_command))
    }
}
