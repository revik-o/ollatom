use tokio::sync::watch;

#[derive(Clone, Debug)]
pub struct StopToken {
    receiver: watch::Receiver<bool>,
}

#[derive(Clone, Debug)]
pub struct StopHandle {
    sender: watch::Sender<bool>,
}

impl StopToken {
    pub fn is_stopped(&self) -> bool {
        *self.receiver.borrow()
    }

    pub async fn cancelled(&self) {
        if self.is_stopped() {
            return;
        }

        let mut receiver = self.receiver.clone();

        while receiver.changed().await.is_ok() {
            if *receiver.borrow() {
                return;
            }
        }
    }
}

impl StopHandle {
    pub fn stop(&self) {
        self.sender.send_if_modified(|stopped| {
            if *stopped {
                false
            } else {
                *stopped = true;
                true
            }
        });
    }

    pub fn is_stopped(&self) -> bool {
        *self.sender.borrow()
    }
}
pub(crate) fn stop_pair() -> (StopToken, StopHandle) {
    let (sender, receiver) = watch::channel(false);

    (StopToken { receiver }, StopHandle { sender })
}

pub(crate) struct StopOnDrop(StopHandle);

impl StopOnDrop {
    pub fn new(handle: StopHandle) -> Self {
        Self(handle)
    }

    pub fn stop(&self) {
        self.0.stop();
    }
}

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.stop();
    }
}
