use infrastructure::{
    Chat, ChatInitializationParameters, Infrastructure, Project, ProjectInitializationParameters,
};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

pub struct TestInfrastructure {
    pub infrastructure: Infrastructure,
    pub temporary_directory: TempDir,
}

pub async fn create_initialized_test_infrastructure() -> TestInfrastructure {
    let temporary_directory = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(
        temporary_directory.path(),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let database_file_path = temporary_directory.path().join("ollatom.sqlite3");
    let infrastructure = Infrastructure::init(database_file_path).await.unwrap();
    TestInfrastructure {
        infrastructure,
        temporary_directory,
    }
}

pub async fn create_project_with_chat(infrastructure: &Infrastructure) -> (Project, Chat) {
    let project_directory_path = infrastructure
        .database_file_path()
        .parent()
        .expect("test database path has a parent directory")
        .join("ollatom-project");
    std::fs::create_dir_all(&project_directory_path).unwrap();
    infrastructure
        .execute_db_actions(async |transaction| {
            let project = transaction
                .create_project(
                    "Ollatom",
                    &project_directory_path,
                    ProjectInitializationParameters::default(),
                )
                .await?;
            let chat = transaction
                .create_chat(
                    "Initial chat",
                    &project,
                    ChatInitializationParameters::default(),
                )
                .await?;
            Ok((project, chat))
        })
        .await
        .unwrap()
}
