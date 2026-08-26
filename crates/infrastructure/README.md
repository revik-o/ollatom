# Infrastructure

The infrastructure crate owns persistent application data, SQLite initialization, transaction boundaries, the trusted SQL builder, and YAML configuration storage.

Every SQLite database is initialized from an explicit absolute file path. The caller creates the parent directory and chooses the filename. Initialization enables foreign keys, WAL journal mode, NORMAL synchronous mode, a five-second busy timeout, and embedded migrations.

Domain mutations require an `InfrastructureTransaction`. Manual transactions commit or roll back explicitly. `execute_db_actions` commits successful closures and rolls back failed closures. A transaction containing a failed operation cannot commit.

User revision numbers and LLM response round numbers are separate concepts. User revisions identify edits to one logical request. LLM response rounds identify multiple responses to one exact active user-message revision.

The SQL builder is a trusted application and plugin API. Values are bound parameters, identifiers are validated, unfiltered updates and deletes require explicit authorization, and schema changes remain controlled by migrations.

## Public API Overview

### 1. Database Initialization and Connection

```rust
use infrastructure::Infrastructure;
use std::path::Path;

let infra = Infrastructure::init(Path::new("/path/to/database.db")).await?;

let mut transaction = infra.make_transaction().await?;
transaction.commit().await?;

let project = infra.execute_db_actions(async |transaction| {
    transaction.get_project_by_name("ollatom").await
}).await?;

let existing_project = infra.get_project_by_path("/path/to/ollatom").await?;
```
**Description:** This shows how to establish a connection pool and execute queries. `Infrastructure::init` connects via `sqlx` and runs DB migrations. Manual transactions can be created with `make_transaction`, which maps to `BEGIN IMMEDIATE` under the hood. Alternatively, `execute_db_actions` accepts an async closure, automatically issuing `commit()` if successful or `rollback()` upon errors. Simple read-only operations can bypass transactions completely.


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

transaction.set_llm_thinking_for_project_by_id(true, project.id).await?;

transaction.delete_project_by_id(project.id).await?;
```
**Description:** Demonstrates the CRUD lifecycle for projects. Under the hood, these methods safely bind variables to execute `INSERT INTO`, `UPDATE`, and `DELETE FROM` statements in the `projects` table using the active `InfrastructureTransaction`.

### 3. Chat API

```rust
use infrastructure::ChatInitializationParameters;

let chat = transaction.create_chat_by_project_id(
    "General Discussion",
    project.id,
    ChatInitializationParameters::default(),
).await?;

transaction.set_llm_context_optimization_for_chat_by_id(true, chat.id).await?;

transaction.delete_chat_by_id(chat.id).await?;
```
**Description:** Demonstrates creating and manipulating chat rooms. Under the hood, these methods perform parameterized SQL executions against the `chats` table. The transaction ensures that chat records remain structurally tied to their parent project.


### 4. Message & LLM Action API

```rust
use infrastructure::{AttachmentInput, LlmActionDetails, CommandActionDetails, LlmActionStatusEventInput, LlmActionStatus};
use serde_json::json;

let user_message = transaction.add_message_from_user_by_chat_id(
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
**Description:** Demonstrates the end-to-end conversation flow. User and LLM messages receive independent revision and response-round metadata. LLM actions are stored in their journal and detail tables, while status updates are append-only events. Finally, `complete_llm_message_by_id` validates terminal action states and switches the message to `Completed`.


### 5. `SqlBuilderFactory` (Custom Query Builder)

```rust
use infrastructure::SqlValue;

let builder = infra.sql_builder();
let rows = builder
    .select(["id", "name", "path"])
    .from("projects")
    .filter("name = {}", [SqlValue::from("My Project")])
    .order_by("created_at", infrastructure::SqlSortDirection::Descending)
    .limit(1)
    .commit()
    .await?;

let mut transaction = infra.make_transaction().await?;
transaction.sql_builder()
    .update("projects")
    .set("cpu_usage_percentage", SqlValue::from(50_i64))
    .filter("id = {}", [SqlValue::from(project.id.as_uuid())])
    .execute()
    .await?;
```
**Description:** Demonstrates dynamically built queries without interpolating values. `commit` opens an internal transaction for infrastructure-bound builders. `fetch_all`, `fetch_optional`, `fetch_one`, and `execute` are available on transaction-bound builders and participate in the caller's transaction.


### 6. YAML Configuration Store (`yaml_configuration`)

```rust
use infrastructure::create_yaml_configuration_file;
use serde_json::json;

let config_store = create_yaml_configuration_file(
    "settings.yaml", 
    "/path/to/config/dir"
).await?;

let port_value = config_store.read_parameter("network.port").await?;

config_store.create_update()
    .add_parameter("network.port", json!(8080))?
    .add_parameter("ui.theme", json!("dark"))?
    .commit()
    .await?;
```
**Description:** Demonstrates reading and modifying a YAML configuration file. Under the hood, this API acts as a client to a dedicated `tokio` worker thread. Creating the update map gathers the desired changes; calling `commit()` flushes those changes across an `mpsc` channel to the worker, ensuring file modifications occur synchronously without OS race conditions.
