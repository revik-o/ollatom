CREATE TABLE projects (
    id BLOB PRIMARY KEY NOT NULL CHECK(length(id) = 16),
    name TEXT NOT NULL COLLATE BINARY UNIQUE CHECK(length(trim(name)) > 0),
    path TEXT NOT NULL COLLATE BINARY UNIQUE CHECK(length(trim(path)) > 0),
    llm_thinking_enabled INTEGER NOT NULL CHECK(llm_thinking_enabled IN (0, 1)),
    llm_context_optimization_enabled INTEGER NOT NULL CHECK(llm_context_optimization_enabled IN (0, 1)),
    cpu_usage_percentage INTEGER NOT NULL CHECK(cpu_usage_percentage BETWEEN 0 AND 100),
    gpu_usage_percentage INTEGER NOT NULL CHECK(gpu_usage_percentage BETWEEN 0 AND 100),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE chats (
    id BLOB PRIMARY KEY NOT NULL CHECK(length(id) = 16),
    project_id BLOB NOT NULL CHECK(length(project_id) = 16) REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL COLLATE BINARY CHECK(length(trim(name)) > 0),
    llm_thinking_enabled INTEGER NOT NULL CHECK(llm_thinking_enabled IN (0, 1)),
    llm_context_optimization_enabled INTEGER NOT NULL CHECK(llm_context_optimization_enabled IN (0, 1)),
    cpu_usage_percentage INTEGER NOT NULL CHECK(cpu_usage_percentage BETWEEN 0 AND 100),
    gpu_usage_percentage INTEGER NOT NULL CHECK(gpu_usage_percentage BETWEEN 0 AND 100),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(project_id, name)
) STRICT;

CREATE TABLE messages (
    id BLOB PRIMARY KEY NOT NULL CHECK(length(id) = 16),
    chat_id BLOB NOT NULL CHECK(length(chat_id) = 16) REFERENCES chats(id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL CHECK(sequence_number >= 1),
    role TEXT NOT NULL CHECK(role IN ('user', 'llm')),
    contents TEXT NOT NULL,
    user_revision_group_id BLOB CHECK(user_revision_group_id IS NULL OR length(user_revision_group_id) = 16),
    user_revision_number INTEGER CHECK(user_revision_number IS NULL OR user_revision_number >= 1),
    llm_reply_to_user_message_id BLOB CHECK(llm_reply_to_user_message_id IS NULL OR length(llm_reply_to_user_message_id) = 16) REFERENCES messages(id) ON DELETE CASCADE,
    llm_response_round_number INTEGER CHECK(llm_response_round_number IS NULL OR llm_response_round_number >= 1),
    llm_message_state TEXT CHECK(llm_message_state IS NULL OR llm_message_state IN ('in_progress', 'completed', 'failed', 'cancelled')),
    validity TEXT NOT NULL CHECK(validity IN ('active', 'deprecated')),
    created_at TEXT NOT NULL,
    updated_at TEXT,
    deprecated_at TEXT,
    CHECK(
        (
            role = 'user'
            AND user_revision_group_id IS NOT NULL
            AND user_revision_number IS NOT NULL
            AND llm_reply_to_user_message_id IS NULL
            AND llm_response_round_number IS NULL
            AND llm_message_state IS NULL
        )
        OR
        (
            role = 'llm'
            AND user_revision_group_id IS NULL
            AND user_revision_number IS NULL
            AND llm_reply_to_user_message_id IS NOT NULL
            AND llm_response_round_number IS NOT NULL
            AND llm_message_state IS NOT NULL
        )
    ),
    CHECK(
        (validity = 'active' AND deprecated_at IS NULL)
        OR
        (validity = 'deprecated' AND deprecated_at IS NOT NULL)
    )
) STRICT;

CREATE UNIQUE INDEX messages_active_sequence_index
ON messages(chat_id, sequence_number)
WHERE validity = 'active';

CREATE UNIQUE INDEX messages_user_revision_index
ON messages(user_revision_group_id, user_revision_number)
WHERE role = 'user';

CREATE UNIQUE INDEX messages_llm_response_round_index
ON messages(llm_reply_to_user_message_id, llm_response_round_number)
WHERE role = 'llm';

CREATE INDEX messages_active_chat_history_index
ON messages(chat_id, validity, sequence_number);

CREATE TABLE attachments (
    id BLOB PRIMARY KEY NOT NULL CHECK(length(id) = 16),
    message_id BLOB NOT NULL CHECK(length(message_id) = 16) REFERENCES messages(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK(position >= 0),
    file_name TEXT NOT NULL CHECK(length(trim(file_name)) > 0),
    media_type TEXT,
    byte_length INTEGER NOT NULL CHECK(byte_length >= 0),
    content_sha256 TEXT,
    storage_reference TEXT NOT NULL CHECK(length(trim(storage_reference)) > 0),
    metadata_json TEXT NOT NULL CHECK(json_valid(metadata_json)),
    created_at TEXT NOT NULL,
    UNIQUE(message_id, position)
) STRICT;

CREATE INDEX attachments_message_index ON attachments(message_id, position);

CREATE TABLE llm_actions (
    id BLOB PRIMARY KEY NOT NULL CHECK(length(id) = 16),
    message_id BLOB NOT NULL CHECK(length(message_id) = 16) REFERENCES messages(id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL CHECK(sequence_number >= 1),
    action_kind TEXT NOT NULL CHECK(action_kind IN ('file_change', 'command', 'tool_call')),
    summary TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(message_id, sequence_number)
) STRICT;

CREATE INDEX llm_actions_message_index ON llm_actions(message_id, sequence_number);

CREATE TABLE action_status_events (
    id BLOB PRIMARY KEY NOT NULL CHECK(length(id) = 16),
    llm_action_id BLOB NOT NULL CHECK(length(llm_action_id) = 16) REFERENCES llm_actions(id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL CHECK(sequence_number >= 1),
    status TEXT NOT NULL CHECK(status IN ('pending', 'running', 'succeeded', 'failed', 'cancelled')),
    payload_json TEXT CHECK(payload_json IS NULL OR json_valid(payload_json)),
    occurred_at TEXT NOT NULL,
    UNIQUE(llm_action_id, sequence_number)
) STRICT;

CREATE INDEX action_status_events_action_index
ON action_status_events(llm_action_id, sequence_number);

CREATE TABLE file_action_details (
    llm_action_id BLOB PRIMARY KEY NOT NULL CHECK(length(llm_action_id) = 16) REFERENCES llm_actions(id) ON DELETE CASCADE,
    operation TEXT NOT NULL CHECK(operation IN ('create', 'modify', 'delete', 'rename')),
    source_path TEXT NOT NULL,
    destination_path TEXT,
    content_before TEXT,
    content_after TEXT,
    unified_diff TEXT,
    metadata_json TEXT NOT NULL CHECK(json_valid(metadata_json))
) STRICT;

CREATE TABLE command_action_details (
    llm_action_id BLOB PRIMARY KEY NOT NULL CHECK(length(llm_action_id) = 16) REFERENCES llm_actions(id) ON DELETE CASCADE,
    command_text TEXT NOT NULL,
    working_directory TEXT NOT NULL,
    environment_json TEXT NOT NULL CHECK(json_valid(environment_json))
) STRICT;

CREATE TABLE tool_call_action_details (
    llm_action_id BLOB PRIMARY KEY NOT NULL CHECK(length(llm_action_id) = 16) REFERENCES llm_actions(id) ON DELETE CASCADE,
    tool_name TEXT NOT NULL,
    arguments_json TEXT NOT NULL CHECK(json_valid(arguments_json))
) STRICT;
