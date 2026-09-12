use crate::{
    ALL_FILESYSTEM_ACCESS, ALL_USER_COMMANDS, FilesystemAccess, RequiredCapability, RunPolicy,
};

impl RunPolicy {
    pub(crate) fn permits(&self, capability: &RequiredCapability) -> bool {
        match capability {
            RequiredCapability::Filesystem { path, access } => {
                let has_unrestricted_filesystem_access =
                    self.permissions().contains(ALL_FILESYSTEM_ACCESS);

                if has_unrestricted_filesystem_access {
                    return true;
                }

                self.has_trusted_folder_access(path, *access)
            }
            RequiredCapability::Command { program, arguments } => {
                let has_unrestricted_command_access =
                    self.permissions().contains(ALL_USER_COMMANDS);

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
        self.trusted_folders().iter().any(|trusted_folder| {
            crate::policy_matching::folder_permits(trusted_folder, path, access)
        })
    }

    fn has_trusted_command_access(&self, program: &str, arguments: &[String]) -> bool {
        let normalized_command = crate::policy_matching::normalize_command(program, arguments);

        self.trusted_commands()
            .iter()
            .any(|pattern| pattern.matches(&normalized_command))
    }
}
