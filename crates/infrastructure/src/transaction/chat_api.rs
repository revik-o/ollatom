use super::InfrastructureTransaction;
use super::store::{create_chat, delete_chat_by_id, update_chat};
use crate::{
    Chat, ChatId, ChatInitializationParameters, ChatUpdateOptions, InfrastructureResult, Project,
    ProjectId,
};

impl InfrastructureTransaction {
    pub async fn create_chat(
        &mut self,
        chat_name: impl Into<String>,
        project: &Project,
        initialization_parameters: ChatInitializationParameters,
    ) -> InfrastructureResult<Chat> {
        self.create_chat_by_project_id(chat_name, project.id, initialization_parameters)
            .await
    }

    pub async fn create_chat_by_project_id(
        &mut self,
        chat_name: impl Into<String>,
        project_id: impl Into<ProjectId>,
        initialization_parameters: ChatInitializationParameters,
    ) -> InfrastructureResult<Chat> {
        let operation_result = create_chat(
            self.database_transaction_mut()?,
            chat_name.into(),
            project_id.into(),
            initialization_parameters,
        )
        .await;
        self.record_operation_result(operation_result)
    }

    pub async fn update_chat(
        &mut self,
        update_options: ChatUpdateOptions,
    ) -> InfrastructureResult<Option<Chat>> {
        let operation_result = update_chat(self.database_transaction_mut()?, update_options).await;
        self.record_operation_result(operation_result)
    }

    pub async fn delete_chat(&mut self, chat: &Chat) -> InfrastructureResult<Option<Chat>> {
        self.delete_chat_by_id(chat.id).await
    }

    pub async fn delete_chat_by_id(
        &mut self,
        chat_id: impl Into<ChatId>,
    ) -> InfrastructureResult<Option<Chat>> {
        let operation_result =
            delete_chat_by_id(self.database_transaction_mut()?, chat_id.into()).await;
        self.record_operation_result(operation_result)
    }

    pub async fn set_llm_thinking_for_chat(
        &mut self,
        is_enabled: bool,
        chat: &Chat,
    ) -> InfrastructureResult<bool> {
        self.set_llm_thinking_for_chat_by_id(is_enabled, chat.id)
            .await
    }

    pub async fn set_llm_thinking_for_chat_by_id(
        &mut self,
        is_enabled: bool,
        chat_id: impl Into<ChatId>,
    ) -> InfrastructureResult<bool> {
        self.set_boolean_chat_value("llm_thinking_enabled", is_enabled, chat_id.into())
            .await
    }

    pub async fn set_llm_context_optimization_for_chat(
        &mut self,
        is_enabled: bool,
        chat: &Chat,
    ) -> InfrastructureResult<bool> {
        self.set_llm_context_optimization_for_chat_by_id(is_enabled, chat.id)
            .await
    }

    pub async fn set_llm_context_optimization_for_chat_by_id(
        &mut self,
        is_enabled: bool,
        chat_id: impl Into<ChatId>,
    ) -> InfrastructureResult<bool> {
        self.set_boolean_chat_value(
            "llm_context_optimization_enabled",
            is_enabled,
            chat_id.into(),
        )
        .await
    }

    pub async fn set_cpu_usage_for_chat(
        &mut self,
        cpu_usage_percentage: u8,
        chat: &Chat,
    ) -> InfrastructureResult<bool> {
        self.set_cpu_usage_for_chat_by_id(cpu_usage_percentage, chat.id)
            .await
    }

    pub async fn set_cpu_usage_for_chat_by_id(
        &mut self,
        cpu_usage_percentage: u8,
        chat_id: impl Into<ChatId>,
    ) -> InfrastructureResult<bool> {
        self.set_usage_chat_value("cpu_usage_percentage", cpu_usage_percentage, chat_id.into())
            .await
    }

    pub async fn set_gpu_usage_for_chat(
        &mut self,
        gpu_usage_percentage: u8,
        chat: &Chat,
    ) -> InfrastructureResult<bool> {
        self.set_gpu_usage_for_chat_by_id(gpu_usage_percentage, chat.id)
            .await
    }

    pub async fn set_gpu_usage_for_chat_by_id(
        &mut self,
        gpu_usage_percentage: u8,
        chat_id: impl Into<ChatId>,
    ) -> InfrastructureResult<bool> {
        self.set_usage_chat_value("gpu_usage_percentage", gpu_usage_percentage, chat_id.into())
            .await
    }
}
