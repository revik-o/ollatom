use crate::{
    InteractionId, InteractionReply, InteractionRequest, LlmError, RunEvent, StopToken,
    events::EventDispatcher,
};
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::oneshot;

pub(crate) struct InteractionHub {
    next_id: AtomicU64,
    pending: Mutex<BTreeMap<InteractionId, oneshot::Sender<InteractionReply>>>,
}
impl InteractionHub {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            pending: Mutex::new(BTreeMap::new()),
        }
    }

    pub async fn request_interaction<CreateRequest>(
        &self,
        create_request: CreateRequest,
        event_dispatcher: &EventDispatcher,
        stop_token: &StopToken,
    ) -> Result<InteractionReply, LlmError>
    where
        CreateRequest: FnOnce(InteractionId) -> InteractionRequest,
    {
        let interaction_id = InteractionId(self.next_id.fetch_add(1, Ordering::SeqCst));
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.pending
            .lock()
            .map_err(|_| LlmError::ProviderProtocol("interaction registry lock poisoned".into()))?
            .insert(interaction_id, reply_sender);

        let emit_result = event_dispatcher
            .emit(RunEvent::InteractionRequested(create_request(
                interaction_id,
            )))
            .await;
        if let Err(error) = emit_result {
            self.remove_pending_interaction(interaction_id);
            return Err(error);
        }

        tokio::select! {
            _ = stop_token.cancelled() => {
                self.remove_pending_interaction(interaction_id);
                Err(LlmError::Cancelled)
            }
            reply = reply_receiver => reply.map_err(|_| {
                LlmError::InteractionAlreadyResolved(interaction_id.0)
            }),
        }
    }

    pub fn resolve_interaction(
        &self,
        interaction_id: InteractionId,
        reply: InteractionReply,
    ) -> Result<(), LlmError> {
        let reply_sender = self
            .pending
            .lock()
            .map_err(|_| LlmError::ProviderProtocol("interaction registry lock poisoned".into()))?
            .remove(&interaction_id)
            .ok_or(LlmError::InteractionNotFound(interaction_id.0))?;

        reply_sender
            .send(reply)
            .map_err(|_| LlmError::InteractionAlreadyResolved(interaction_id.0))
    }

    fn remove_pending_interaction(&self, interaction_id: InteractionId) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(&interaction_id);
        }
    }
}
