# Requirements Document

## Introduction

チャットセッションごとにテキストファイルを「ナレッジ」として管理するビルトインプラグイン。ユーザーがドロップしたファイルの内容をスナップショットとしてDB内部に保存し、セッション中のAI応答に活用する。注入モードとして「常時注入（system_prompt）」と「参照型（tool_reference）」の2種を提供し、ファイルごとに切り替え可能。

## Glossary

- **Knowledge_Plugin**: ナレッジ管理機能を提供するビルトインプラグイン（PluginHandler trait実装）
- **Knowledge_Entry**: DB上に保存された1件のナレッジファイルレコード（file_name, content, injection_mode等を保持）
- **Injection_Mode**: ナレッジの注入方式。system_prompt（常時注入）またはtool_reference（参照型）のいずれか
- **System_Prompt_Mode**: 毎ターン、有効なナレッジ全文をシステムプロンプトに埋め込む注入方式
- **Tool_Reference_Mode**: get_knowledge ツールをAIに公開し、AIがtool_callで必要時に参照する注入方式
- **Drop_Zone**: ToolManagementPane内のナレッジセクションに表示されるファイルドロップ領域
- **ToolManagementPane**: チャット画面右側に表示されるツール管理パネル（既存コンポーネント）
- **Session**: 1つのチャットセッション。ナレッジはセッション単位で管理される
- **Engine**: チャット処理エンジン（engine.rs）。LLMへのメッセージ構築とツール実行を担当

## Requirements

### Requirement 1: ナレッジの追加（スナップショット保存）

**User Story:** As a user, I want to drop a text file into the knowledge section, so that the file content is stored as a snapshot for the current chat session.

#### Acceptance Criteria

1. WHEN a user drops a file onto the Drop_Zone, THE Knowledge_Plugin SHALL read the file content as UTF-8 text and store it as a Knowledge_Entry in the session_knowledge table with the current session_id, accepting files up to 512 KB in size
2. THE Knowledge_Plugin SHALL store the file content as a point-in-time snapshot without maintaining synchronization with the original file
3. WHEN a user drops a file with the same file_name as an existing Knowledge_Entry in the same session, THE Knowledge_Plugin SHALL replace the existing entry content with the new file content and update the size_bytes and created_at fields
4. WHEN a file is successfully added, THE Knowledge_Plugin SHALL record the file_name (basename without directory path), content, size_bytes, and created_at timestamp in the Knowledge_Entry
5. THE Knowledge_Plugin SHALL set newly added Knowledge_Entry records to enabled=true and injection_mode=system_prompt as defaults
6. IF a dropped file exceeds 512 KB or cannot be read as valid UTF-8 text, THEN THE Knowledge_Plugin SHALL reject the file and display an error message indicating the reason for rejection without creating a Knowledge_Entry

### Requirement 2: ナレッジの削除

**User Story:** As a user, I want to remove a knowledge entry from the session, so that it no longer affects AI responses.

#### Acceptance Criteria

1. WHEN a user clicks the delete button for a Knowledge_Entry, THE Knowledge_Plugin SHALL display a confirmation dialog before proceeding with deletion
2. WHEN the user confirms deletion of a Knowledge_Entry, THE Knowledge_Plugin SHALL remove the corresponding record from the session_knowledge table using the session_id and file_name as identifiers
3. WHEN a Knowledge_Entry is removed, THE Engine SHALL exclude the removed entry from subsequent LLM requests in both System_Prompt_Mode injection and Tool_Reference_Mode tool availability
4. IF the user cancels the deletion confirmation, THEN THE Knowledge_Plugin SHALL retain the Knowledge_Entry unchanged
5. IF a delete operation targets a Knowledge_Entry that does not exist in the session_knowledge table, THEN THE Knowledge_Plugin SHALL return an error message indicating the entry was not found

### Requirement 3: ナレッジの有効/無効切り替え

**User Story:** As a user, I want to toggle a knowledge entry on or off, so that I can temporarily exclude it from AI context without deleting it.

#### Acceptance Criteria

1. WHEN a user toggles a Knowledge_Entry to disabled, THE Knowledge_Plugin SHALL set enabled=false for that entry and persist the change to the database immediately
2. WHILE a Knowledge_Entry has enabled=false, THE Engine SHALL exclude that entry from LLM request construction regardless of Injection_Mode
3. WHEN a user toggles a Knowledge_Entry to enabled, THE Knowledge_Plugin SHALL set enabled=true and persist the change to the database immediately
4. WHEN a Knowledge_Entry is toggled to enabled, THE Engine SHALL include that entry in the next LLM request constructed after the toggle, according to its current Injection_Mode
5. IF a toggle operation targets a Knowledge_Entry that does not exist in the session, THEN THE Knowledge_Plugin SHALL return an error indicating the entry was not found

### Requirement 4: 注入モード切り替え

**User Story:** As a user, I want to switch the injection mode of each knowledge entry between system_prompt and tool_reference, so that I can control how the AI accesses the knowledge.

#### Acceptance Criteria

1. WHEN a user changes the Injection_Mode of a Knowledge_Entry to system_prompt, THE Knowledge_Plugin SHALL update the injection_mode field to "system_prompt" and display the updated mode in the UI within 1 second
2. WHEN a user changes the Injection_Mode of a Knowledge_Entry to tool_reference, THE Knowledge_Plugin SHALL update the injection_mode field to "tool_reference" and display the updated mode in the UI within 1 second
3. WHEN a user selects a new Injection_Mode value, THE Knowledge_Plugin SHALL persist the change to the database within 2 seconds of the user's selection
4. IF the Injection_Mode change fails to persist to the database, THEN THE Knowledge_Plugin SHALL revert the displayed Injection_Mode to the previous value and display an error message indicating the persistence failure

### Requirement 5: System_Prompt_Mode による注入

**User Story:** As a user, I want knowledge entries in system_prompt mode to be automatically included in every AI turn, so that the AI always has access to that context.

#### Acceptance Criteria

1. WHILE one or more Knowledge_Entry records have enabled=true and injection_mode=system_prompt, THE Engine SHALL concatenate their full content in created_at ascending order and inject the result into the system prompt for every LLM request in that session
2. THE Engine SHALL format each injected knowledge block with a markdown heading containing the file_name (e.g., "## {file_name}") followed by the entry content, separated by a blank line between blocks
3. THE Engine SHALL append the concatenated knowledge section after the base system prompt content and before any other dynamic context (thoughts, memories)
4. WHILE no Knowledge_Entry records have enabled=true and injection_mode=system_prompt, THE Engine SHALL not add any knowledge section to the system prompt

### Requirement 6: Tool_Reference_Mode による注入

**User Story:** As a user, I want knowledge entries in tool_reference mode to be accessible via a get_knowledge tool, so that the AI can retrieve them on demand without consuming prompt space every turn.

#### Acceptance Criteria

1. WHILE one or more Knowledge_Entry records have enabled=true and injection_mode=tool_reference in a session, THE Engine SHALL include a get_knowledge tool in the tool definitions sent to the LLM, with a description that explains its purpose and a required file_name string parameter whose description lists the currently available knowledge file names
2. WHEN the LLM calls get_knowledge with a file_name argument that exactly matches (case-sensitive) the file_name of an enabled tool_reference Knowledge_Entry in the current session, THE Knowledge_Plugin SHALL return the full content of that Knowledge_Entry
3. IF the LLM calls get_knowledge with a file_name that does not exactly match any enabled tool_reference Knowledge_Entry in the current session, THEN THE Knowledge_Plugin SHALL return an error message indicating no match was found, followed by a list of currently available knowledge file names
4. WHILE no Knowledge_Entry records have enabled=true and injection_mode=tool_reference in a session, THE Engine SHALL not include the get_knowledge tool in tool definitions
5. WHEN a Knowledge_Entry's enabled flag or injection_mode is changed during a session, THE Engine SHALL update the get_knowledge tool's parameter description to reflect the current set of available file names on the next LLM request

### Requirement 7: ナレッジのエクスポート

**User Story:** As a user, I want to export a knowledge entry to a local file, so that I can retrieve the stored snapshot content.

#### Acceptance Criteria

1. WHEN a user clicks the export button for a Knowledge_Entry, THE Knowledge_Plugin SHALL open the system file dialog with the entry's file_name as the default file name, and upon user confirmation write the entry content to the selected path
2. IF the user cancels the file dialog during export, THEN THE Knowledge_Plugin SHALL abort the export operation without writing any file and without displaying an error
3. IF the file write fails during export, THEN THE Knowledge_Plugin SHALL display an error notification indicating the failure reason without modifying the Knowledge_Entry

### Requirement 8: ナレッジ一覧の表示

**User Story:** As a user, I want to see all knowledge entries for the current session in the ToolManagementPane, so that I can manage them.

#### Acceptance Criteria

1. THE ToolManagementPane SHALL display the Knowledge_Plugin as an accordion item alongside other plugins
2. WHEN the Knowledge_Plugin accordion is expanded, THE ToolManagementPane SHALL display the Drop_Zone at the top of the section, followed by a list of all Knowledge_Entry records for the current session ordered by created_at ascending
3. THE ToolManagementPane SHALL display each Knowledge_Entry with its file_name, size_bytes formatted in human-readable units (bytes, KB, or MB), enabled state as a toggle, and current Injection_Mode as a selectable indicator
4. THE ToolManagementPane SHALL display a badge on the Knowledge_Plugin accordion header showing the count of Knowledge_Entry records in the current session, and SHALL hide the badge when the count is 0
5. WHILE a Knowledge_Entry has enabled=false, THE ToolManagementPane SHALL render that entry with opacity of 0.5 to indicate disabled state
6. IF the current session has no Knowledge_Entry records, THEN THE ToolManagementPane SHALL display a placeholder message within the expanded accordion indicating that no knowledge files have been added

### Requirement 9: データベーススキーマ

**User Story:** As a developer, I want a dedicated session_knowledge table, so that knowledge entries are persisted per session with all required metadata.

#### Acceptance Criteria

1. THE Knowledge_Plugin SHALL use a session_knowledge table with columns: id (TEXT PRIMARY KEY), session_id (TEXT NOT NULL), file_name (TEXT NOT NULL), content (TEXT NOT NULL), size_bytes (INTEGER NOT NULL), enabled (INTEGER NOT NULL DEFAULT 1), injection_mode (TEXT NOT NULL DEFAULT 'system_prompt'), created_at (TEXT NOT NULL)
2. THE session_knowledge table SHALL have a foreign key constraint on session_id referencing the chat_sessions table with ON DELETE CASCADE
3. THE session_knowledge table SHALL enforce a UNIQUE constraint on (session_id, file_name) to prevent duplicate file names within a session
4. THE session_knowledge table SHALL enforce a CHECK constraint on injection_mode restricting values to 'system_prompt' or 'tool_reference'
5. THE Knowledge_Plugin SHALL create an index on the session_id column of the session_knowledge table for query performance

### Requirement 10: Tauriコマンドインターフェース

**User Story:** As a frontend developer, I want Tauri commands for all knowledge operations, so that the frontend can interact with the knowledge backend.

#### Acceptance Criteria

1. THE Knowledge_Plugin SHALL expose an add_knowledge Tauri command accepting session_id (String), file_name (String), and content (String) parameters, and returning the created Knowledge_Entry record on success
2. THE Knowledge_Plugin SHALL expose a remove_knowledge Tauri command accepting session_id (String) and file_name (String) parameters, and returning unit on success
3. THE Knowledge_Plugin SHALL expose a list_knowledge Tauri command accepting session_id (String) and returning all Knowledge_Entry records for that session, where each record includes id, file_name, size_bytes, enabled, injection_mode, and created_at fields but excludes the content field
4. THE Knowledge_Plugin SHALL expose a toggle_knowledge Tauri command accepting session_id (String), file_name (String), and enabled (bool) parameters, and returning unit on success
5. THE Knowledge_Plugin SHALL expose a set_injection_mode Tauri command accepting session_id (String), file_name (String), and injection_mode (String limited to "system_prompt" or "tool_reference") parameters, and returning unit on success
6. THE Knowledge_Plugin SHALL expose an export_knowledge Tauri command accepting session_id (String) and file_name (String) parameters, and returning the content (String) of the matching Knowledge_Entry
7. IF a command receives a session_id and file_name combination that does not match any existing Knowledge_Entry, THEN THE Knowledge_Plugin SHALL return an error indicating that the specified entry was not found
8. IF set_injection_mode receives an injection_mode value other than "system_prompt" or "tool_reference", THEN THE Knowledge_Plugin SHALL return a validation error indicating the allowed values
