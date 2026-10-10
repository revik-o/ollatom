use super::execution_lifecycle::{ActiveRunGuard, terminal_event_for};
use crate::{LlmError, LlmRun, LlmRunOutcome, cancellation::stop_pair, events::EventDispatcher};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod request;

pub(crate) fn start_run(
    runtime_handle: tokio::runtime::Handle,
    mut request_data: crate::request::RequestData,
) -> Result<LlmRun, LlmError> {
    let (stop_token, stop_handle) = stop_pair();

    let run_id = match request_data.runtime.as_ref() {
        Some(runtime) => runtime.next_run_id()?,
        None => crate::RunId(0),
    };

    let event_sink = request_data
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.inner.event_sink.clone());

    let overall_timeout_milliseconds = request_data.options.transport.overall_timeout_ms;

    let overall_deadline = overall_timeout_milliseconds
        .and_then(|milliseconds| Instant::now().checked_add(Duration::from_millis(milliseconds)));

    let event_delivery_timeout = request_data
        .options
        .transport
        .event_delivery_timeout_ms
        .map(Duration::from_millis);

    let (event_dispatcher, event_stream) = EventDispatcher::new(
        run_id,
        event_sink,
        request_data.callbacks.clone(),
        overall_deadline,
        event_delivery_timeout,
    );

    let event_dispatcher = Arc::new(event_dispatcher);

    let registration_error = request_data
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.register_run(run_id, stop_handle.clone()).err());

    if let Some(error) = registration_error {
        request_data.validation_error = Some(error);
    }

    let registration_guard = ActiveRunGuard::new(request_data.runtime.clone(), run_id);
    let execution_stop_handle = stop_handle.clone();
    let (outcome_sender, outcome_receiver) = tokio::sync::oneshot::channel();

    let execution_task = runtime_handle.spawn(async move {
        let _registration_guard = registration_guard;

        let run_future = async {
            let run_result = request::execute_request(
                &mut request_data,
                run_id,
                event_dispatcher.clone(),
                stop_token,
                execution_stop_handle,
                overall_deadline,
            )
            .await;

            let terminal_event = terminal_event_for(&run_result);
            let (terminal_event_delivered, terminal_event_result) =
                event_dispatcher.emit_with_durable_ack(terminal_event).await;

            match run_result {
                Ok(LlmRunOutcome::Failed(mut failed_run)) => {
                    failed_run.terminal_event_delivered = terminal_event_delivered;
                    Ok(LlmRunOutcome::Failed(failed_run))
                }
                Ok(LlmRunOutcome::Completed(response)) if terminal_event_delivered => {
                    Ok(LlmRunOutcome::Completed(response))
                }
                Ok(LlmRunOutcome::Completed(response)) => {
                    Ok(LlmRunOutcome::Failed(crate::FailedRun {
                        error: terminal_event_result.err().unwrap_or_else(|| {
                            LlmError::EventSink("terminal event was not acknowledged".into())
                        }),
                        partial_response: response.into(),
                        terminal_event_delivered: false,
                    }))
                }
                Ok(LlmRunOutcome::Cancelled(partial_response)) if terminal_event_delivered => {
                    Ok(LlmRunOutcome::Cancelled(partial_response))
                }
                Ok(LlmRunOutcome::Cancelled(partial_response)) => {
                    Ok(LlmRunOutcome::Failed(crate::FailedRun {
                        error: terminal_event_result.err().unwrap_or_else(|| {
                            LlmError::EventSink("terminal event was not acknowledged".into())
                        }),
                        partial_response,
                        terminal_event_delivered: false,
                    }))
                }
                Err(error) => Err(error),
            }
        };

        let final_result = run_future.await;
        let _ignored_send_result = outcome_sender.send(final_result);
    });

    let outcome_future = Box::pin(async move {
        outcome_receiver
            .await
            .map_err(|_| LlmError::Provider("run task terminated without an outcome".into()))?
    });

    Ok(LlmRun {
        id: run_id,
        stop_handle,
        outcome_future,
        event_stream: Some(event_stream),
        finished: false,
        detached: false,
        _execution_task: execution_task,
    })
}
