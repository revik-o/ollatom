use super::InfrastructureTransaction;
use super::store::{
    create_project, delete_project_by_id, get_project_by_id, get_project_by_name,
    get_project_by_path, update_project,
};
use crate::{
    InfrastructureResult, Project, ProjectId, ProjectInitializationParameters, ProjectUpdateOptions,
};

impl InfrastructureTransaction {
    pub async fn create_project(
        &mut self,
        project_name: impl Into<String>,
        project_path: impl Into<String>,
        initialization_parameters: ProjectInitializationParameters,
    ) -> InfrastructureResult<Project> {
        let operation_result = create_project(
            self.database_transaction_mut()?,
            project_name.into(),
            project_path.into(),
            initialization_parameters,
        )
        .await;
        self.record_operation_result(operation_result)
    }

    pub async fn get_project_by_id(
        &mut self,
        project_id: impl Into<ProjectId>,
    ) -> InfrastructureResult<Option<Project>> {
        let operation_result =
            get_project_by_id(self.database_transaction_mut()?, project_id.into()).await;
        self.record_operation_result(operation_result)
    }

    pub async fn get_project_by_name(
        &mut self,
        project_name: &str,
    ) -> InfrastructureResult<Option<Project>> {
        let operation_result =
            get_project_by_name(self.database_transaction_mut()?, project_name).await;
        self.record_operation_result(operation_result)
    }

    pub async fn get_project_by_path(
        &mut self,
        project_path: &str,
    ) -> InfrastructureResult<Option<Project>> {
        let operation_result =
            get_project_by_path(self.database_transaction_mut()?, project_path).await;
        self.record_operation_result(operation_result)
    }

    pub async fn update_project(
        &mut self,
        update_options: ProjectUpdateOptions,
    ) -> InfrastructureResult<Option<Project>> {
        let operation_result =
            update_project(self.database_transaction_mut()?, update_options).await;
        self.record_operation_result(operation_result)
    }

    pub async fn delete_project(
        &mut self,
        project: &Project,
    ) -> InfrastructureResult<Option<Project>> {
        self.delete_project_by_id(project.id).await
    }

    pub async fn delete_project_by_id(
        &mut self,
        project_id: impl Into<ProjectId>,
    ) -> InfrastructureResult<Option<Project>> {
        let operation_result =
            delete_project_by_id(self.database_transaction_mut()?, project_id.into()).await;
        self.record_operation_result(operation_result)
    }

    pub async fn set_llm_thinking_for_project(
        &mut self,
        is_enabled: bool,
        project: &Project,
    ) -> InfrastructureResult<bool> {
        self.set_llm_thinking_for_project_by_id(is_enabled, project.id)
            .await
    }

    pub async fn set_llm_thinking_for_project_by_id(
        &mut self,
        is_enabled: bool,
        project_id: impl Into<ProjectId>,
    ) -> InfrastructureResult<bool> {
        self.set_boolean_project_value("llm_thinking_enabled", is_enabled, project_id.into())
            .await
    }

    pub async fn set_llm_context_optimization_for_project(
        &mut self,
        is_enabled: bool,
        project: &Project,
    ) -> InfrastructureResult<bool> {
        self.set_llm_context_optimization_for_project_by_id(is_enabled, project.id)
            .await
    }

    pub async fn set_llm_context_optimization_for_project_by_id(
        &mut self,
        is_enabled: bool,
        project_id: impl Into<ProjectId>,
    ) -> InfrastructureResult<bool> {
        self.set_boolean_project_value(
            "llm_context_optimization_enabled",
            is_enabled,
            project_id.into(),
        )
        .await
    }

    pub async fn set_cpu_usage_for_project(
        &mut self,
        cpu_usage_percentage: u8,
        project: &Project,
    ) -> InfrastructureResult<bool> {
        self.set_cpu_usage_for_project_by_id(cpu_usage_percentage, project.id)
            .await
    }

    pub async fn set_cpu_usage_for_project_by_id(
        &mut self,
        cpu_usage_percentage: u8,
        project_id: impl Into<ProjectId>,
    ) -> InfrastructureResult<bool> {
        self.set_usage_project_value(
            "cpu_usage_percentage",
            cpu_usage_percentage,
            project_id.into(),
        )
        .await
    }

    pub async fn set_gpu_usage_for_project(
        &mut self,
        gpu_usage_percentage: u8,
        project: &Project,
    ) -> InfrastructureResult<bool> {
        self.set_gpu_usage_for_project_by_id(gpu_usage_percentage, project.id)
            .await
    }

    pub async fn set_gpu_usage_for_project_by_id(
        &mut self,
        gpu_usage_percentage: u8,
        project_id: impl Into<ProjectId>,
    ) -> InfrastructureResult<bool> {
        self.set_usage_project_value(
            "gpu_usage_percentage",
            gpu_usage_percentage,
            project_id.into(),
        )
        .await
    }
}
