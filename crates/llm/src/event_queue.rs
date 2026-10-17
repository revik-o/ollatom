use crate::events::{CoalescedRunDelta, RunEvent, RunEventDeliveryFailure, RunStreamItem, RunStreamLag, SequencedEvent};
use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

mod helpers;

use helpers::{coalesce_delta, item_size, push_bounded};

pub(crate) const MAXIMUM_QUEUE_ITEMS: usize = 1_024;
pub(crate) const MAXIMUM_QUEUE_BYTES: usize = 4 * 1_024 * 1_024;
const MAXIMUM_COALESCED_DELTA_BYTES: usize = 64 * 1_024;

pub struct RunEventStream {
    event_queue: Arc<EventQueue>,
}

impl RunEventStream {
    pub(crate) fn new(event_queue: Arc<EventQueue>) -> Self {
        Self { event_queue }
    }

    pub async fn next(&mut self) -> Option<RunStreamItem> {
        loop {
            let event_available = self.event_queue.notify.notified();
            if let Some(event) = self.event_queue.pop() {
                return Some(event);
            }
            if self.event_queue.closed.load(Ordering::SeqCst) {
                return None;
            }
            event_available.await;
        }
    }
}

pub(crate) struct QueueState {
    pub(crate) items: VecDeque<RunStreamItem>,
    pub(crate) estimated_bytes: usize,
}

pub(crate) struct EventQueue {
    pub(crate) state: Mutex<QueueState>,
    pub(crate) notify: Notify,
    pub(crate) closed: AtomicBool,
}

impl EventQueue {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(QueueState {
                items: VecDeque::new(),
                estimated_bytes: 0,
            }),
            notify: Notify::new(),
            closed: AtomicBool::new(false),
        }
    }

    pub(crate) fn push(&self, sequenced_event: SequencedEvent) {
        let terminal = sequenced_event.event.is_terminal();
        let Ok(mut state) = self.state.lock() else {
            return;
        };

        if !coalesce_delta(&mut state, &sequenced_event) {
            push_bounded(&mut state, RunStreamItem::Event(sequenced_event));
        }

        if terminal {
            self.closed.store(true, Ordering::SeqCst);
        }

        self.notify.notify_one();
    }

    pub(crate) fn delivery_failed(&self, failure: RunEventDeliveryFailure) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }

        if let Ok(mut state) = self.state.lock() {
            push_bounded(&mut state, RunStreamItem::DeliveryFailed(failure));
        }

        self.notify.notify_waiters();
    }

    pub(crate) fn pop(&self) -> Option<RunStreamItem> {
        let mut state = self.state.lock().ok()?;
        let item = state.items.pop_front()?;

        state.estimated_bytes = state.estimated_bytes.saturating_sub(item_size(&item));

        Some(item)
    }

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}
