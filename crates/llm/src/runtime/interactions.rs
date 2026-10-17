use crate::{
    InteractionId, InteractionReply, InteractionRequest, LlmError, RunEvent, StopToken, events::EventDispatcher,
    ids::allocate_monotonic_identifier,
};
use std::{
    collections::BTreeMap,
    sync::{Mutex, atomic::AtomicU64},
};
use tokio::sync::oneshot;

pub(crate) struct InteractionHub {
    next_id: AtomicU64,
    pending: Mutex<BTreeMap<InteractionId, oneshot::Sender<InteractionReply>>>,
}

struct PendingInteractionGuard<'hub> {
    interaction_hub: &'hub InteractionHub,
    interaction_id: InteractionId,
}

impl Drop for PendingInteractionGuard<'_> {
    fn drop(&mut self) {
        self.interaction_hub.remove_pending_interaction(self.interaction_id);
    }
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
        let interaction_id = InteractionId(allocate_monotonic_identifier(&self.next_id, "interaction")?);
        let (reply_sender, reply_receiver) = oneshot::channel();

        self.pending
            .lock()
            .map_err(|_| LlmError::ProviderProtocol("interaction registry lock poisoned".into()))?
            .insert(interaction_id, reply_sender);

        let _pending_interaction_guard = PendingInteractionGuard {
            interaction_hub: self,
            interaction_id,
        };

        event_dispatcher
            .emit(RunEvent::InteractionRequested(create_request(interaction_id)))
            .await?;

        tokio::select! {
            _ = stop_token.cancelled() => Err(LlmError::Cancelled),
            reply = reply_receiver => reply.map_err(|_| {
                LlmError::InteractionAlreadyResolved(interaction_id.0)
            }),
        }
    }

    pub fn resolve_interaction(&self, interaction_id: InteractionId, reply: InteractionReply) -> Result<(), LlmError> {
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
