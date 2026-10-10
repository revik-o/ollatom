use super::{
    CALLBACK_QUEUE_CAPACITY, EventCallback, EventCallbacks, RunEvent, RunEventDeliveryFailure,
    RunEventSink, RunEventStream, RunId, SequencedEvent, StreamedOutput, ToolEvent,
};
use crate::{AssistantRound, LlmError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OnceCell, mpsc};

use crate::event_queue::EventQueue;

mod deadline;

const DEFAULT_EVENT_DELIVERY_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct EventDispatcher {
    run_id: RunId,
    state: Mutex<DispatchState>,
    event_sink: Option<Arc<dyn RunEventSink>>,
    callbacks: Vec<EventCallback>,
    callback_senders: OnceCell<Vec<mpsc::Sender<SequencedEvent>>>,
    event_queue: Arc<EventQueue>,
    overall_deadline: Option<Instant>,
    event_delivery_timeout: Duration,
}

struct DispatchState {
    next_sequence: u64,
    terminal: bool,
    streamed: StreamedOutput,
    pending_delivery: Option<SequencedEvent>,
}

impl DispatchState {
    fn accumulate_streamed_output(&mut self, event: &RunEvent) {
        match event {
            RunEvent::ResponseDelta(delta) => {
                if let Some(current_round) = &mut self.streamed.current_round {
                    current_round.text.push_str(delta);
                }
            }
            RunEvent::ReasoningSummaryDelta(delta) => {
                if let Some(current_round) = &mut self.streamed.current_round {
                    current_round
                        .visible_reasoning
                        .get_or_insert_with(String::new)
                        .push_str(delta);
                }
            }
            RunEvent::Tool(ToolEvent::Planned { call } | ToolEvent::Started { call }) => {
                self.streamed.add_tool_call(call);
            }
            RunEvent::Usage(usage) => {
                self.streamed.usage = usage.clone();
            }
            RunEvent::ProviderRoundStarted { round } => {
                self.streamed.finish_current_round();
                self.streamed.current_round = Some(AssistantRound {
                    round_number: *round,
                    ..AssistantRound::default()
                });
            }
            RunEvent::ProviderRoundFinished {
                round,
                finish_reason,
                tool_calls,
                usage,
            } => self
                .streamed
                .record_round_completion(*round, *finish_reason, tool_calls, usage),
            RunEvent::Completed | RunEvent::Cancelled | RunEvent::Failed(_) => {
                self.streamed.finish_current_round();
            }
            _ => {}
        }
    }
}

impl EventDispatcher {
    pub fn new(
        run_id: RunId,
        event_sink: Option<Arc<dyn RunEventSink>>,
        callbacks: EventCallbacks,
        overall_deadline: Option<Instant>,
        event_delivery_timeout: Option<Duration>,
    ) -> (Self, RunEventStream) {
        let event_queue = Arc::new(EventQueue::new());

        let stream = RunEventStream::new(event_queue.clone());

        (
            Self {
                run_id,
                state: Mutex::new(DispatchState {
                    next_sequence: 1,
                    terminal: false,
                    streamed: StreamedOutput::default(),
                    pending_delivery: None,
                }),
                event_sink,
                callbacks: callbacks.callbacks,
                callback_senders: OnceCell::new(),
                event_queue,
                overall_deadline,
                event_delivery_timeout: event_delivery_timeout
                    .unwrap_or(DEFAULT_EVENT_DELIVERY_TIMEOUT),
            },
            stream,
        )
    }
    pub async fn emit(&self, event: RunEvent) -> Result<(), LlmError> {
        self.emit_with_durable_ack(event).await.1
    }

    pub(crate) async fn emit_with_durable_ack(
        &self,
        event: RunEvent,
    ) -> (bool, Result<(), LlmError>) {
        let event_delivery_deadline = Instant::now().checked_add(self.event_delivery_timeout);
        let mut state = match self.lock_for_event_delivery(event_delivery_deadline).await {
            Ok(state) => state,
            Err(error) => return (false, Err(error)),
        };

        if state.terminal {
            return (
                false,
                Err(LlmError::ProviderProtocol(
                    "event emitted after terminal event".into(),
                )),
            );
        }

        let event_is_terminal = event.is_terminal();

        let sequenced_event = match &state.pending_delivery {
            Some(pending_event) if pending_event.event != event => {
                return (
                    false,
                    Err(LlmError::EventSink(format!(
                        "event sequence {} is waiting for durable delivery",
                        pending_event.sequence
                    ))),
                );
            }
            Some(pending_event) => pending_event.clone(),
            None => SequencedEvent {
                run_id: self.run_id,
                sequence: state.next_sequence,
                event,
            },
        };

        let next_sequence = match sequenced_event.sequence.checked_add(1) {
            Some(next_sequence) => next_sequence,
            None => {
                return (
                    false,
                    Err(LlmError::ProviderProtocol("event sequence overflow".into())),
                );
            }
        };

        if let Some(event_sink) = &self.event_sink
            && let Err(error) = self
                .persist_before_deadline(
                    event_sink,
                    sequenced_event.clone(),
                    event_delivery_deadline,
                )
                .await
        {
            state.pending_delivery = Some(sequenced_event.clone());
            self.event_queue.delivery_failed(RunEventDeliveryFailure {
                sequence: sequenced_event.sequence,
                message: error.to_string(),
            });

            return (false, Err(error));
        }

        state.pending_delivery = None;
        state.accumulate_streamed_output(&sequenced_event.event);
        self.event_queue.push(sequenced_event.clone());
        state.next_sequence = next_sequence;
        state.terminal = event_is_terminal;

        let senders = self
            .callback_senders
            .get_or_init(|| async {
                self.callbacks
                    .iter()
                    .cloned()
                    .map(|callback| {
                        let (callback_sender, mut callback_receiver) =
                            mpsc::channel(CALLBACK_QUEUE_CAPACITY);

                        tokio::spawn(async move {
                            while let Some(callback_event) = callback_receiver.recv().await {
                                callback(callback_event).await;
                            }
                        });

                        callback_sender
                    })
                    .collect()
            })
            .await;

        for callback_sender in senders {
            if let Err(error) = callback_sender.try_send(sequenced_event.clone()) {
                let callback_error = LlmError::EventSink(format!(
                    "callback delivery failed for sequence {}: {error}",
                    sequenced_event.sequence
                ));

                self.event_queue.delivery_failed(RunEventDeliveryFailure {
                    sequence: sequenced_event.sequence,
                    message: callback_error.to_string(),
                });

                return (true, Err(callback_error));
            }
        }

        (true, Ok(()))
    }

    pub async fn streamed_output(&self) -> StreamedOutput {
        self.state.lock().await.streamed.clone()
    }
}

impl Drop for EventDispatcher {
    fn drop(&mut self) {
        self.event_queue.close();
    }
}
