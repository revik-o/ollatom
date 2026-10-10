use super::columns::read_column;
use super::settings::map_execution_settings;
use crate::database::{parse_timestamp, parse_uuid};
use crate::{Chat, ChatId, InfrastructureResult, ProjectId};
use sqlx::sqlite::SqliteRow;

pub(crate) fn map_chat_row(database_row: &SqliteRow) -> InfrastructureResult<Chat> {
    Ok(Chat {
        id: ChatId::from(parse_uuid(read_column(database_row, "id")?)?),
        project_id: ProjectId::from(parse_uuid(read_column(database_row, "project_id")?)?),
        name: read_column(database_row, "name")?,
        execution_settings: map_execution_settings(database_row)?,
        created_at: parse_timestamp(read_column::<&str>(database_row, "created_at")?)?,
        updated_at: parse_timestamp(read_column::<&str>(database_row, "updated_at")?)?,
    })
}
