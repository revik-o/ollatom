use super::RequestBuilder;
use crate::{InteractionReply, InteractionRequest, RunEvent, SequencedEvent};
use std::{future::Future, sync::Arc};

impl<State> RequestBuilder<State> {
    pub fn on_event<Callback, CallbackFuture>(mut self, callback: Callback) -> Self
    where
        Callback: Fn(SequencedEvent) -> CallbackFuture + Send + Sync + 'static,
        CallbackFuture: Future<Output = ()> + Send + 'static,
    {
        self.data
            .callbacks
            .push(Arc::new(move |event| Box::pin(callback(event))));

        self
    }

    pub fn on_reasoning_delta<Callback, CallbackFuture>(self, callback: Callback) -> Self
    where
        Callback: Fn(String) -> CallbackFuture + Send + Sync + 'static,
        CallbackFuture: Future<Output = ()> + Send + 'static,
    {
        self.on_event_payload(
            |event| match event {
                RunEvent::ReasoningSummaryDelta(delta) => Some(delta),
                _ => None,
            },
            callback,
        )
    }

    pub fn on_tool_event<Callback, CallbackFuture>(self, callback: Callback) -> Self
    where
        Callback: Fn(crate::ToolEvent) -> CallbackFuture + Send + Sync + 'static,
        CallbackFuture: Future<Output = ()> + Send + 'static,
    {
        self.on_event_payload(
            |event| match event {
                RunEvent::Tool(tool_event) => Some(tool_event),
                _ => None,
            },
            callback,
        )
    }

    pub fn on_response_delta<Callback, CallbackFuture>(self, callback: Callback) -> Self
    where
        Callback: Fn(String) -> CallbackFuture + Send + Sync + 'static,
        CallbackFuture: Future<Output = ()> + Send + 'static,
    {
        self.on_event_payload(
            |event| match event {
                RunEvent::ResponseDelta(delta) => Some(delta),
                _ => None,
            },
            callback,
        )
    }

    pub fn on_usage<Callback, CallbackFuture>(self, callback: Callback) -> Self
    where
        Callback: Fn(crate::Usage) -> CallbackFuture + Send + Sync + 'static,
        CallbackFuture: Future<Output = ()> + Send + 'static,
    {
        self.on_event_payload(
            |event| match event {
                RunEvent::Usage(usage) => Some(usage),
                _ => None,
            },
            callback,
        )
    }

    pub fn on_interaction<Callback, CallbackFuture>(mut self, callback: Callback) -> Self
    where
        Callback: Fn(InteractionRequest) -> CallbackFuture + Send + Sync + 'static,
        CallbackFuture: Future<Output = InteractionReply> + Send + 'static,
    {
        self.data.interaction_callback = Some(Arc::new(move |request| Box::pin(callback(request))));
        self
    }

    fn on_event_payload<Payload, Selector, Callback, CallbackFuture>(
        self,
        selector: Selector,
        callback: Callback,
    ) -> Self
    where
        Payload: Send + 'static,
        Selector: Fn(RunEvent) -> Option<Payload> + Send + Sync + 'static,
        Callback: Fn(Payload) -> CallbackFuture + Send + Sync + 'static,
        CallbackFuture: Future<Output = ()> + Send + 'static,
    {
        let selector = Arc::new(selector);
        let callback = Arc::new(callback);

        self.on_event(move |sequenced_event| {
            let selector = selector.clone();
            let callback = callback.clone();

            Box::pin(async move {
                if let Some(payload) = selector(sequenced_event.event) {
                    callback(payload).await;
                }
            })
        })
    }
}
