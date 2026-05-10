# Tasks: File Ops Sync Permission

## Phase 1: Rust Backend API & Plugin Interface Update
- [x] 1.1 `src-tauri/src/plugin/system.rs`: `PluginHandler::execute` と `PluginSystem::handle_tool_calls` の引数に `app_handle: &tauri::AppHandle` を追加し、`DefaultPluginSystem::handle_tool_calls` のシグネチャを更新して `execute_tool` 呼び出しに `app_handle` を渡す
- [x] 1.2 `src-tauri/src/plugin/registry.rs`: `PluginRegistry::execute_tool` の引数に `app_handle: &tauri::AppHandle` を追加し、`DefaultPluginRegistry::execute_tool` を更新して各プラグインの `execute` に `app_handle` を渡す
- [x] 1.3 `src-tauri/src/plugin/builtin/*.rs` (calculator.rs, file_ops.rs, web_search.rs) および `src-tauri/src/plugin/custom/executor.rs`: 各 `PluginHandler` 実装の `execute` メソッドのシグネチャに `app_handle: &tauri::AppHandle` を追加する
- [x] 1.4 `src-tauri/src/chat/engine.rs`: `DefaultChatEngine::send_message`, `regenerate`, `edit_and_resend` 内の `handle_tool_calls` 呼び出し箇所に `app_handle` を渡すように修正する

## Phase 2: Request State Management & DB Access
- [x] 2.1 `src-tauri/src/state.rs`: `FileOpsStateManager` 構造体を定義する（`pending_requests: tokio::sync::Mutex<HashMap<String, oneshot::Sender<bool>>>`）
- [x] 2.2 `src-tauri/src/main.rs`: `FileOpsStateManager` を State として登録し、`FileOpsPlugin::new` に `db` (Database Arc Mutex) を渡すように変更する
- [x] 2.3 `src-tauri/src/plugin/builtin/file_ops.rs`: `FileOpsPlugin` 構造体に `db: Arc<Mutex<Database>>` フィールドを追加する

## Phase 3: FileOpsPlugin Internals Update (Async Wait)
- [x] 3.1 `src-tauri/src/plugin/builtin/file_ops.rs`: パス検証処理を async に変更し、アクセス拒否時に oneshot チャネルで UI 応答を待機するフローを実装する（UUID生成、emit、DB更新含む）
- [x] 3.2 `src-tauri/src/plugin/builtin/file_ops.rs`: ツール定義から `request_directory_access` を削除し、`execute` 内の対応する match アームも削除する

## Phase 4: Tauri Command `resolve_file_ops_access`
- [x] 4.1 `src-tauri/src/commands/plugin.rs`: `resolve_file_ops_access` コマンドを実装する（request_id で Sender を取り出し send(granted) で待機解除）
- [x] 4.2 `src-tauri/src/main.rs`: `generate_handler!` に `resolve_file_ops_access` を追加する

## Phase 5: Frontend UI Event Listener & Dialog Implementation
- [x] 5.1 `src/components/chat/FileOpsDirectoryManager.tsx`: `file_ops:request_access` イベントリスナーを追加し、アクセス許可ダイアログUIを実装する
- [x] 5.2 `src/components/chat/MessageBubble.tsx`: 不要となった `DirectoryAccessRequestUI` コンポーネントと `parseDirectoryAccessRequest` ロジックを削除する
