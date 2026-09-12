use crate::events::SequencedEvent;
use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

pub struct RunEventStream {
    event_queue: Arc<EventQueue>,
}

impl RunEventStream {
    pub(crate) fn new(event_queue: Arc<EventQueue>) -> Self {
        Self { event_queue }
    }

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

pub(crate) struct EventQueue {
    pub(crate) events: Mutex<VecDeque<SequencedEvent>>,
    pub(crate) notify: Notify,
    pub(crate) closed: AtomicBool,
}

impl EventQueue {
    pub(crate) fn new() -> Self {
        Self {
            events: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            closed: AtomicBool::new(false),
        }
    }

    pub(crate) fn push(&self, sequenced_event: SequencedEvent) {
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

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}
