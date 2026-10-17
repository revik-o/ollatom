mod support;

use infrastructure::{
    ExecutionSettingsUpdate, InfrastructureErrorKind, ProjectId, ProjectInitializationParameters, ProjectUpdateOptions,
};
use std::path::{Component, Path, PathBuf};
use support::{TestInfrastructure, create_initialized_test_infrastructure};

fn create_project_directory(test_infrastructure: &TestInfrastructure, name: &str) -> PathBuf {
    let project_path = test_infrastructure.temporary_directory.path().join(name);
    std::fs::create_dir(&project_path).unwrap();
    project_path
}

fn relative_path_from_current_directory(target_path: &Path) -> PathBuf {
    let current_directory = std::env::current_dir().unwrap();
    let current_components = current_directory.components().collect::<Vec<_>>();
    let target_components = target_path.components().collect::<Vec<_>>();
    let shared_component_count = current_components
        .iter()
        .zip(&target_components)
        .take_while(|(current, target)| current == target)
        .count();
    let mut relative_path = PathBuf::new();
    for _ in shared_component_count..current_components.len() {
        relative_path.push("..");
    }
    for component in &target_components[shared_component_count..] {
        if let Component::Normal(component) = component {
            relative_path.push(component);
        }
    }
    relative_path
}

#[path = "project_persistence/path_identity.rs"]
mod path_identity;
#[path = "project_persistence/project_lifecycle.rs"]
mod project_lifecycle;
#[path = "project_persistence/validation.rs"]
mod validation;
