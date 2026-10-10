CREATE TABLE projects (
    id BLOB PRIMARY KEY NOT NULL CHECK(length(id) = 16),
    name TEXT NOT NULL COLLATE BINARY UNIQUE CHECK(length(trim(name)) > 0),
    canonical_directory_path TEXT NOT NULL COLLATE BINARY CHECK(length(trim(canonical_directory_path)) > 0),
    filesystem_identity_kind TEXT NOT NULL CHECK(filesystem_identity_kind IN ('unix', 'windows')),
    filesystem_volume_identifier BLOB NOT NULL,
    filesystem_object_identifier BLOB NOT NULL,
    llm_thinking_enabled INTEGER NOT NULL CHECK(llm_thinking_enabled IN (0, 1)),
    llm_context_optimization_enabled INTEGER NOT NULL CHECK(llm_context_optimization_enabled IN (0, 1)),
    cpu_usage_percentage INTEGER NOT NULL CHECK(cpu_usage_percentage BETWEEN 0 AND 100),
    gpu_usage_percentage INTEGER NOT NULL CHECK(gpu_usage_percentage BETWEEN 0 AND 100),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK(
        (filesystem_identity_kind = 'unix'
            AND length(filesystem_volume_identifier) = 8
            AND length(filesystem_object_identifier) = 8)
        OR
        (filesystem_identity_kind = 'windows'
            AND length(filesystem_volume_identifier) = 8
            AND length(filesystem_object_identifier) = 16)
    ),
    UNIQUE(
        filesystem_identity_kind,
        filesystem_volume_identifier,
        filesystem_object_identifier
    )
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
    sequence_number INTEGER NOT NULL CHECK(sequence_number BETWEEN 1 AND 4294967295),
    role TEXT NOT NULL CHECK(role IN ('user', 'llm')),
    contents TEXT NOT NULL,
    user_revision_group_id BLOB CHECK(user_revision_group_id IS NULL OR length(user_revision_group_id) = 16),
    user_revision_number INTEGER CHECK(user_revision_number IS NULL OR user_revision_number BETWEEN 1 AND 4294967295),
    llm_reply_to_user_message_id BLOB CHECK(llm_reply_to_user_message_id IS NULL OR length(llm_reply_to_user_message_id) = 16) REFERENCES messages(id) ON DELETE CASCADE,
    llm_response_round_number INTEGER CHECK(llm_response_round_number IS NULL OR llm_response_round_number BETWEEN 1 AND 4294967295),
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
    position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 4294967295),
    file_name TEXT NOT NULL CHECK(length(trim(file_name)) > 0),
    media_type TEXT CHECK(media_type IS NULL OR length(trim(media_type)) > 0),
    byte_length INTEGER NOT NULL CHECK(byte_length >= 0),
    content_sha256 TEXT CHECK(
        content_sha256 IS NULL
        OR (
            length(content_sha256) = 64
            AND content_sha256 = lower(content_sha256)
            AND content_sha256 NOT GLOB '*[^0-9a-f]*'
        )
    ),
    storage_reference TEXT NOT NULL CHECK(length(trim(storage_reference)) > 0),
    metadata_json TEXT NOT NULL CHECK(json_valid(metadata_json)),
    created_at TEXT NOT NULL,
    UNIQUE(message_id, position)
) STRICT;

CREATE INDEX attachments_message_index ON attachments(message_id, position);

CREATE TABLE llm_actions (
    id BLOB PRIMARY KEY NOT NULL CHECK(length(id) = 16),
    message_id BLOB NOT NULL CHECK(length(message_id) = 16) REFERENCES messages(id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL CHECK(sequence_number BETWEEN 1 AND 4294967295),
    action_kind TEXT NOT NULL CHECK(action_kind IN ('file_change', 'command', 'tool_call')),
    action_details_json TEXT NOT NULL CHECK(
        json_valid(action_details_json)
        AND json_type(action_details_json) IS 'object'
    ),
    summary TEXT CHECK(summary IS NULL OR length(trim(summary)) > 0),
    initial_status TEXT NOT NULL CHECK(initial_status IN ('pending', 'running', 'succeeded', 'failed', 'cancelled')),
    initial_status_payload_json TEXT CHECK(
        initial_status_payload_json IS NULL
        OR json_valid(initial_status_payload_json)
    ),
    initial_status_occurred_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CHECK(
        json_type(action_details_json, '$.kind') IS 'text'
        AND json_extract(action_details_json, '$.kind') = action_kind
    ),
    CHECK(
        action_kind != 'file_change'
        OR (
            json_type(action_details_json, '$.operation') IS 'text'
            AND json_extract(action_details_json, '$.operation') IN ('create', 'modify', 'delete', 'rename')
            AND json_type(action_details_json, '$.source_path') IS 'text'
            AND length(trim(json_extract(action_details_json, '$.source_path'))) > 0
            AND (
                (json_extract(action_details_json, '$.operation') = 'rename'
                    AND json_type(action_details_json, '$.destination_path') IS 'text'
                    AND length(trim(json_extract(action_details_json, '$.destination_path'))) > 0)
                OR (json_extract(action_details_json, '$.operation') != 'rename'
                    AND (
                        json_type(action_details_json, '$.destination_path') IS 'null'
                        OR (json_type(action_details_json, '$.destination_path') IS 'text'
                            AND length(trim(json_extract(action_details_json, '$.destination_path'))) > 0)
                    ))
            )
            AND (json_type(action_details_json, '$.content_before') IS 'null'
                OR json_type(action_details_json, '$.content_before') IS 'text')
            AND (json_type(action_details_json, '$.content_after') IS 'null'
                OR json_type(action_details_json, '$.content_after') IS 'text')
            AND (json_type(action_details_json, '$.unified_diff') IS 'null'
                OR json_type(action_details_json, '$.unified_diff') IS 'text')
            AND json_type(action_details_json, '$.metadata') IS 'object'
        )
    ),
    CHECK(
        action_kind != 'command'
        OR (
            json_type(action_details_json, '$.command_text') IS 'text'
            AND length(trim(json_extract(action_details_json, '$.command_text'))) > 0
            AND json_type(action_details_json, '$.working_directory') IS 'text'
            AND length(trim(json_extract(action_details_json, '$.working_directory'))) > 0
            AND json_type(action_details_json, '$.environment') IS 'object'
        )
    ),
    CHECK(
        action_kind != 'tool_call'
        OR (
            json_type(action_details_json, '$.tool_name') IS 'text'
            AND length(trim(json_extract(action_details_json, '$.tool_name'))) > 0
            AND json_type(action_details_json, '$.arguments') IS 'object'
        )
    ),
    UNIQUE(message_id, sequence_number)
) STRICT;

CREATE INDEX llm_actions_message_index ON llm_actions(message_id, sequence_number);

CREATE TABLE action_status_events (
    id BLOB PRIMARY KEY NOT NULL CHECK(length(id) = 16),
    llm_action_id BLOB NOT NULL CHECK(length(llm_action_id) = 16) REFERENCES llm_actions(id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL CHECK(sequence_number BETWEEN 2 AND 4294967295),
    status TEXT NOT NULL CHECK(status IN ('pending', 'running', 'succeeded', 'failed', 'cancelled')),
    payload_json TEXT CHECK(payload_json IS NULL OR json_valid(payload_json)),
    occurred_at TEXT NOT NULL,
    UNIQUE(llm_action_id, sequence_number)
) STRICT;

CREATE INDEX action_status_events_action_index
ON action_status_events(llm_action_id, sequence_number);

CREATE TRIGGER messages_llm_reply_insert_guard
BEFORE INSERT ON messages
WHEN NEW.role = 'llm'
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1
        FROM messages AS reply_target
        WHERE reply_target.id = NEW.llm_reply_to_user_message_id
          AND reply_target.role = 'user'
          AND reply_target.validity = 'active'
          AND reply_target.chat_id = NEW.chat_id
    ) THEN RAISE(ABORT, 'LLM reply target must be an active user message in the same chat') END;
END;

CREATE TRIGGER messages_llm_reply_update_guard
BEFORE UPDATE OF chat_id, role, llm_reply_to_user_message_id ON messages
WHEN NEW.role = 'llm'
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1
        FROM messages AS reply_target
        WHERE reply_target.id = NEW.llm_reply_to_user_message_id
          AND reply_target.role = 'user'
          AND reply_target.validity = 'active'
          AND reply_target.chat_id = NEW.chat_id
    ) THEN RAISE(ABORT, 'LLM reply target must be an active user message in the same chat') END;
END;

CREATE TRIGGER llm_actions_parent_guard
BEFORE INSERT ON llm_actions
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1
        FROM messages AS parent_message
        WHERE parent_message.id = NEW.message_id
          AND parent_message.role = 'llm'
          AND parent_message.validity = 'active'
          AND parent_message.llm_message_state = 'in_progress'
    ) THEN RAISE(ABORT, 'actions require an active in-progress LLM message') END;
END;

CREATE TRIGGER action_status_events_parent_guard
BEFORE INSERT ON action_status_events
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1
        FROM llm_actions AS parent_action
        JOIN messages AS parent_message ON parent_message.id = parent_action.message_id
        WHERE parent_action.id = NEW.llm_action_id
          AND parent_message.validity = 'active'
          AND parent_message.llm_message_state = 'in_progress'
    ) THEN RAISE(ABORT, 'status events require an active in-progress parent message') END;
    SELECT CASE WHEN NEW.sequence_number != (
        SELECT COALESCE(MAX(existing_event.sequence_number), 1) + 1
        FROM action_status_events AS existing_event
        WHERE existing_event.llm_action_id = NEW.llm_action_id
    ) THEN RAISE(ABORT, 'action status event sequence must be contiguous') END;
    SELECT CASE WHEN NOT (
        (
            COALESCE(
                (SELECT previous_event.status
                 FROM action_status_events AS previous_event
                 WHERE previous_event.llm_action_id = NEW.llm_action_id
                 ORDER BY previous_event.sequence_number DESC
                 LIMIT 1),
                (SELECT parent_action.initial_status
                 FROM llm_actions AS parent_action
                 WHERE parent_action.id = NEW.llm_action_id)
            ) = 'pending'
            AND NEW.status IN ('running', 'cancelled')
        )
        OR (
            COALESCE(
                (SELECT previous_event.status
                 FROM action_status_events AS previous_event
                 WHERE previous_event.llm_action_id = NEW.llm_action_id
                 ORDER BY previous_event.sequence_number DESC
                 LIMIT 1),
                (SELECT parent_action.initial_status
                 FROM llm_actions AS parent_action
                 WHERE parent_action.id = NEW.llm_action_id)
            ) = 'running'
            AND NEW.status IN ('succeeded', 'failed', 'cancelled')
        )
    ) THEN RAISE(ABORT, 'invalid action status transition') END;
END;

CREATE TRIGGER llm_actions_are_immutable
BEFORE UPDATE ON llm_actions
BEGIN
    SELECT RAISE(ABORT, 'LLM actions are append-only');
END;

CREATE TRIGGER llm_actions_delete_requires_parent_deletion
BEFORE DELETE ON llm_actions
WHEN EXISTS (
    SELECT 1 FROM messages WHERE id = OLD.message_id
)
BEGIN
    SELECT RAISE(ABORT, 'LLM actions can only be deleted with their parent message');
END;

CREATE TRIGGER action_status_events_are_immutable
BEFORE UPDATE ON action_status_events
BEGIN
    SELECT RAISE(ABORT, 'action status events are append-only');
END;

CREATE TRIGGER action_status_events_delete_requires_parent_deletion
BEFORE DELETE ON action_status_events
WHEN EXISTS (
    SELECT 1 FROM llm_actions WHERE id = OLD.llm_action_id
)
BEGIN
    SELECT RAISE(ABORT, 'action status events can only be deleted with their action');
END;

CREATE TRIGGER messages_validate_action_aggregates_on_completion
BEFORE UPDATE OF llm_message_state ON messages
WHEN OLD.role = 'llm'
AND OLD.llm_message_state = 'in_progress'
AND NEW.llm_message_state IN ('completed', 'failed', 'cancelled')
BEGIN
    SELECT CASE WHEN EXISTS (
        SELECT 1
        FROM llm_actions AS action
        WHERE action.message_id = OLD.id
          AND COALESCE(
              (SELECT latest_event.status
               FROM action_status_events AS latest_event
               WHERE latest_event.llm_action_id = action.id
               ORDER BY latest_event.sequence_number DESC
               LIMIT 1),
              action.initial_status
          ) NOT IN ('succeeded', 'failed', 'cancelled')
    ) THEN RAISE(ABORT, 'LLM messages cannot finish with incomplete action records') END;
END;

CREATE TRIGGER messages_deprecation_requires_terminal_actions
BEFORE UPDATE OF validity ON messages
WHEN OLD.validity = 'active' AND NEW.validity = 'deprecated'
BEGIN
    SELECT CASE WHEN EXISTS (
        SELECT 1
        FROM llm_actions AS action
        WHERE action.message_id = OLD.id
          AND COALESCE(
              (SELECT latest_event.status
               FROM action_status_events AS latest_event
               WHERE latest_event.llm_action_id = action.id
               ORDER BY latest_event.sequence_number DESC
               LIMIT 1),
              action.initial_status
          ) NOT IN ('succeeded', 'failed', 'cancelled')
    ) THEN RAISE(ABORT, 'messages cannot be deprecated with unfinished actions') END;
END;
