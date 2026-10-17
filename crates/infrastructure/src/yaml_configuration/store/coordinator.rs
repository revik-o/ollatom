use super::{ConfigurationCommitRegistry, ConfigurationCoordinator};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, Weak};
use tokio::sync::Mutex as AsyncMutex;

static CONFIGURATION_COORDINATORS: LazyLock<Mutex<HashMap<PathBuf, Weak<ConfigurationCoordinator>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn configuration_coordinator(configuration_path: PathBuf) -> Arc<ConfigurationCoordinator> {
    let mut coordinators = CONFIGURATION_COORDINATORS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    if let Some(coordinator) = coordinators.get(&configuration_path).and_then(Weak::upgrade) {
        return coordinator;
    }

    let coordinator = Arc::new(ConfigurationCoordinator {
        mutation_lock: Arc::new(AsyncMutex::new(())),
        commit_registry: Arc::new(ConfigurationCommitRegistry::default()),
    });

    coordinators.insert(configuration_path, Arc::downgrade(&coordinator));

    coordinator
}
