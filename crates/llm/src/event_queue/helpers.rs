use super::*;

pub(super) fn coalesce_delta(state: &mut QueueState, event: &SequencedEvent) -> bool {
    let Some(delta) = delta_text(&event.event) else {
        return false;
    };

    if let Some(RunStreamItem::Event(previous)) = state.items.back()
        && same_delta_kind(&previous.event, &event.event)
    {
        return replace_event_with_coalesced(state, event);
    }

    let Some(RunStreamItem::CoalescedDelta(coalesced)) = state.items.back_mut() else {
        return false;
    };

    if !same_delta_kind(&coalesced.event, &event.event) {
        return false;
    }

    let Some(existing_delta) = delta_text_mut(&mut coalesced.event) else {
        return false;
    };

    if existing_delta.len().saturating_add(delta.len()) > MAXIMUM_COALESCED_DELTA_BYTES {
        return false;
    }

    existing_delta.push_str(delta);
    coalesced.last_sequence = event.sequence;
    state.estimated_bytes = state.estimated_bytes.saturating_add(delta.len());

    true
}

fn replace_event_with_coalesced(state: &mut QueueState, event: &SequencedEvent) -> bool {
    let Some(RunStreamItem::Event(previous)) = state.items.back() else {
        return false;
    };

    let (Some(previous_delta), Some(delta)) =
        (delta_text(&previous.event), delta_text(&event.event))
    else {
        return false;
    };

    if previous_delta.len().saturating_add(delta.len()) > MAXIMUM_COALESCED_DELTA_BYTES {
        return false;
    }

    let first_sequence = previous.sequence;
    let mut combined_event = previous.event.clone();

    if let Some(combined_delta) = delta_text_mut(&mut combined_event) {
        combined_delta.push_str(delta);
    }

    if let Some(previous_item) = state.items.pop_back() {
        state.estimated_bytes = state
            .estimated_bytes
            .saturating_sub(item_size(&previous_item));
    }

    let item = RunStreamItem::CoalescedDelta(CoalescedRunDelta {
        first_sequence,
        last_sequence: event.sequence,
        event: combined_event,
    });

    state.estimated_bytes = state.estimated_bytes.saturating_add(item_size(&item));
    state.items.push_back(item);

    true
}

pub(super) fn push_bounded(state: &mut QueueState, item: RunStreamItem) {
    let item_bytes = item_size(&item);

    if item_bytes > MAXIMUM_QUEUE_BYTES {
        if let Some((first, last, count)) = sequence_range(&item) {
            push_bounded(
                state,
                RunStreamItem::Lagged(RunStreamLag {
                    first_missing_sequence: first,
                    last_missing_sequence: last,
                    missing_event_count: count,
                }),
            );
        }

        return;
    }

    let mut missing_range: Option<(u64, u64, u64)> = None;

    while state.items.len().saturating_add(1) > MAXIMUM_QUEUE_ITEMS
        || state.estimated_bytes.saturating_add(item_bytes) > MAXIMUM_QUEUE_BYTES
    {
        let Some(evicted) = state.items.pop_front() else {
            break;
        };

        state.estimated_bytes = state.estimated_bytes.saturating_sub(item_size(&evicted));

        if let Some((first, last, count)) = sequence_range(&evicted) {
            missing_range = Some(match missing_range {
                Some((existing_first, _, existing_count)) => {
                    (existing_first, last, existing_count.saturating_add(count))
                }
                None => (first, last, count),
            });
        }
    }

    if let Some((first, last, count)) = missing_range {
        while state.items.len().saturating_add(2) > MAXIMUM_QUEUE_ITEMS {
            if let Some(evicted) = state.items.pop_front() {
                state.estimated_bytes = state.estimated_bytes.saturating_sub(item_size(&evicted));
            }
        }

        let lag = RunStreamItem::Lagged(RunStreamLag {
            first_missing_sequence: first,
            last_missing_sequence: last,
            missing_event_count: count,
        });

        state.estimated_bytes = state.estimated_bytes.saturating_add(item_size(&lag));
        state.items.push_front(lag);
    }
    state.estimated_bytes = state.estimated_bytes.saturating_add(item_bytes);
    state.items.push_back(item);
}

fn sequence_range(item: &RunStreamItem) -> Option<(u64, u64, u64)> {
    match item {
        RunStreamItem::Event(event) => Some((event.sequence, event.sequence, 1)),
        RunStreamItem::CoalescedDelta(delta) => Some((
            delta.first_sequence,
            delta.last_sequence,
            delta
                .last_sequence
                .saturating_sub(delta.first_sequence)
                .saturating_add(1),
        )),
        RunStreamItem::Lagged(lag) => Some((
            lag.first_missing_sequence,
            lag.last_missing_sequence,
            lag.missing_event_count,
        )),
        RunStreamItem::DeliveryFailed(_) => None,
    }
}

pub(super) fn item_size(item: &RunStreamItem) -> usize {
    serde_json::to_vec(item).map_or(MAXIMUM_QUEUE_BYTES, |bytes| bytes.len())
}

fn same_delta_kind(left: &RunEvent, right: &RunEvent) -> bool {
    matches!(
        (left, right),
        (RunEvent::ResponseDelta(_), RunEvent::ResponseDelta(_))
            | (
                RunEvent::ReasoningSummaryDelta(_),
                RunEvent::ReasoningSummaryDelta(_)
            )
            | (RunEvent::ModelTraceDelta(_), RunEvent::ModelTraceDelta(_))
    )
}

fn delta_text(event: &RunEvent) -> Option<&str> {
    match event {
        RunEvent::ResponseDelta(delta)
        | RunEvent::ReasoningSummaryDelta(delta)
        | RunEvent::ModelTraceDelta(delta) => Some(delta),
        _ => None,
    }
}

fn delta_text_mut(event: &mut RunEvent) -> Option<&mut String> {
    match event {
        RunEvent::ResponseDelta(delta)
        | RunEvent::ReasoningSummaryDelta(delta)
        | RunEvent::ModelTraceDelta(delta) => Some(delta),
        _ => None,
    }
}
