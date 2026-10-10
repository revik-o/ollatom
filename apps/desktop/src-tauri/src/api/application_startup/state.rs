use super::*;

impl StartupState {
    pub(super) fn new(snapshot: StartupSnapshot) -> Self {
        Self {
            resources: Mutex::new(None),
            resources_ready: Notify::new(),
            snapshot: Mutex::new(snapshot),
            interactive_regions: Mutex::new(Vec::new()),
        }
    }

    pub async fn resources(&self) -> Result<StartupResources, String> {
        loop {
            let notified = self.resources_ready.notified();
            let resources = self
                .resources
                .lock()
                .map_err(|_| "startup state is unavailable")?
                .clone();

            if let Some(value) = resources {
                return value;
            }

            notified.await;
        }
    }

    pub(super) fn complete_resources(&self, value: Result<StartupResources, String>) {
        let Ok(mut resources) = self.resources.lock() else {
            return;
        };

        if resources.is_some() {
            return;
        }

        *resources = Some(value);
        self.resources_ready.notify_waiters();
    }

    pub(super) fn snapshot(&self) -> Result<StartupSnapshot, String> {
        self.snapshot
            .lock()
            .map(|value| value.clone())
            .map_err(|_| "window startup state is unavailable".to_owned())
    }

    pub(super) fn update_backdrop(
        &self,
        backdrop: BackdropKind,
    ) -> Result<StartupSnapshot, String> {
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| "window startup state is unavailable".to_owned())?;

        snapshot.backdrop = backdrop;

        Ok(snapshot.clone())
    }

    #[cfg(target_os = "linux")]
    pub(super) fn update_linux_controls(
        &self,
        controls_side: WindowControlsSide,
        controls: Vec<String>,
    ) -> Result<StartupSnapshot, String> {
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| "window startup state is unavailable".to_owned())?;

        snapshot.chrome.controls_side = controls_side;
        snapshot.chrome.controls = controls;

        Ok(snapshot.clone())
    }
}
