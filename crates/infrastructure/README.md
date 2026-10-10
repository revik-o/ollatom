# Infrastructure

The infrastructure crate owns persistent application data, SQLite initialization, typed transaction operations, and YAML configuration storage.

Every SQLite database is initialized from an explicit absolute file path. The caller creates the parent directory and chooses the filename. Initialization enables foreign keys, WAL journal mode, `FULL` synchronous mode by default, a five-second busy timeout, and embedded migrations. Applications that explicitly choose lower durability can pass `DatabaseDurability::Balanced`, which uses `NORMAL` synchronous mode.

On Unix, the database parent directory must be owned by the current user with mode `0700`; SQLite files must be regular files owned by that user with mode `0600`. Initialization creates new database files with those permissions and rejects insecure existing files. Initialization fails closed on platforms without private database ACL enforcement.

Domain mutations require an `InfrastructureTransaction`. Manual transactions commit or roll back explicitly. `execute_db_actions` commits successful closures and rolls back failed closures. A transaction containing a failed operation cannot commit.

User revision numbers and LLM response round numbers are separate concepts. User revisions identify edits to one logical request. LLM response rounds identify multiple responses to one exact active user-message revision.

SQL access is private to the infrastructure crate. Public operations use typed repositories and parameterized statements; callers cannot submit raw SQL or choose arbitrary table and column names.

## Public API Overview

### 1. Database Initialization and Connection

```rust
use infrastructure::{DatabaseConfiguration, DatabaseDurability, Infrastructure};
use std::path::Path;

let infra = Infrastructure::init(Path::new("/path/to/database.db")).await?;

let balanced_infra = Infrastructure::init_with_configuration(
    Path::new("/path/to/secondary-database.db"),
    DatabaseConfiguration {
        durability: DatabaseDurability::Balanced,
    },
).await?;

let mut transaction = infra.make_transaction().await?;
transaction.commit().await?;

let project = infra.execute_db_actions(async |transaction| {
    transaction.get_project_by_name("ollatom").await
}).await?;

let existing_project = infra.get_project_by_path("/path/to/ollatom").await?;
```
**Description:** This shows how to establish a connection pool and execute queries. `Infrastructure::init` connects via `sqlx` and runs DB migrations. Manual transactions can be created with `make_transaction`, which maps to `BEGIN IMMEDIATE` under the hood. Alternatively, `execute_db_actions` accepts an async closure, automatically issuing `commit()` if successful or `rollback()` upon errors. Read-only path lookups use a deferred transaction.


### 2. Project API

```rust
use infrastructure::{ProjectInitializationParameters, ProjectUpdateOptions};

let project = transaction.create_project(
    "My Project",
    "/path/to/project",
    ProjectInitializationParameters::default(),
).await?;

transaction.update_project(
    ProjectUpdateOptions::new(project.id)
        .with_name("Renamed Project")
        .with_cpu_usage_percentage(80)
).await?;

transaction.set_llm_thinking_for_project(true, project.id).await?;

transaction.delete_project(project.id).await?;
```
**Description:** Register an existing directory as a project. The crate stores its canonical UTF-8 path and physical filesystem identity, so relative paths and symlinks resolve to the same project. Updating a project's path requires the new path to identify the same directory; register a new project to use a different directory.

### 3. Chat API

```rust
use infrastructure::ChatInitializationParameters;

let chat = transaction.create_chat(
    "General Discussion",
    project.id,
    ChatInitializationParameters::default(),
).await?;

transaction.set_llm_context_optimization_for_chat(true, chat.id).await?;

transaction.delete_chat(chat.id).await?;
```
**Description:** Demonstrates creating and manipulating chat rooms. Under the hood, these methods perform parameterized SQL executions against the `chats` table. The transaction ensures that chat records remain structurally tied to their parent project.


### 4. Message & LLM Action API

```rust
use infrastructure::{LlmActionDetails, CommandActionDetails, LlmActionStatusEventInput, LlmActionStatus};
use serde_json::json;

let user_message = transaction.add_message_from_user(
    "Can you create a hello world app?",
    vec![],
    chat.id,
).await?;

let llm_message = transaction.begin_llm_message_by_chat_id_and_user_message_id(
    chat.id,
    user_message.id,
).await?;

let action = transaction.add_llm_action_to_message_by_id(
    llm_message.id,
    Some("Running a terminal command".to_string()),
    LlmActionDetails::Command(CommandActionDetails {
        command_text: "echo 'Hello World'".into(),
        working_directory: "/path".into(),
        environment: Default::default(),
    }),
    LlmActionStatusEventInput {
        status: LlmActionStatus::Running,
        payload: None,
    }
).await?;

transaction.append_llm_action_status_event(
    action.id,
    LlmActionStatusEventInput {
        status: LlmActionStatus::Succeeded,
        payload: Some(json!({"stdout": "Hello World\n"})),
    }
).await?;

transaction.complete_llm_message_by_id(
    llm_message.id,
    "I have executed the hello world command successfully.",
).await?;
```
**Description:** Demonstrates the end-to-end conversation flow. User and LLM messages receive independent revision and response-round metadata. Each action stores its kind-specific JSON details and initial status in one immutable row; later status changes append events starting at sequence 2. `complete_llm_message_by_id` requires terminal action states before completing the message.


### 5. YAML Configuration Store (`yaml_configuration`)

```rust
use infrastructure::{create_json_configuration_store, create_typed_yaml_file};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, Serialize)]
struct AppConfig {
    network_port: u16,
}

let settings = create_json_configuration_store(
    "settings.yaml",
    "/path/to/config/dir",
).await?;

let port = settings.read_parameter("network.port").await?;
let commit = settings.clone().add_parameter("network.port", json!(8080))?
    .add_parameter("ui.theme", json!("dark"))?
    .commit()?;
let operation_identifier = commit.operation_identifier();
let status = commit.wait().await;

let typed_settings = create_typed_yaml_file::<AppConfig>(
    "settings.yaml",
    "/path/to/config/dir",
).await?;
let configuration = typed_settings.read().await?;
typed_settings.write(&configuration).await?;
```
**Description:** `JsonConfigurationStore` supports a JSON-compatible mapping and dotted keys. Its updates reread the current file under a shared process coordinator and sibling OS advisory lock, then write atomically. `commit()` returns an operation handle before completion; `wait()` observes and acknowledges its result. `TypedYamlFile<Configuration>` reads and writes full typed YAML values, including scalar and sequence roots, without converting through JSON.
