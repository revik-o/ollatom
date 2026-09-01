use crate::{BoxFuture, InteractionRequest, LlmError, RunId, ToolCall, ToolOutput, Usage};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{Mutex, Notify, OnceCell, mpsc};

const CALLBACK_QUEUE_CAPACITY: usize = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ToolEvent {
    Planned { call: ToolCall },
    ApprovalRequested { call_id: String },
    Started { call: ToolCall },
    Finished { output: ToolOutput },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RunEvent {
    ReasoningSummaryDelta(String),
    ModelTraceDelta(String),
    ResponseDelta(String),
    Tool(ToolEvent),
    Usage(Usage),
    Warning(String),
    InteractionRequested(InteractionRequest),
    ProviderRoundStarted { round: u16 },
    Completed,
    Cancelled,
    Failed(String),
}

impl RunEvent {
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::Tool(_)
                | Self::InteractionRequested(_)
                | Self::Completed
                | Self::Cancelled
                | Self::Failed(_)
        )
    }
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed(_))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SequencedEvent {
    pub run_id: RunId,
    pub sequence: u64,
    pub event: RunEvent,
}

pub trait RunEventSink: Send + Sync {
    fn emit(&self, event: SequencedEvent) -> BoxFuture<'_, Result<(), LlmError>>;
}

pub type EventCallback = Arc<dyn Fn(SequencedEvent) -> BoxFuture<'static, ()> + Send + Sync>;

#[derive(Clone, Default)]
pub struct EventCallbacks {
    callbacks: Vec<EventCallback>,
}

impl EventCallbacks {
    pub fn push(&mut self, callback: EventCallback) {
        self.callbacks.push(callback);
    }
}

pub struct RunEventStream {
    event_queue: Arc<EventQueue>,
}

impl RunEventStream {
    pub async fn next(&mut self) -> Option<SequencedEvent> {
        loop {
            let event_available = self.event_queue.notify.notified();
            if let Some(event) = self.event_queue.events.lock().ok()?.pop_front() {
                return Some(event);
            }
            if self.event_queue.closed.load(Ordering::SeqCst) {
                return None;
            }
            event_available.await;
        }
    }
}

struct EventQueue {
    events: StdMutex<VecDeque<SequencedEvent>>,
    notify: Notify,
    closed: AtomicBool,
}

impl EventQueue {
    fn push(&self, sequenced_event: SequencedEvent) {
        let Ok(mut queued_events) = self.events.lock() else {
            return;
        };
        queued_events.push_back(sequenced_event);
        let terminal_event_was_queued = queued_events
            .back()
            .is_some_and(|queued_event| queued_event.event.is_terminal());
        if terminal_event_was_queued {
            self.closed.store(true, Ordering::SeqCst);
        }
        self.notify.notify_one();
    }
    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}

pub(crate) struct EventDispatcher {
    run_id: RunId,
    state: Mutex<DispatchState>,
    event_sink: Option<Arc<dyn RunEventSink>>,
    callbacks: Vec<EventCallback>,
    callback_senders: OnceCell<Vec<mpsc::Sender<SequencedEvent>>>,
    event_queue: Arc<EventQueue>,
}

struct DispatchState {
    next_sequence: u64,
    terminal: bool,
    streamed: StreamedOutput,
}

#[derive(Clone, Default)]
pub(crate) struct StreamedOutput {
    pub text: String,
    pub visible_reasoning: String,
    pub usage: Usage,
}

impl DispatchState {
    fn accumulate_streamed_output(&mut self, event: &RunEvent) {
        match event {
            RunEvent::ResponseDelta(delta) => self.streamed.text.push_str(delta),
            RunEvent::ReasoningSummaryDelta(delta) => {
                self.streamed.visible_reasoning.push_str(delta);
            }
            RunEvent::Usage(usage) => self.streamed.usage = usage.clone(),
            _ => {}
        }
    }
}

impl EventDispatcher {
    pub fn new(
        run_id: RunId,
        event_sink: Option<Arc<dyn RunEventSink>>,
        callbacks: EventCallbacks,
    ) -> (Self, RunEventStream) {
        let event_queue = Arc::new(EventQueue {
            events: StdMutex::new(VecDeque::new()),
            notify: Notify::new(),
            closed: AtomicBool::new(false),
        });

        let stream = RunEventStream {
            event_queue: event_queue.clone(),
        };

        (
            Self {
                run_id,
                state: Mutex::new(DispatchState {
                    next_sequence: 1,
                    terminal: false,
                    streamed: StreamedOutput::default(),
                }),
                event_sink,
                callbacks: callbacks.callbacks,
                callback_senders: OnceCell::new(),
                event_queue,
            },
            stream,
        )
    }
    pub async fn emit(&self, event: RunEvent) -> Result<(), LlmError> {
        let mut state = self.state.lock().await;

        if state.terminal {
            return Err(LlmError::ProviderProtocol(
                "event emitted after terminal event".into(),
            ));
        }

        let event_is_terminal = event.is_terminal();
        state.accumulate_streamed_output(&event);
        let sequenced_event = SequencedEvent {
            run_id: self.run_id,
            sequence: state.next_sequence,
            event,
        };
        state.next_sequence += 1;

        if let Some(event_sink) = &self.event_sink {
            event_sink.emit(sequenced_event.clone()).await?;
        }

        self.event_queue.push(sequenced_event.clone());

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
            if sequenced_event.event.is_critical() {
                let _ignored_closed_receiver = callback_sender.send(sequenced_event.clone()).await;
            } else {
                let _ignored_full_or_closed_queue =
                    callback_sender.try_send(sequenced_event.clone());
            }
        }

        state.terminal = event_is_terminal;

        Ok(())
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
