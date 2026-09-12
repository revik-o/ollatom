use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const PROJECT_DIRECTORY_NAME: &str = "ollama-react-test";
const OWNERSHIP_MARKER_FILE_NAME: &str = "ollama-react-test.benchmark-owned";
const OWNERSHIP_MARKER_CONTENT: &str = "ollama-benchmark-owned-v1\n";

#[derive(Clone, Debug)]
#[must_use]
pub struct BenchmarkArtifacts {
    project_root: PathBuf,
    ownership_marker: PathBuf,
    stage_logs: [PathBuf; 5],
}

impl BenchmarkArtifacts {
    pub fn from_current_executable() -> io::Result<Self> {
        let executable_path = env::current_exe()?;
        let executable_directory = executable_path
            .parent()
            .ok_or_else(|| io::Error::other("executable has no parent"))?
            .to_path_buf();
        Ok(Self::from_directory(executable_directory))
    }

    pub fn from_directory(executable_directory: impl Into<PathBuf>) -> Self {
        let executable_directory = executable_directory.into();
        let project_root = executable_directory.join(PROJECT_DIRECTORY_NAME);
        let ownership_marker = executable_directory.join(OWNERSHIP_MARKER_FILE_NAME);
        let stage_logs = std::array::from_fn(|index| {
            executable_directory.join(format!("stage-{}.log", index + 1))
        });

        Self {
            project_root,
            ownership_marker,
            stage_logs,
        }
    }

    pub fn initialize(&self) -> io::Result<()> {
        for stage_log_path in &self.stage_logs {
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(stage_log_path)?;
        }

        Ok(())
    }

    #[must_use]
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    #[must_use]
    pub fn ownership_marker(&self) -> &Path {
        &self.ownership_marker
    }

    pub fn stage_log(&self, stage_number: u8) -> io::Result<&Path> {
        let stage_index = stage_number
            .checked_sub(1)
            .map(usize::from)
            .ok_or_else(invalid_stage_number_error)?;
        self.stage_logs
            .get(stage_index)
            .map(PathBuf::as_path)
            .ok_or_else(invalid_stage_number_error)
    }

    pub fn prepare_project_for_generation(&self) -> io::Result<()> {
        let ownership_marker_already_exists = match fs::symlink_metadata(&self.project_root) {
            Ok(project_metadata) => {
                Self::require_regular_project_directory(&project_metadata)?;
                self.require_ownership_marker()?;
                fs::remove_dir_all(&self.project_root)?;
                true
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                self.remove_orphaned_ownership_marker()?;
                false
            }
            Err(error) => return Err(error),
        };

        fs::create_dir_all(&self.project_root)?;

        if ownership_marker_already_exists {
            return Ok(());
        }

        if let Err(error) = self.write_ownership_marker() {
            let _ = fs::remove_dir(&self.project_root);
            return Err(error);
        }
        Ok(())
    }

    pub fn require_owned_project(&self) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&self.project_root).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("benchmark project is unavailable: {error}"),
            )
        })?;

        Self::require_regular_project_directory(&metadata)?;
        self.require_ownership_marker()
    }

    pub fn remove_stale_dist(&self) -> io::Result<()> {
        self.require_owned_project()?;
        let dist_path = self.project_root.join("dist");
        let metadata = match fs::symlink_metadata(&dist_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };

        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "refusing to remove a symlinked dist directory",
            ));
        }

        if metadata.is_dir() {
            fs::remove_dir_all(dist_path)
        } else {
            fs::remove_file(dist_path)
        }
    }

    pub fn open_stage_log(&self, stage_number: u8) -> io::Result<File> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.stage_log(stage_number)?)
    }

    fn require_regular_project_directory(metadata: &fs::Metadata) -> io::Result<()> {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "benchmark project path is not a regular directory",
            ));
        }
        Ok(())
    }

    fn remove_orphaned_ownership_marker(&self) -> io::Result<()> {
        match fs::symlink_metadata(&self.ownership_marker) {
            Ok(_) => {
                self.require_ownership_marker()?;
                fs::remove_file(&self.ownership_marker)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn require_ownership_marker(&self) -> io::Result<()> {
        self.require_marker_file()?;
        let marker_content = fs::read_to_string(&self.ownership_marker)?;

        if marker_content != OWNERSHIP_MARKER_CONTENT {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "benchmark ownership marker is invalid",
            ));
        }

        Ok(())
    }

    fn require_marker_file(&self) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&self.ownership_marker).map_err(|error| {
            if error.kind() == ErrorKind::NotFound {
                io::Error::new(
                    ErrorKind::PermissionDenied,
                    "benchmark project is not owned by this benchmark",
                )
            } else {
                error
            }
        })?;

        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "benchmark ownership marker is not a regular file",
            ));
        }

        Ok(())
    }

    fn write_ownership_marker(&self) -> io::Result<()> {
        let temporary_marker = self.temporary_ownership_marker_path();
        let marker_write_result = Self::write_temporary_ownership_marker(&temporary_marker);

        if let Err(error) = marker_write_result {
            let _ = fs::remove_file(&temporary_marker);
            return Err(error);
        }

        if let Err(error) = fs::rename(&temporary_marker, &self.ownership_marker) {
            let _ = fs::remove_file(&temporary_marker);
            return Err(error);
        }
        Ok(())
    }

    fn write_temporary_ownership_marker(temporary_marker: &Path) -> io::Result<()> {
        let mut marker_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(temporary_marker)?;
        marker_file.write_all(OWNERSHIP_MARKER_CONTENT.as_bytes())?;
        marker_file.sync_all()
    }

    fn temporary_ownership_marker_path(&self) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        self.ownership_marker.with_extension(format!(
            "benchmark-owned.{}.{timestamp}.tmp",
            std::process::id()
        ))
    }
}

fn invalid_stage_number_error() -> io::Error {
    io::Error::new(
        ErrorKind::InvalidInput,
        "stage must be between one and five",
    )
}
