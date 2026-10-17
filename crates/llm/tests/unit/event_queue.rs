use crate::event_queue::*;
use crate::{RunEvent, RunEventDeliveryFailure, RunId, RunStreamItem, SequencedEvent};
use std::sync::atomic::Ordering;

fn event(sequence: u64, event: RunEvent) -> SequencedEvent {
    SequencedEvent {
        run_id: RunId(1),
        sequence,
        event,
    }
}

#[test]
fn adjacent_deltas_are_coalesced() {
    let queue = EventQueue::new();
    queue.push(event(1, RunEvent::ResponseDelta("a".into())));
    queue.push(event(2, RunEvent::ResponseDelta("b".into())));

    let Some(RunStreamItem::CoalescedDelta(delta)) = queue.pop() else {
        panic!("expected coalesced delta");
    };
    assert_eq!((delta.first_sequence, delta.last_sequence), (1, 2));
    assert_eq!(delta.event, RunEvent::ResponseDelta("ab".into()));
}

#[test]
fn pressure_is_reported_and_terminal_event_is_retained() {
    let queue = EventQueue::new();
    for sequence in 1..=1_100 {
        queue.push(event(sequence, RunEvent::Warning(sequence.to_string())));
    }
    queue.push(event(1_101, RunEvent::Completed));

    let state = queue.state.lock().unwrap();
    assert!(state.items.len() <= MAXIMUM_QUEUE_ITEMS);
    assert!(state.estimated_bytes <= MAXIMUM_QUEUE_BYTES);
    assert!(state.items.iter().any(|item| matches!(item, RunStreamItem::Lagged(_))));
    assert!(matches!(
        state.items.back(),
        Some(RunStreamItem::Event(SequencedEvent {
            event: RunEvent::Completed,
            ..
        }))
    ));
}

#[test]
fn durable_failure_is_visible_before_stream_closure() {
    let queue = EventQueue::new();
    queue.delivery_failed(RunEventDeliveryFailure {
        sequence: 7,
        message: "disk unavailable".into(),
    });

    assert!(queue.closed.load(Ordering::SeqCst));
    assert!(matches!(
        queue.pop(),
        Some(RunStreamItem::DeliveryFailed(RunEventDeliveryFailure {
            sequence: 7,
            ..
        }))
    ));
}
