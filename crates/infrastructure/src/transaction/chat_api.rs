use super::InfrastructureTransaction;
use super::store::{
    BooleanEntitySetting, UsageEntitySetting, create_chat, delete_chat_by_id, update_chat,
};
use crate::{
    Chat, ChatId, ChatInitializationParameters, ChatUpdateOptions, InfrastructureResult, ProjectId,
};

impl InfrastructureTransaction {
    pub async fn create_chat(
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

    pub async fn delete_chat(
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
        chat_id: impl Into<ChatId>,
    ) -> InfrastructureResult<bool> {
        self.set_boolean_setting(
            BooleanEntitySetting::ChatThinking,
            is_enabled,
            chat_id.into().as_bytes().to_vec(),
        )
        .await
    }

    pub async fn set_llm_context_optimization_for_chat(
        &mut self,
        is_enabled: bool,
        chat_id: impl Into<ChatId>,
    ) -> InfrastructureResult<bool> {
        self.set_boolean_setting(
            BooleanEntitySetting::ChatContextOptimization,
            is_enabled,
            chat_id.into().as_bytes().to_vec(),
        )
        .await
    }

    pub async fn set_cpu_usage_for_chat(
        &mut self,
        cpu_usage_percentage: u8,
        chat_id: impl Into<ChatId>,
    ) -> InfrastructureResult<bool> {
        self.set_usage_setting(
            UsageEntitySetting::ChatCpu,
            cpu_usage_percentage,
            chat_id.into().as_bytes().to_vec(),
        )
        .await
    }

    pub async fn set_gpu_usage_for_chat(
        &mut self,
        gpu_usage_percentage: u8,
        chat_id: impl Into<ChatId>,
    ) -> InfrastructureResult<bool> {
        self.set_usage_setting(
            UsageEntitySetting::ChatGpu,
            gpu_usage_percentage,
            chat_id.into().as_bytes().to_vec(),
        )
        .await
    }
}
