use crate::{LlmError, LlmRunOutcome, RunEventStream, RunId, StopHandle};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub struct LlmRun {
    pub(crate) id: RunId,
    pub(crate) stop_handle: StopHandle,
    pub(crate) outcome_future:
        Pin<Box<dyn Future<Output = Result<LlmRunOutcome, LlmError>> + Send>>,
    pub(crate) event_stream: Option<RunEventStream>,
    pub(crate) finished: bool,
    pub(crate) detached: bool,
}

impl LlmRun {
    pub fn id(&self) -> RunId {
        self.id
    }

    pub fn stop_handle(&self) -> StopHandle {
        self.stop_handle.clone()
    }

    pub fn event_stream(&mut self) -> Option<&mut RunEventStream> {
        self.event_stream.as_mut()
    }

    pub fn take_event_stream(&mut self) -> Option<RunEventStream> {
        self.event_stream.take()
    }

    pub fn detach(mut self) -> Result<RunId, LlmError> {
        let handle = tokio::runtime::Handle::try_current()
            .map_err(|_| LlmError::InvalidRequest("detach requires a Tokio runtime".into()))?;
        self.detached = true;

        let run_id = self.id;
        handle.spawn(async move {
            let _ = (&mut self).await;
        });

        Ok(run_id)
    }

    fn finish(&mut self) {
        if !self.finished {
            self.finished = true;
        }
    }
}

impl Future for LlmRun {
    type Output = Result<LlmRunOutcome, LlmError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        match self.outcome_future.as_mut().poll(context) {
            Poll::Ready(output) => {
                self.finish();
                Poll::Ready(output)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for LlmRun {
    fn drop(&mut self) {
        if !self.finished && !self.detached {
            self.stop_handle.stop();
        }
    }
}
