use super::super::PROJECT_COLUMNS;
use crate::database::{database_operation_error, format_timestamp, validate_nonblank_value, validate_usage_percentage};
use crate::mapping::map_project_row;
use crate::{InfrastructureResult, Project, ProjectId, ProjectInitializationParameters, ProjectUpdateOptions};
use sqlx::{AssertSqlSafe, Sqlite, Transaction};
use std::path::Path;
use time::OffsetDateTime;

mod path;

pub(crate) async fn create_project(
    database_transaction: &mut Transaction<'static, Sqlite>,
    project_name: String,
    project_path: &Path,
    initialization_parameters: ProjectInitializationParameters,
) -> InfrastructureResult<Project> {
    validate_nonblank_value("project name", &project_name)?;

    validate_usage_percentage(
        "project CPU usage percentage",
        initialization_parameters.cpu_usage_percentage,
    )?;

    validate_usage_percentage(
        "project GPU usage percentage",
        initialization_parameters.gpu_usage_percentage,
    )?;

    let (canonical_directory_path, canonical_path_text, filesystem_identity) =
        path::resolve_project_directory(project_path).await?;
    let (identity_kind, volume_identifier, object_identifier) = path::filesystem_identity_columns(&filesystem_identity);

    let project_id = ProjectId::new();
    let created_at = OffsetDateTime::now_utc();
    let created_at_text = format_timestamp(created_at)?;

    sqlx::query(
        "INSERT INTO projects (id, name, canonical_directory_path, filesystem_identity_kind, filesystem_volume_identifier, filesystem_object_identifier, llm_thinking_enabled, llm_context_optimization_enabled, cpu_usage_percentage, gpu_usage_percentage, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(project_id.as_bytes().to_vec())
    .bind(&project_name)
    .bind(&canonical_path_text)
    .bind(identity_kind)
    .bind(volume_identifier)
    .bind(object_identifier)
    .bind(initialization_parameters.large_language_model_thinking_enabled)
    .bind(initialization_parameters.context_optimization_enabled)
    .bind(i64::from(initialization_parameters.cpu_usage_percentage))
    .bind(i64::from(initialization_parameters.gpu_usage_percentage))
    .bind(&created_at_text)
    .bind(&created_at_text)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to create project", source))?;

    Ok(Project {
        id: project_id,
        name: project_name,
        canonical_directory_path,
        filesystem_identity,
        execution_settings: initialization_parameters,
        created_at,
        updated_at: created_at,
    })
}

pub(crate) async fn get_project_by_id(
    database_transaction: &mut Transaction<'static, Sqlite>,
    project_id: ProjectId,
) -> InfrastructureResult<Option<Project>> {
    let query_text = format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE id = ?");
    let database_row = sqlx::query(AssertSqlSafe(query_text))
        .bind(project_id.as_bytes().to_vec())
        .fetch_optional(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to get project by ID", source))?;
    database_row.as_ref().map(map_project_row).transpose()
}

pub(crate) async fn get_project_by_name(
    database_transaction: &mut Transaction<'static, Sqlite>,
    project_name: &str,
) -> InfrastructureResult<Option<Project>> {
    let query_text = format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE name = ?");
    let database_row = sqlx::query(AssertSqlSafe(query_text))
        .bind(project_name)
        .fetch_optional(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to get project by name", source))?;

    database_row.as_ref().map(map_project_row).transpose()
}

pub(crate) async fn get_project_by_path(
    database_transaction: &mut Transaction<'static, Sqlite>,
    project_path: &Path,
) -> InfrastructureResult<Option<Project>> {
    let (_, _, filesystem_identity) = path::resolve_project_directory(project_path).await?;
    let (identity_kind, volume_identifier, object_identifier) = path::filesystem_identity_columns(&filesystem_identity);
    let query_text = format!(
        "SELECT {PROJECT_COLUMNS} FROM projects WHERE filesystem_identity_kind = ? AND filesystem_volume_identifier = ? AND filesystem_object_identifier = ?"
    );
    let database_row = sqlx::query(AssertSqlSafe(query_text))
        .bind(identity_kind)
        .bind(volume_identifier)
        .bind(object_identifier)
        .fetch_optional(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to get project by path", source))?;

    database_row.as_ref().map(map_project_row).transpose()
}

pub(crate) async fn update_project(
    database_transaction: &mut Transaction<'static, Sqlite>,
    update_options: ProjectUpdateOptions,
) -> InfrastructureResult<Option<Project>> {
    let Some(existing_project) = get_project_by_id(database_transaction, update_options.project_id).await? else {
        return Ok(None);
    };

    if !update_options.contains_changes() {
        return Ok(Some(existing_project));
    }

    let project_name = update_options.name.unwrap_or(existing_project.name);

    let (canonical_directory_path, canonical_path_text) =
        if let Some(requested_path) = update_options.canonical_directory_path {
            let (canonical_directory_path, canonical_path_text, resolved_identity) =
                path::resolve_project_directory(&requested_path).await?;

            if resolved_identity != existing_project.filesystem_identity {
                return Err(path::invalid_project_path(
                    &requested_path,
                    "an existing project cannot be retargeted to a different directory",
                ));
            }

            (canonical_directory_path, canonical_path_text)
        } else {
            (
                existing_project.canonical_directory_path.clone(),
                existing_project
                    .canonical_directory_path
                    .to_str()
                    .expect("stored canonical project paths are valid UTF-8")
                    .to_owned(),
            )
        };

    let execution_settings = update_options
        .execution_settings
        .apply_to(&existing_project.execution_settings);

    validate_nonblank_value("project name", &project_name)?;

    validate_usage_percentage("project CPU usage percentage", execution_settings.cpu_usage_percentage)?;

    validate_usage_percentage("project GPU usage percentage", execution_settings.gpu_usage_percentage)?;

    let updated_at = OffsetDateTime::now_utc();
    let updated_at_text = format_timestamp(updated_at)?;

    sqlx::query(
        "UPDATE projects SET name = ?, canonical_directory_path = ?, llm_thinking_enabled = ?, llm_context_optimization_enabled = ?, cpu_usage_percentage = ?, gpu_usage_percentage = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&project_name)
    .bind(&canonical_path_text)
    .bind(execution_settings.large_language_model_thinking_enabled)
    .bind(execution_settings.context_optimization_enabled)
    .bind(i64::from(execution_settings.cpu_usage_percentage))
    .bind(i64::from(execution_settings.gpu_usage_percentage))
    .bind(&updated_at_text)
    .bind(update_options.project_id.as_bytes().to_vec())
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to update project", source))?;

    Ok(Some(Project {
        id: update_options.project_id,
        name: project_name,
        canonical_directory_path,
        filesystem_identity: existing_project.filesystem_identity,
        execution_settings,
        created_at: existing_project.created_at,
        updated_at,
    }))
}

pub(crate) async fn delete_project_by_id(
    database_transaction: &mut Transaction<'static, Sqlite>,
    project_id: ProjectId,
) -> InfrastructureResult<Option<Project>> {
    let Some(project) = get_project_by_id(database_transaction, project_id).await? else {
        return Ok(None);
    };

    sqlx::query("DELETE FROM projects WHERE id = ?")
        .bind(project_id.as_bytes().to_vec())
        .execute(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to delete project", source))?;

    Ok(Some(project))
}
