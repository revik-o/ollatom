use super::InfrastructureTransaction;
use crate::{
    AttachmentInput, Chat, ChatId, InfrastructureResult, LlmAction, LlmActionDetails,
    LlmActionInput, LlmActionStatusEventInput, Message, MessageId,
};

impl InfrastructureTransaction {
    pub async fn add_message_from_user(
        &mut self,
        message_contents: impl Into<String>,
        attachments: Vec<AttachmentInput>,
        chat: &Chat,
    ) -> InfrastructureResult<Message> {
        self.add_message_from_user_by_chat_id(message_contents, attachments, chat.id)
            .await
    }

    pub async fn add_message_from_user_by_chat_id(
        &mut self,
        message_contents: impl Into<String>,
        attachments: Vec<AttachmentInput>,
        chat_id: impl Into<ChatId>,
    ) -> InfrastructureResult<Message> {
        let operation_result = crate::message_operations::add_message_from_user(
            self.database_transaction_mut()?,
            message_contents.into(),
            attachments,
            chat_id.into(),
        )
        .await;
        self.record_operation_result(operation_result)
    }

    pub async fn add_message_from_llm(
        &mut self,
        message_contents: impl Into<String>,
        actions: Vec<LlmActionInput>,
        answer_on_user_message: &Message,
        chat: &Chat,
    ) -> InfrastructureResult<Message> {
        self.add_message_from_llm_by_chat_id_and_user_message_id(
            message_contents,
            actions,
            chat.id,
            answer_on_user_message.id,
        )
        .await
    }

    pub async fn add_message_from_llm_by_chat_id_and_user_message_id(
        &mut self,
        message_contents: impl Into<String>,
        actions: Vec<LlmActionInput>,
        chat_id: impl Into<ChatId>,
        user_message_id: impl Into<MessageId>,
    ) -> InfrastructureResult<Message> {
        let operation_result = crate::message_operations::add_message_from_llm(
            self.database_transaction_mut()?,
            message_contents.into(),
            actions,
            chat_id.into(),
            user_message_id.into(),
        )
        .await;
        self.record_operation_result(operation_result)
    }

    pub async fn edit_message_from_user(
        &mut self,
        new_message_contents: impl Into<String>,
        old_message: &Message,
    ) -> InfrastructureResult<Option<Message>> {
        self.edit_message_from_user_by_id(new_message_contents, old_message.id)
            .await
    }

    pub async fn edit_message_from_user_by_id(
        &mut self,
        new_message_contents: impl Into<String>,
        message_id: impl Into<MessageId>,
    ) -> InfrastructureResult<Option<Message>> {
        let operation_result = crate::message_operations::edit_message_from_user(
            self.database_transaction_mut()?,
            new_message_contents.into(),
            message_id.into(),
        )
        .await;
        self.record_operation_result(operation_result)
    }

    pub async fn delete_message_from_user(
        &mut self,
        message: &Message,
    ) -> InfrastructureResult<Option<Message>> {
        self.delete_message_from_user_by_id(message.id).await
    }

    pub async fn delete_message_from_user_by_id(
        &mut self,
        message_id: impl Into<MessageId>,
    ) -> InfrastructureResult<Option<Message>> {
        let operation_result = crate::message_operations::delete_message_from_user(
            self.database_transaction_mut()?,
            message_id.into(),
        )
        .await;
        self.record_operation_result(operation_result)
    }

    pub async fn begin_llm_message_by_chat_id_and_user_message_id(
        &mut self,
        chat_id: impl Into<ChatId>,
        user_message_id: impl Into<MessageId>,
    ) -> InfrastructureResult<Message> {
        let operation_result = crate::message_operations::begin_llm_message(
            self.database_transaction_mut()?,
            chat_id.into(),
            user_message_id.into(),
        )
        .await;
        self.record_operation_result(operation_result)
    }

    pub async fn add_llm_action_to_message_by_id(
        &mut self,
        message_id: impl Into<MessageId>,
        summary: Option<String>,
        action_details: LlmActionDetails,
        initial_status_event: LlmActionStatusEventInput,
    ) -> InfrastructureResult<LlmAction> {
        let operation_result = crate::message_operations::add_llm_action_to_message(
            self.database_transaction_mut()?,
            message_id.into(),
            summary,
            action_details,
            initial_status_event,
        )
        .await;
        self.record_operation_result(operation_result)
    }

    pub async fn append_llm_action_status_event(
        &mut self,
        llm_action_id: impl Into<crate::LlmActionId>,
        status_event: LlmActionStatusEventInput,
    ) -> InfrastructureResult<crate::LlmActionStatusEvent> {
        let operation_result = crate::message_operations::append_llm_action_status_event(
            self.database_transaction_mut()?,
            llm_action_id.into(),
            status_event,
        )
        .await;
        self.record_operation_result(operation_result)
    }

    pub async fn complete_llm_message_by_id(
        &mut self,
        message_id: impl Into<MessageId>,
        completed_message_contents: impl Into<String>,
    ) -> InfrastructureResult<Option<Message>> {
        let operation_result = crate::message_operations::complete_llm_message(
            self.database_transaction_mut()?,
            message_id.into(),
            completed_message_contents.into(),
        )
        .await;
        self.record_operation_result(operation_result)
    }

    pub async fn mark_llm_message_as_failed_by_id(
        &mut self,
        message_id: impl Into<MessageId>,
        failure_message_contents: impl Into<String>,
    ) -> InfrastructureResult<Option<Message>> {
        let operation_result = crate::message_operations::finish_llm_message_unsuccessfully(
            self.database_transaction_mut()?,
            message_id.into(),
            crate::LlmMessageState::Failed,
            failure_message_contents.into(),
        )
        .await;
        self.record_operation_result(operation_result)
    }

    pub async fn mark_llm_message_as_cancelled_by_id(
        &mut self,
        message_id: impl Into<MessageId>,
    ) -> InfrastructureResult<Option<Message>> {
        let operation_result = crate::message_operations::finish_llm_message_unsuccessfully(
            self.database_transaction_mut()?,
            message_id.into(),
            crate::LlmMessageState::Cancelled,
            String::new(),
        )
        .await;
        self.record_operation_result(operation_result)
    }
}
