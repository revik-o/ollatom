use super::{CommandError, SandboxNetworkPolicy};
use rappct::{
    AppContainerProfile, KnownCapability, SecurityCapabilitiesBuilder,
    acl::{AccessMask, ResourcePath, grant_to_package},
};
use std::os::windows::process::CommandExt;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use uuid::Uuid;

#[path = "command_windows/environment.rs"]
pub(crate) mod environment;
#[path = "command_windows/monitor.rs"]
mod monitor;

pub(super) use monitor::monitor_command;

const MAXIMUM_COMBINED_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const FILE_EXECUTE_ACCESS: u32 = 0x0000_0020;
const FILE_DELETE_CHILD_ACCESS: u32 = 0x0000_0040;
const DELETE_ACCESS: u32 = 0x0001_0000;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(super) fn ensure_sandbox_backend_available() -> Result<(), CommandError> {
    let system_root = environment::system_root_directory()?;
    let executable_path = system_root.join("System32").join("icacls.exe");

    let executable_metadata = std::fs::symlink_metadata(&executable_path)
        .map_err(|error| CommandError::SandboxUnavailable(error.to_string()))?;

    if executable_metadata.file_type().is_symlink() || !executable_metadata.is_file() {
        return Err(CommandError::SandboxUnavailable(
            "the Windows ACL utility is unavailable".into(),
        ));
    }

    Ok(())
}

struct WorkspacePackageAccess {
    workspace_directory: PathBuf,
    additional_read_only_directories: Vec<PathBuf>,
    package_profile: Option<AppContainerProfile>,
    package_security_identifier: String,
    access_granted: bool,
}

impl WorkspacePackageAccess {
    fn grant(
        workspace_directory: &Path,
        additional_read_only_directories: &[PathBuf],
        network_policy: SandboxNetworkPolicy,
    ) -> Result<(Self, rappct::SecurityCapabilities), CommandError> {
        let unique_profile_name = format!("ollatom.command.{}.{}", std::process::id(), Uuid::new_v4());

        let package_profile = AppContainerProfile::ensure(
            &unique_profile_name,
            "Ollatom command",
            Some("Temporary command sandbox"),
        )
        .map_err(|error| CommandError::SandboxUnavailable(error.to_string()))?;

        let package_security_identifier = package_profile.sid.as_string().to_owned();

        let mut workspace_access = Self {
            workspace_directory: workspace_directory.to_path_buf(),
            additional_read_only_directories: Vec::new(),
            package_profile: Some(package_profile),
            package_security_identifier,
            access_granted: false,
        };

        let package_profile = workspace_access
            .package_profile
            .as_ref()
            .ok_or_else(|| CommandError::SandboxUnavailable("profile was unavailable".into()))?;

        let capability_builder = SecurityCapabilitiesBuilder::new(&package_profile.sid);

        let capability_builder = match network_policy {
            SandboxNetworkPolicy::Denied => capability_builder,
            SandboxNetworkPolicy::Allowed => capability_builder.with_known(&[KnownCapability::InternetClient]),
        };

        let security_capabilities = capability_builder
            .build()
            .map_err(|error| CommandError::SandboxUnavailable(error.to_string()))?;

        workspace_access.access_granted = true;

        grant_to_package(
            ResourcePath::Directory(workspace_access.workspace_directory.clone()),
            &package_profile.sid,
            AccessMask(
                AccessMask::FILE_GENERIC_READ.0
                    | AccessMask::FILE_GENERIC_WRITE.0
                    | FILE_EXECUTE_ACCESS
                    | FILE_DELETE_CHILD_ACCESS
                    | DELETE_ACCESS,
            ),
        )
        .map_err(|error| CommandError::SandboxUnavailable(error.to_string()))?;

        for directory in additional_read_only_directories {
            grant_to_package(
                ResourcePath::Directory(directory.clone()),
                &package_profile.sid,
                AccessMask(AccessMask::FILE_GENERIC_READ.0 | FILE_EXECUTE_ACCESS),
            )
            .map_err(|error| CommandError::SandboxUnavailable(error.to_string()))?;

            workspace_access
                .additional_read_only_directories
                .push(directory.clone());
        }

        Ok((workspace_access, security_capabilities))
    }

    fn revoke(&mut self) -> Result<(), CommandError> {
        for directory in self.additional_read_only_directories.drain(..).rev() {
            remove_package_access(&directory, &self.package_security_identifier)?;
        }

        if self.access_granted {
            remove_package_access(&self.workspace_directory, &self.package_security_identifier)?;
            self.access_granted = false;
        }

        if let Some(package_profile) = self.package_profile.take() {
            package_profile
                .delete()
                .map_err(|error| CommandError::Termination(error.to_string()))?;
        }

        Ok(())
    }
}

impl Drop for WorkspacePackageAccess {
    fn drop(&mut self) {
        for directory in self.additional_read_only_directories.drain(..).rev() {
            let _ = remove_package_access(&directory, &self.package_security_identifier);
        }

        if self.access_granted {
            let _ = remove_package_access(&self.workspace_directory, &self.package_security_identifier);
            self.access_granted = false;
        }

        if let Some(package_profile) = self.package_profile.take() {
            let _ = package_profile.delete();
        }
    }
}
fn remove_package_access(workspace_directory: &Path, package_security_identifier: &str) -> Result<(), CommandError> {
    let system_root = environment::system_root_directory()?;
    let acl_utility_path = system_root.join("System32").join("icacls.exe");

    let mut acl_utility_command = Command::new(&acl_utility_path);

    acl_utility_command
        .arg(workspace_directory)
        .arg("/remove:g")
        .arg(format!("*{package_security_identifier}"))
        .env_clear()
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    acl_utility_command.env("SystemRoot", system_root);

    let acl_utility_output = acl_utility_command
        .output()
        .map_err(|error| CommandError::Termination(error.to_string()))?;

    if !acl_utility_output.status.success() {
        return Err(CommandError::Termination(format!(
            "icacls exited with {}: {}",
            acl_utility_output.status,
            String::from_utf8_lossy(&acl_utility_output.stderr)
        )));
    }

    Ok(())
}
