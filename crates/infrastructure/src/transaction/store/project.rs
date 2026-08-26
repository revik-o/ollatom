use super::super::PROJECT_COLUMNS;
use crate::database::{
    database_operation_error, format_timestamp, validate_nonblank_value, validate_usage_percentage,
};
use crate::mapping::map_project_row;
use crate::{
    InfrastructureResult, Project, ProjectId, ProjectInitializationParameters, ProjectUpdateOptions,
};
use sqlx::{AssertSqlSafe, Sqlite, Transaction};
use time::OffsetDateTime;

pub(crate) async fn create_project(
    database_transaction: &mut Transaction<'static, Sqlite>,
    project_name: String,
    project_path: String,
    initialization_parameters: ProjectInitializationParameters,
) -> InfrastructureResult<Project> {
    validate_nonblank_value("project name", &project_name)?;
    validate_nonblank_value("project path", &project_path)?;
    validate_usage_percentage(
        "project CPU usage percentage",
        initialization_parameters.cpu_usage_percentage,
    )?;
    validate_usage_percentage(
        "project GPU usage percentage",
        initialization_parameters.gpu_usage_percentage,
    )?;

    let project_id = ProjectId::new();
    let created_at = OffsetDateTime::now_utc();
    let created_at_text = format_timestamp(created_at)?;
    sqlx::query(
        "INSERT INTO projects (id, name, path, llm_thinking_enabled, llm_context_optimization_enabled, cpu_usage_percentage, gpu_usage_percentage, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(project_id.as_bytes().to_vec())
    .bind(&project_name)
    .bind(&project_path)
    .bind(initialization_parameters.llm_thinking_enabled)
    .bind(initialization_parameters.llm_context_optimization_enabled)
    .bind(initialization_parameters.cpu_usage_percentage as i64)
    .bind(initialization_parameters.gpu_usage_percentage as i64)
    .bind(&created_at_text)
    .bind(&created_at_text)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to create project", source))?;

    Ok(Project {
        id: project_id,
        name: project_name,
        path: project_path,
        llm_thinking_enabled: initialization_parameters.llm_thinking_enabled,
        llm_context_optimization_enabled: initialization_parameters
            .llm_context_optimization_enabled,
        cpu_usage_percentage: initialization_parameters.cpu_usage_percentage,
        gpu_usage_percentage: initialization_parameters.gpu_usage_percentage,
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
    project_path: &str,
) -> InfrastructureResult<Option<Project>> {
    let query_text = format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE path = ?");
    let database_row = sqlx::query(AssertSqlSafe(query_text))
        .bind(project_path)
        .fetch_optional(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to get project by path", source))?;
    database_row.as_ref().map(map_project_row).transpose()
}

pub(crate) async fn update_project(
    database_transaction: &mut Transaction<'static, Sqlite>,
    update_options: ProjectUpdateOptions,
) -> InfrastructureResult<Option<Project>> {
    let Some(existing_project) =
        get_project_by_id(database_transaction, update_options.project_id).await?
    else {
        return Ok(None);
    };
    if !update_options.contains_changes() {
        return Ok(Some(existing_project));
    }

    let project_name = update_options.name.unwrap_or(existing_project.name);
    let project_path = update_options.path.unwrap_or(existing_project.path);
    let llm_thinking_enabled = update_options
        .llm_thinking_enabled
        .unwrap_or(existing_project.llm_thinking_enabled);
    let llm_context_optimization_enabled = update_options
        .llm_context_optimization_enabled
        .unwrap_or(existing_project.llm_context_optimization_enabled);
    let cpu_usage_percentage = update_options
        .cpu_usage_percentage
        .unwrap_or(existing_project.cpu_usage_percentage);
    let gpu_usage_percentage = update_options
        .gpu_usage_percentage
        .unwrap_or(existing_project.gpu_usage_percentage);
    validate_nonblank_value("project name", &project_name)?;
    validate_nonblank_value("project path", &project_path)?;
    validate_usage_percentage("project CPU usage percentage", cpu_usage_percentage)?;
    validate_usage_percentage("project GPU usage percentage", gpu_usage_percentage)?;
    let updated_at = OffsetDateTime::now_utc();
    let updated_at_text = format_timestamp(updated_at)?;

    sqlx::query(
        "UPDATE projects SET name = ?, path = ?, llm_thinking_enabled = ?, llm_context_optimization_enabled = ?, cpu_usage_percentage = ?, gpu_usage_percentage = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&project_name)
    .bind(&project_path)
    .bind(llm_thinking_enabled)
    .bind(llm_context_optimization_enabled)
    .bind(cpu_usage_percentage as i64)
    .bind(gpu_usage_percentage as i64)
    .bind(&updated_at_text)
    .bind(update_options.project_id.as_bytes().to_vec())
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to update project", source))?;

    Ok(Some(Project {
        id: update_options.project_id,
        name: project_name,
        path: project_path,
        llm_thinking_enabled,
        llm_context_optimization_enabled,
        cpu_usage_percentage,
        gpu_usage_percentage,
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
