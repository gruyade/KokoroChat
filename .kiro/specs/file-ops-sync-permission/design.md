# Design: File Ops Sync Permission

## Architecture & Data Flow

### 1. PluginSystem & PluginHandler の変更
イベント発行（ダイアログ要求）を行うために、プラグイン実行時に `AppHandle` が必要です。
- `PluginHandler::execute` トレイトメソッドの引数に `app_handle: &tauri::AppHandle` を追加します。
- `PluginSystem::handle_tool_calls` にも `app_handle: &tauri::AppHandle` を追加します。
- `src-tauri/src/chat/engine.rs` のツール実行ループから `app_handle` を渡すように修正します。

### 2. リクエスト管理状態 (State)
UIからの結果を受け取るために、グローバルな待機状態を管理します。
- `FileOpsStateManager` 構造体を定義します。
  ```rust
  pub struct FileOpsStateManager {
      pub pending_requests: tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
  }
  ```
- アプリケーション起動時（`main.rs`）に `app.manage(FileOpsStateManager::default())` で登録します。
- `FileOpsPlugin` からはこの State にアクセスして `Sender` を保存し、`Receiver` で待機します。

### 3. FileOpsPlugin の実行フロー変更
`read_file`, `write_file`, `list_directory`, `search_files` でアクセス検証を行う際、権限がない場合：
1. リクエストID (UUID) を生成。
2. `oneshot::channel` を作成し、`Sender` を `FileOpsStateManager` に保存。
3. `app_handle.emit("file_ops:request_access", payload)` を発行。
4. `receiver.await` でユーザーの応答を待機。
5. 応答が `true` の場合、設定DB（`chat_plugin_config`）の ACL を直接更新し、要求されたファイル操作を実行して結果を返す。
6. 応答が `false` の場合、アクセス拒否エラーを返す。

### 4. フロントエンドの対応
- イベント `file_ops:request_access` のリスナーを実装します。（`FileOpsDirectoryManager` 内か、Chat全体のラッパー層などでリスン）
- イベント受信時、ユーザーに確認ダイアログまたはインラインの許可UIを表示します。
- ユーザーのアクション後、Tauriコマンド `resolve_file_ops_access(request_id: String, granted: bool)` を呼び出します。

### 5. 不要になるツール
- LLMが直接呼び出すための `request_directory_access` ツールは不要になりますが、後方互換性のため一旦残しておくか、ツール定義から削除します。（削除が望ましい）