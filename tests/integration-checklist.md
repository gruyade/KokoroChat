# フロントエンド-バックエンド結合確認チェックリスト

## 1. Tauri Command invoke呼び出し一致確認

フロントエンドの `invoke()` 呼び出しとバックエンドの `#[tauri::command]` 関数名が一致していることを確認。

### Character Commands

| Frontend invoke名 | Backend command名 | パラメータ一致 | 戻り値型一致 |
|---|---|---|---|
| `invoke('create_character', { name, description })` | `create_character(name: String, description: String)` | ✅ | ✅ `Character` |
| `invoke('list_characters')` | `list_characters()` | ✅ | ✅ `Vec<Character>` |
| `invoke('get_character', { id })` | `get_character(id: String)` | ✅ | ✅ `Option<Character>` |
| `invoke('update_character', { id, updates })` | `update_character(id: String, updates: CharacterUpdate)` | ✅ | ✅ `()` |
| `invoke('delete_character', { id })` | `delete_character(id: String)` | ✅ | ✅ `()` |

### Chat Commands

| Frontend invoke名 | Backend command名 | パラメータ一致 | 戻り値型一致 |
|---|---|---|---|
| `invoke('create_session', { characterId })` | `create_session(character_id: String)` | ✅ | ✅ `String` |
| `invoke('send_message', { sessionId, content, attachments })` | `send_message(session_id: String, content: String, attachments: Option<Vec<String>>)` | ✅ | ✅ `()` |
| `invoke('get_history', { sessionId })` | `get_history(session_id: String)` | ✅ | ✅ `Vec<ChatMessageRecord>` |
| `invoke('list_sessions', { characterId })` | `list_sessions(character_id: String)` | ✅ | ✅ `Vec<ChatSession>` |
| `invoke('delete_session', { sessionId })` | `delete_session(session_id: String)` | ✅ | ✅ `()` |

### Config Commands

| Frontend invoke名 | Backend command名 | パラメータ一致 | 戻り値型一致 |
|---|---|---|---|
| `invoke('get_config')` | `get_config()` | ✅ | ✅ `AppConfig` |
| `invoke('set_config', { config })` | `set_config(config: AppConfig)` | ✅ | ✅ `()` |
| `invoke('test_llm_connection', { settings })` | `test_llm_connection(settings: ModelSettings)` | ✅ | ✅ `()` |

### Attachment Commands

| Frontend invoke名 | Backend command名 | パラメータ一致 | 戻り値型一致 |
|---|---|---|---|
| `invoke('process_attachment', { filePath })` | `process_attachment(file_path: String)` | ✅ | ✅ `Attachment` |
| `invoke('get_supported_extensions')` | `get_supported_extensions()` | ✅ | ✅ `Vec<String>` |

### Plugin Commands

| Frontend invoke名 | Backend command名 | パラメータ一致 | 戻り値型一致 |
|---|---|---|---|
| `invoke('list_plugins')` | `list_plugins()` | ✅ | ✅ `Vec<PluginInfo>` |
| `invoke('enable_plugin', { name })` | `enable_plugin(name: String)` | ✅ | ✅ `()` |
| `invoke('disable_plugin', { name })` | `disable_plugin(name: String)` | ✅ | ✅ `()` |
| `invoke('get_plugin_config', { name })` | `get_plugin_config(name: String)` | ✅ | ✅ `Option<Value>` |
| `invoke('set_plugin_config', { name, config })` | `set_plugin_config(name: String, config: Value)` | ✅ | ✅ `()` |

### Memory Commands

| Frontend invoke名 | Backend command名 | パラメータ一致 | 戻り値型一致 |
|---|---|---|---|
| `invoke('list_memories', { characterId })` | `list_memories(character_id: String)` | ✅ | ✅ `Vec<Memory>` |
| `invoke('update_memory', { id, content })` | `update_memory(id: String, content: String)` | ✅ | ✅ `()` |
| `invoke('delete_memory', { id })` | `delete_memory(id: String)` | ✅ | ✅ `()` |

### TTS Commands

| Frontend invoke名 | Backend command名 | パラメータ一致 | 戻り値型一致 |
|---|---|---|---|
| `invoke('synthesize_speech', { text, config })` | `synthesize_speech(text: String, config: TTSConfig)` | ✅ | ✅ `Vec<u8>` |
| `invoke('test_tts_connection', { config })` | `test_tts_connection(config: TTSConfig)` | ✅ | ✅ `()` |

## 2. Tauri Event名一致確認

フロントエンドの `listen()` イベント名とバックエンドの `emit()` イベント名が一致していることを確認。

| Event名 | Backend emit箇所 | Frontend listen箇所 | ペイロード型一致 |
|---|---|---|---|
| `chat:stream` | `ChatEngine::send_message` | `src/hooks/useChat.ts` | ✅ `{ session_id, chunk, done }` |
| `spontaneous:message` | `SpontaneousSpeaker` | `src/hooks/useChat.ts` | ✅ `{ session_id, message }` |
| `thought:generated` | `ThoughtEngine` | (ThoughtView直接) | ✅ `{ character_id, thought }` |
| `tts:audio` | `ChatEngine (TTS有効時)` | `src/hooks/useAudio.ts` | ✅ `{ data: string }` |
| `tool:executing` | `ChatEngine (tool_call時)` | `src/hooks/useChat.ts` | ✅ `{ session_id, tool_name }` |
| `tool:result` | `ChatEngine (tool結果時)` | (未使用 — UIはstream完了で表示) | ✅ `{ session_id, tool_call_id, content, is_error }` |

## 3. TypeScript型 ↔ Rust struct シリアライゼーション一致確認

### Character

| フィールド | TypeScript型 | Rust型 | serde属性 | 一致 |
|---|---|---|---|---|
| id | `string` | `String` | — | ✅ |
| name | `string` | `String` | — | ✅ |
| description | `string` | `String` | — | ✅ |
| system_prompt | `string` | `String` | — | ✅ |
| avatar_path | `string?` | `Option<String>` | — | ✅ |
| tts_config | `TTSConfig?` | `Option<TTSConfig>` | — | ✅ |
| created_at | `string` | `String` (ISO 8601) | — | ✅ |
| updated_at | `string` | `String` (ISO 8601) | — | ✅ |

### ChatSession

| フィールド | TypeScript型 | Rust型 | 一致 |
|---|---|---|---|
| id | `string` | `String` | ✅ |
| character_id | `string` | `String` | ✅ |
| title | `string?` | `Option<String>` | ✅ |
| last_message_at | `string?` | `Option<String>` | ✅ |
| last_message_preview | `string?` | `Option<String>` | ✅ |
| created_at | `string` | `String` | ✅ |

### ChatMessageRecord

| フィールド | TypeScript型 | Rust型 | 一致 |
|---|---|---|---|
| id | `string` | `String` | ✅ |
| session_id | `string` | `String` | ✅ |
| role | `ChatRole` | `ChatRole` (serde rename_all=lowercase) | ✅ |
| content | `string` | `String` | ✅ |
| attachments | `MessageAttachment[]?` | `Option<Vec<MessageAttachment>>` | ✅ |
| tool_calls | `ToolCall[]?` | `Option<Vec<ToolCall>>` | ✅ |
| tool_call_id | `string?` | `Option<String>` | ✅ |
| created_at | `string` | `String` | ✅ |

### AppConfig

| フィールド | TypeScript型 | Rust型 | 一致 |
|---|---|---|---|
| models | `Record<ModelPurpose, ModelSettings>` | `HashMap<ModelPurpose, ModelSettings>` | ✅ |
| spontaneous | `SpontaneousConfig` | `SpontaneousConfig` | ✅ |
| thought | `ThoughtConfig` | `ThoughtConfig` | ✅ |
| memory | `MemoryConfig` | `MemoryConfig` | ✅ |
| tts | `TTSGlobalConfig` | `TTSGlobalConfig` | ✅ |
| ui | `UIConfig` | `UIConfig` | ✅ |
| plugins | `PluginsConfig` | `PluginsConfig` | ✅ |
| attachment | `AttachmentConfig` | `AttachmentConfig` | ✅ |

### PluginInfo

| フィールド | TypeScript型 | Rust型 | 一致 |
|---|---|---|---|
| name | `string` | `String` | ✅ |
| description | `string` | `String` | ✅ |
| version | `string` | `String` | ✅ |
| enabled | `boolean` | `bool` | ✅ |
| tools | `ToolDefinition[]` | `Vec<ToolDefinition>` | ✅ |
| config | `Record<string, unknown>?` | `Option<Value>` | ✅ |

### Attachment

| フィールド | TypeScript型 | Rust型 | 一致 |
|---|---|---|---|
| id | `string` | `String` | ✅ |
| file_name | `string` | `String` | ✅ |
| file_path | `string` | `String` | ✅ |
| attachment_type | `AttachmentType` | `AttachmentType` | ✅ |
| size_bytes | `number` | `u64` | ✅ |
| extracted_text | `string?` | `Option<String>` | ✅ |
| base64_data | `string?` | `Option<String>` | ✅ |

## 4. 確認結果サマリー

- **Tauri Command**: 全18コマンドの名前・パラメータ・戻り値型が一致
- **Tauri Event**: 全6イベントの名前・ペイロード構造が一致
- **型定義**: 全主要struct/interfaceのフィールド名・型が一致（snake_case統一）
- **注意点**: TauriのinvokeパラメータはcamelCaseで渡すが、Rust側は`#[serde(rename_all = "camelCase")]`またはTauriの自動変換で対応

## 5. 未実装・将来対応事項

- `src-tauri/src/commands/thought.rs` — Thought関連コマンド未実装（ThoughtViewは直接eventリスンで対応）
- `tool:result` イベント — フロントエンドで明示的にリスンしていないが、ストリーミング完了で結果表示されるため問題なし
