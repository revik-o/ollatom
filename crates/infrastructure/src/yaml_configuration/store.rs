use filesystem::FilePointer;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::{Mutex as AsyncMutex, watch};

pub(crate) mod commit;
mod coordinator;
mod factory;
mod file;
mod json;
mod typed;

pub use factory::{create_json_configuration_store, create_typed_yaml_file};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ConfigurationCommitIdentifier(u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigurationCommitStatus {
    Pending,
    Succeeded,
    Failed(String),
}

#[derive(Debug, Default)]
pub(crate) struct ConfigurationCommitRegistry {
    statuses:
        Mutex<HashMap<ConfigurationCommitIdentifier, watch::Receiver<ConfigurationCommitStatus>>>,
}

#[derive(Debug)]
struct ConfigurationCoordinator {
    mutation_lock: Arc<AsyncMutex<()>>,
    commit_registry: Arc<ConfigurationCommitRegistry>,
}
#[derive(Clone, Debug)]
struct ConfigurationFile {
    file_pointer: FilePointer,
    file_path: PathBuf,
    lock_file_path: PathBuf,
    coordinator: Arc<ConfigurationCoordinator>,
}

#[derive(Clone, Debug)]
pub struct JsonConfigurationStore {
    file: ConfigurationFile,
}

pub struct TypedYamlFile<Configuration> {
    file: ConfigurationFile,
    configuration_marker: PhantomData<Configuration>,
}

impl<Configuration> Clone for TypedYamlFile<Configuration> {
    fn clone(&self) -> Self {
        Self {
            file: self.file.clone(),
            configuration_marker: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct JsonConfigurationUpdate {
    configuration_store: JsonConfigurationStore,
    parameters: BTreeMap<String, Value>,
}

pub struct ConfigurationCommitHandle {
    operation_identifier: ConfigurationCommitIdentifier,
    completion_receiver: watch::Receiver<ConfigurationCommitStatus>,
    coordinator: Arc<ConfigurationCoordinator>,
}
