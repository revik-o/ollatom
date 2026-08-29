use infrastructure::Infrastructure;
use std::path::Path;

const APPLICATION_DATABASE_FILE_NAME: &str = "ollatom.sqlite3";

pub async fn initialize(application_data_directory_path: &Path) -> Result<Infrastructure, String> {
    std::fs::create_dir_all(application_data_directory_path).map_err(|error| error.to_string())?;
    Infrastructure::init(application_data_directory_path.join(APPLICATION_DATABASE_FILE_NAME))
        .await
        .map_err(|error| error.to_string())
}
