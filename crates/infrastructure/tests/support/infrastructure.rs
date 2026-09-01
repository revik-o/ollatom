use infrastructure::{
    Chat, ChatInitializationParameters, Infrastructure, Project, ProjectInitializationParameters,
};
use tempfile::TempDir;

pub struct TestInfrastructure {
    pub infrastructure: Infrastructure,
    pub temporary_directory: TempDir,
}

pub async fn create_initialized_test_infrastructure() -> TestInfrastructure {
    let temporary_directory = tempfile::tempdir().unwrap();
    let database_file_path = temporary_directory.path().join("ollatom.sqlite3");
    let infrastructure = Infrastructure::init(database_file_path).await.unwrap();
    TestInfrastructure {
        infrastructure,
        temporary_directory,
    }
}

pub async fn create_project_with_chat(infrastructure: &Infrastructure) -> (Project, Chat) {
    infrastructure
        .execute_db_actions(async |transaction| {
            let project = transaction
                .create_project(
                    "Ollatom",
                    "/projects/ollatom",
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
