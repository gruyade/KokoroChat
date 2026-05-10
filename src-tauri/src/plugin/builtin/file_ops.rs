// ファイル読み書きプラグイン

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use serde_json::{json, Value};
use tauri::{Emitter, Manager};
use uuid::Uuid;

use crate::db::database::Database;
use crate::db::repositories::chat_plugin_config;
use crate::error::AppError;
use crate::models::chat::{DirectoryPermission, FileOpsConfig};
use crate::models::plugin::{ToolCall, ToolDefinition, ToolExecutionContext, ToolResult};
use crate::plugin::system::PluginHandler;
use crate::state::FileOpsStateManager;

/// ファイル操作プラグイン — ファイルの読み書きを行う（サンドボックス付き）
pub struct FileOpsPlugin {
    /// サンドボックスのベースディレクトリ（ACL未設定時のフォールバック）
    base_dir: PathBuf,
    /// データベースへの参照（ACL永続化等に使用）
    db: Arc<Mutex<Database>>,
}

impl FileOpsPlugin {
    pub fn new(base_dir: PathBuf, db: Arc<Mutex<Database>>) -> Self {
        Self { base_dir, db }
    }

    /// パスに ".." コンポーネントが含まれていないか検証
    fn reject_traversal(path: &Path) -> Result<(), String> {
        for component in path.components() {
            if let std::path::Component::ParentDir = component {
                return Err("パスに '..' を含めることはできない".to_string());
            }
        }
        Ok(())
    }

    /// パスを正規化して比較用文字列を生成（Windows対応: バックスラッシュをスラッシュに統一）
    fn normalize_for_comparison(path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/").to_lowercase()
    }

    /// パスが指定ディレクトリに内包されているか確認
    fn is_path_within(path: &Path, dir: &Path) -> bool {
        let norm_path = Self::normalize_for_comparison(path);
        let mut norm_dir = Self::normalize_for_comparison(dir);

        // ディレクトリパスが "/" で終わっていなければ追加
        if !norm_dir.ends_with('/') {
            norm_dir.push('/');
        }

        // パスがディレクトリと完全一致、またはディレクトリ配下にある
        norm_path == norm_dir.trim_end_matches('/')
            || norm_path.starts_with(&norm_dir)
    }

    /// ACLリストに基づくパス検証
    /// - `require_write` が true の場合、`allow_write == true` のディレクトリのみ有効
    /// - `require_write` が false の場合、`allow_read == true` のディレクトリのみ有効
    pub fn validate_path_with_acl(
        &self,
        path_str: &str,
        acl: &[DirectoryPermission],
        require_write: bool,
    ) -> Result<PathBuf, String> {
        let path = Path::new(path_str);
        Self::reject_traversal(path)?;

        // 絶対パスはそのまま、相対パスはbase_dir基準で解決
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_dir.join(path)
        };

        // ACLリストのいずれかのディレクトリに内包されているか確認
        for perm in acl {
            let dir_path = Path::new(&perm.path);
            let allowed = if require_write {
                perm.allow_write
            } else {
                perm.allow_read
            };

            if allowed && Self::is_path_within(&resolved, dir_path) {
                return Ok(resolved);
            }
        }

        let op = if require_write { "書き込み" } else { "読み取り" };
        Err(format!(
            "アクセス拒否: '{}' は許可されたディレクトリ内にない（{}権限なし）",
            path_str, op
        ))
    }

    /// 従来のbase_dirベースのパス検証（ACL未設定時のフォールバック）
    fn validate_path(&self, path_str: &str) -> Result<PathBuf, String> {
        let path = Path::new(path_str);
        Self::reject_traversal(path)?;

        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_dir.join(path)
        };

        let base_str = Self::normalize_for_comparison(&self.base_dir);
        let resolved_str = Self::normalize_for_comparison(&resolved);

        if !resolved_str.starts_with(&base_str) {
            return Err(format!("アクセス拒否: '{}' はサンドボックス外", path_str));
        }

        Ok(resolved)
    }

    /// ToolExecutionContext から ACL を抽出
    fn extract_acl(context: &Option<ToolExecutionContext>) -> Option<Vec<DirectoryPermission>> {
        let ctx = context.as_ref()?;
        let config_json = ctx.plugin_config_json.as_ref()?;
        let config: FileOpsConfig = serde_json::from_str(config_json).ok()?;
        if config.directories.is_empty() {
            None
        } else {
            Some(config.directories)
        }
    }

    /// パス検証（ACLがあればACLベース、なければbase_dirフォールバック）
    fn validate_path_for_op(
        &self,
        path_str: &str,
        acl: &Option<Vec<DirectoryPermission>>,
        require_write: bool,
    ) -> Result<PathBuf, String> {
        match acl {
            Some(dirs) => self.validate_path_with_acl(path_str, dirs, require_write),
            None => self.validate_path(path_str),
        }
    }

    /// 画像ファイルの拡張子かどうか判定
    fn is_image_extension(path: &Path) -> bool {
        const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "svg"];
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    fn read_file(
        &self,
        path_str: &str,
        acl: &Option<Vec<DirectoryPermission>>,
    ) -> Result<String, String> {
        let path = self.validate_path_for_op(path_str, acl, false)?;

        // 画像ファイルの場合はBase64エンコードして返す
        if Self::is_image_extension(&path) {
            let bytes = std::fs::read(&path).map_err(|e| format!("画像読み込みエラー: {}", e))?;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Ok(format!("[IMAGE_BASE64]:{}", b64));
        }

        std::fs::read_to_string(&path).map_err(|e| format!("ファイル読み込みエラー: {}", e))
    }

    fn write_file(
        &self,
        path_str: &str,
        content: &str,
        acl: &Option<Vec<DirectoryPermission>>,
    ) -> Result<String, String> {
        let path = self.validate_path_for_op(path_str, acl, true)?;

        // 親ディレクトリが存在しない場合は作成
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("ディレクトリ作成エラー: {}", e))?;
        }

        std::fs::write(&path, content).map_err(|e| format!("ファイル書き込みエラー: {}", e))?;

        Ok(format!("ファイルを書き込み完了: {}", path.display()))
    }

    fn list_directory(
        &self,
        path_str: &str,
        acl: &Option<Vec<DirectoryPermission>>,
    ) -> Result<String, String> {
        let path = self.validate_path_for_op(path_str, acl, false)?;

        if !path.is_dir() {
            return Err(format!("'{}' はディレクトリではない", path_str));
        }

        let entries = std::fs::read_dir(&path)
            .map_err(|e| format!("ディレクトリ読み込みエラー: {}", e))?;

        let mut items: Vec<String> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| format!("エントリ読み込みエラー: {}", e))?;
            let file_type = entry.file_type().map_err(|e| format!("タイプ取得エラー: {}", e))?;
            let name = entry.file_name().to_string_lossy().to_string();
            let prefix = if file_type.is_dir() { "[DIR] " } else { "" };
            items.push(format!("{}{}", prefix, name));
        }

        Ok(items.join("\n"))
    }

    fn search_files(
        &self,
        path_str: &str,
        pattern: &str,
        acl: &Option<Vec<DirectoryPermission>>,
    ) -> Result<String, String> {
        let path = self.validate_path_for_op(path_str, acl, false)?;

        if !path.is_dir() {
            return Err(format!("'{}' はディレクトリではない", path_str));
        }

        let mut matches: Vec<String> = Vec::new();
        Self::search_recursive(&path, pattern, &mut matches)?;

        if matches.is_empty() {
            Ok(format!("パターン '{}' に一致するファイルなし", pattern))
        } else {
            Ok(matches.join("\n"))
        }
    }

    fn search_recursive(
        dir: &Path,
        pattern: &str,
        matches: &mut Vec<String>,
    ) -> Result<(), String> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("ディレクトリ読み込みエラー: {}", e))?;

        let pattern_lower = pattern.to_lowercase();

        for entry in entries {
            let entry = entry.map_err(|e| format!("エントリ読み込みエラー: {}", e))?;
            let entry_path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            if entry_path.is_dir() {
                Self::search_recursive(&entry_path, pattern, matches)?;
            } else if file_name.to_lowercase().contains(&pattern_lower) {
                matches.push(entry_path.to_string_lossy().to_string());
            }
        }

        Ok(())
    }

    /// DBから最新のACLを再取得する（許可後の再実行用）
    fn reload_acl_from_db(&self, session_id: &str) -> Option<Vec<DirectoryPermission>> {
        let db_guard = self.db.lock().ok()?;
        let conn = db_guard.connection();
        let record = chat_plugin_config::get_config(conn, session_id, "file_ops").ok()??;
        let config: FileOpsConfig = serde_json::from_str(&record.config_json).ok()?;
        if config.directories.is_empty() {
            None
        } else {
            Some(config.directories)
        }
    }

    /// アクセス拒否時にUIへ許可リクエストを発行し、ユーザー応答を待機する。
    /// 許可された場合はDBのACLを更新して `Ok(())` を返す。
    /// 拒否またはチャネルエラーの場合は `Err(original_error)` を返す。
    async fn request_permission_and_wait(
        &self,
        app_handle: &tauri::AppHandle,
        session_id: &str,
        path_str: &str,
        requires_write: bool,
        original_error: String,
    ) -> Result<(), String> {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();

        // Sender を FileOpsStateManager に保存
        let state_manager = app_handle.state::<FileOpsStateManager>();
        {
            let mut pending = state_manager.pending_requests.lock().await;
            pending.insert(request_id.clone(), tx);
        }

        // UIにイベント発行
        let payload = FileOpsAccessRequestPayload {
            session_id: session_id.to_string(),
            request_id: request_id.clone(),
            path: path_str.to_string(),
            requires_write,
        };

        if let Err(e) = app_handle.emit("file_ops:request_access", payload) {
            // emit失敗時はSenderをクリーンアップして元のエラーを返す
            let mut pending = state_manager.pending_requests.lock().await;
            pending.remove(&request_id);
            eprintln!("[file_ops] request_access emit失敗: {}", e);
            return Err(original_error);
        }

        // ユーザー応答を待機
        let granted = match rx.await {
            Ok(value) => value,
            Err(_) => {
                // チャネルがドロップされた（タイムアウト等）
                return Err(original_error);
            }
        };

        if !granted {
            return Err(original_error);
        }

        // 許可された: DBのACLを更新
        self.update_acl_in_db(session_id, path_str, requires_write)?;

        Ok(())
    }

    /// DBの chat_plugin_configs テーブルの file_ops 設定にディレクトリを追加する
    fn update_acl_in_db(
        &self,
        session_id: &str,
        path_str: &str,
        requires_write: bool,
    ) -> Result<(), String> {
        let db_guard = self.db.lock().map_err(|e| format!("DB lock失敗: {}", e))?;
        let conn = db_guard.connection();

        // 現在の設定を取得
        let existing = chat_plugin_config::get_config(conn, session_id, "file_ops")
            .map_err(|e| format!("設定取得エラー: {}", e))?;

        let mut config: FileOpsConfig = match &existing {
            Some(record) => serde_json::from_str(&record.config_json)
                .unwrap_or(FileOpsConfig { directories: vec![] }),
            None => FileOpsConfig { directories: vec![] },
        };

        // パスの親ディレクトリを許可対象として追加（ファイルパスの場合は親を使う）
        let dir_path = {
            let p = Path::new(path_str);
            if p.is_file() || p.extension().is_some() {
                // ファイルパスの場合は親ディレクトリを使用
                p.parent()
                    .map(|pp| pp.to_string_lossy().to_string())
                    .unwrap_or_else(|| path_str.to_string())
            } else {
                path_str.to_string()
            }
        };

        // 既存のエントリを確認し、あれば権限を拡張、なければ追加
        let mut found = false;
        for perm in &mut config.directories {
            if perm.path == dir_path {
                if requires_write {
                    perm.allow_write = true;
                }
                perm.allow_read = true;
                found = true;
                break;
            }
        }

        if !found {
            config.directories.push(DirectoryPermission {
                path: dir_path,
                allow_read: true,
                allow_write: requires_write,
            });
        }

        // DB に書き戻し
        let config_json = serde_json::to_string(&config)
            .map_err(|e| format!("設定シリアライズエラー: {}", e))?;

        chat_plugin_config::upsert_config(conn, session_id, "file_ops", &config_json)
            .map_err(|e| format!("設定更新エラー: {}", e))?;

        Ok(())
    }
}

/// UIへのアクセス許可リクエストイベントペイロード
#[derive(Clone, serde::Serialize)]
struct FileOpsAccessRequestPayload {
    session_id: String,
    request_id: String,
    path: String,
    requires_write: bool,
}


#[async_trait]
impl PluginHandler for FileOpsPlugin {
    fn name(&self) -> &str {
        "file_ops"
    }

    fn description(&self) -> &str {
        "ユーザーが許可したディレクトリ内のファイルを読み書きする。絶対パスで指定可能。アクセス許可が必要な場合は自動的にユーザーに確認される。"
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "read_file".to_string(),
                description: "指定したパスのファイルを読み込む。テキストファイルはそのまま内容を返す。画像ファイル（.png/.jpg/.jpeg/.gif/.webp/.bmp）はBase64エンコードして返す。ファイルを編集する前には必ずこのツールで現在の内容を確認すること。アクセス許可が必要な場合はユーザーに確認ダイアログが表示される。".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "読み込むファイルの絶対パス（例: C:/Users/user/documents/file.txt）またはサンドボックス内の相対パス"
                        }
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "ファイルを新規作成または上書き保存する。ファイルが存在しない場合は新規作成、存在する場合は内容を完全に上書きする。ユーザーからファイルの作成・編集・保存を依頼された場合は必ずこのツールを使うこと。親ディレクトリが存在しない場合は自動的に作成される。アクセス許可が必要な場合はユーザーに確認ダイアログが表示される。".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "書き込み先ファイルの絶対パス（例: C:/Users/user/documents/file.txt）またはサンドボックス内の相対パス"
                        },
                        "content": {
                            "type": "string",
                            "description": "ファイルに書き込む内容（テキスト）。既存ファイルの場合は全内容が置き換わる"
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
            ToolDefinition {
                name: "list_directory".to_string(),
                description: "指定したディレクトリのファイルとサブディレクトリ一覧を取得する。ファイル構成を把握したい場合や、編集・参照するファイルのパスを確認したい場合に使う。アクセス許可が必要な場合はユーザーに確認ダイアログが表示される。".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "一覧を取得するディレクトリの絶対パス（例: C:/Users/user/documents）またはサンドボックス内の相対パス"
                        }
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "search_files".to_string(),
                description: "指定したディレクトリ以下のファイルをパターンで検索する（再帰的）。ファイル名の一部を指定して対象ファイルを探したい場合に使う。アクセス許可が必要な場合はユーザーに確認ダイアログが表示される。".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "検索対象ディレクトリの絶対パス（例: C:/Users/user/documents）またはサンドボックス内の相対パス"
                        },
                        "pattern": {
                            "type": "string",
                            "description": "検索パターン（ファイル名の部分一致。例: '.txt' で全テキストファイル、'readme' で readme を含むファイル）"
                        }
                    },
                    "required": ["path", "pattern"]
                }),
            },

        ]
    }

    async fn execute(&self, tool_call: &ToolCall, app_handle: &tauri::AppHandle) -> Result<ToolResult, AppError> {
        let acl = Self::extract_acl(&tool_call.context);
        let session_id = tool_call
            .context
            .as_ref()
            .and_then(|c| c.session_id.as_ref())
            .cloned();

        match tool_call.name.as_str() {
            "read_file" => {
                let path = tool_call
                    .arguments
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::Plugin("'path' パラメータが必要".to_string()))?;

                let result = match self.read_file(path, &acl) {
                    Ok(content) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content,
                        is_error: false,
                    },
                    Err(err) if err.contains("アクセス拒否") && session_id.is_some() => {
                        let sid = session_id.as_ref().unwrap();
                        match self
                            .request_permission_and_wait(app_handle, sid, path, false, err.clone())
                            .await
                        {
                            Ok(()) => {
                                // 許可された: ACLを再取得して再実行
                                let new_acl = self.reload_acl_from_db(sid);
                                match self.read_file(path, &new_acl) {
                                    Ok(content) => ToolResult {
                                        tool_call_id: tool_call.id.clone(),
                                        content,
                                        is_error: false,
                                    },
                                    Err(e) => ToolResult {
                                        tool_call_id: tool_call.id.clone(),
                                        content: e,
                                        is_error: true,
                                    },
                                }
                            }
                            Err(denied_err) => ToolResult {
                                tool_call_id: tool_call.id.clone(),
                                content: denied_err,
                                is_error: true,
                            },
                        }
                    }
                    Err(err) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content: err,
                        is_error: true,
                    },
                };
                Ok(result)
            }
            "write_file" => {
                let path = tool_call
                    .arguments
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::Plugin("'path' パラメータが必要".to_string()))?;
                let content = tool_call
                    .arguments
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::Plugin("'content' パラメータが必要".to_string()))?;

                let result = match self.write_file(path, content, &acl) {
                    Ok(msg) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content: msg,
                        is_error: false,
                    },
                    Err(err) if err.contains("アクセス拒否") && session_id.is_some() => {
                        let sid = session_id.as_ref().unwrap();
                        match self
                            .request_permission_and_wait(app_handle, sid, path, true, err.clone())
                            .await
                        {
                            Ok(()) => {
                                let new_acl = self.reload_acl_from_db(sid);
                                match self.write_file(path, content, &new_acl) {
                                    Ok(msg) => ToolResult {
                                        tool_call_id: tool_call.id.clone(),
                                        content: msg,
                                        is_error: false,
                                    },
                                    Err(e) => ToolResult {
                                        tool_call_id: tool_call.id.clone(),
                                        content: e,
                                        is_error: true,
                                    },
                                }
                            }
                            Err(denied_err) => ToolResult {
                                tool_call_id: tool_call.id.clone(),
                                content: denied_err,
                                is_error: true,
                            },
                        }
                    }
                    Err(err) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content: err,
                        is_error: true,
                    },
                };
                Ok(result)
            }
            "list_directory" => {
                let path = tool_call
                    .arguments
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::Plugin("'path' パラメータが必要".to_string()))?;

                let result = match self.list_directory(path, &acl) {
                    Ok(content) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content,
                        is_error: false,
                    },
                    Err(err) if err.contains("アクセス拒否") && session_id.is_some() => {
                        let sid = session_id.as_ref().unwrap();
                        match self
                            .request_permission_and_wait(app_handle, sid, path, false, err.clone())
                            .await
                        {
                            Ok(()) => {
                                let new_acl = self.reload_acl_from_db(sid);
                                match self.list_directory(path, &new_acl) {
                                    Ok(content) => ToolResult {
                                        tool_call_id: tool_call.id.clone(),
                                        content,
                                        is_error: false,
                                    },
                                    Err(e) => ToolResult {
                                        tool_call_id: tool_call.id.clone(),
                                        content: e,
                                        is_error: true,
                                    },
                                }
                            }
                            Err(denied_err) => ToolResult {
                                tool_call_id: tool_call.id.clone(),
                                content: denied_err,
                                is_error: true,
                            },
                        }
                    }
                    Err(err) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content: err,
                        is_error: true,
                    },
                };
                Ok(result)
            }
            "search_files" => {
                let path = tool_call
                    .arguments
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::Plugin("'path' パラメータが必要".to_string()))?;
                let pattern = tool_call
                    .arguments
                    .get("pattern")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        AppError::Plugin("'pattern' パラメータが必要".to_string())
                    })?;

                let result = match self.search_files(path, pattern, &acl) {
                    Ok(content) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content,
                        is_error: false,
                    },
                    Err(err) if err.contains("アクセス拒否") && session_id.is_some() => {
                        let sid = session_id.as_ref().unwrap();
                        match self
                            .request_permission_and_wait(app_handle, sid, path, false, err.clone())
                            .await
                        {
                            Ok(()) => {
                                let new_acl = self.reload_acl_from_db(sid);
                                match self.search_files(path, pattern, &new_acl) {
                                    Ok(content) => ToolResult {
                                        tool_call_id: tool_call.id.clone(),
                                        content,
                                        is_error: false,
                                    },
                                    Err(e) => ToolResult {
                                        tool_call_id: tool_call.id.clone(),
                                        content: e,
                                        is_error: true,
                                    },
                                }
                            }
                            Err(denied_err) => ToolResult {
                                tool_call_id: tool_call.id.clone(),
                                content: denied_err,
                                is_error: true,
                            },
                        }
                    }
                    Err(err) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content: err,
                        is_error: true,
                    },
                };
                Ok(result)
            }
            _ => Err(AppError::Plugin(format!(
                "不明なツール: {}",
                tool_call.name
            ))),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (FileOpsPlugin, TempDir) {
        let tmp = TempDir::new().unwrap();
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let plugin = FileOpsPlugin::new(tmp.path().to_path_buf(), db);
        (plugin, tmp)
    }

    fn make_mock_app() -> tauri::App {
        tauri::test::mock_builder().build(tauri::generate_context!()).unwrap()
    }

    fn make_acl(dirs: Vec<(&str, bool, bool)>) -> Vec<DirectoryPermission> {
        dirs.into_iter()
            .map(|(path, allow_read, allow_write)| DirectoryPermission {
                path: path.to_string(),
                allow_read,
                allow_write,
            })
            .collect()
    }

    // --- 基本メタデータ ---

    #[test]
    fn test_plugin_metadata() {
        let (plugin, _tmp) = setup();
        assert_eq!(plugin.name(), "file_ops");
        assert_eq!(plugin.description(), "ユーザーが許可したディレクトリ内のファイルを読み書きする。絶対パスで指定可能。アクセス許可が必要な場合は自動的にユーザーに確認される。");

        let tools = plugin.tools();
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"write_file"));
        assert!(tool_names.contains(&"list_directory"));
        assert!(tool_names.contains(&"search_files"));
        assert_eq!(tools.len(), 4);
    }

    // --- 従来のbase_dirベース検証（フォールバック） ---

    #[test]
    fn test_path_validation_relative() {
        let (plugin, _tmp) = setup();
        assert!(plugin.validate_path("test.txt").is_ok());
        assert!(plugin.validate_path("subdir/test.txt").is_ok());
    }

    #[test]
    fn test_path_validation_traversal_rejected() {
        let (plugin, _tmp) = setup();
        assert!(plugin.validate_path("../etc/passwd").is_err());
        assert!(plugin.validate_path("subdir/../../etc/passwd").is_err());
    }

    #[test]
    fn test_path_validation_absolute_outside_sandbox() {
        let (plugin, _tmp) = setup();
        assert!(plugin.validate_path("/etc/passwd").is_err());
    }

    // --- ACLベース検証 ---

    #[test]
    fn test_acl_validation_read_allowed() {
        let (plugin, tmp) = setup();
        let tmp_path = tmp.path().to_string_lossy().to_string();
        let acl = make_acl(vec![(&tmp_path, true, false)]);

        let result = plugin.validate_path_with_acl("test.txt", &acl, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_acl_validation_read_denied_when_no_read_perm() {
        let (plugin, _tmp) = setup();
        let acl = make_acl(vec![("/some/other/dir", false, true)]);

        let result = plugin.validate_path_with_acl("test.txt", &acl, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("読み取り"));
    }

    #[test]
    fn test_acl_validation_write_allowed() {
        let (plugin, tmp) = setup();
        let tmp_path = tmp.path().to_string_lossy().to_string();
        let acl = make_acl(vec![(&tmp_path, true, true)]);

        let result = plugin.validate_path_with_acl("test.txt", &acl, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_acl_validation_write_denied_when_read_only() {
        let (plugin, tmp) = setup();
        let tmp_path = tmp.path().to_string_lossy().to_string();
        let acl = make_acl(vec![(&tmp_path, true, false)]);

        let result = plugin.validate_path_with_acl("test.txt", &acl, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("書き込み"));
    }

    #[test]
    fn test_acl_validation_traversal_rejected() {
        let (plugin, tmp) = setup();
        let tmp_path = tmp.path().to_string_lossy().to_string();
        let acl = make_acl(vec![(&tmp_path, true, true)]);

        let result = plugin.validate_path_with_acl("../etc/passwd", &acl, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains(".."));
    }

    #[test]
    fn test_acl_validation_multiple_directories() {
        let (plugin, tmp) = setup();
        let tmp_path = tmp.path().to_string_lossy().to_string();
        // 1つ目はread-only、2つ目はread+write
        let acl = make_acl(vec![
            ("/readonly/dir", true, false),
            (&tmp_path, true, true),
        ]);

        // base_dir配下のファイルへの書き込みは2つ目のACLで許可
        let result = plugin.validate_path_with_acl("test.txt", &acl, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_acl_validation_absolute_path_in_allowed_dir() {
        let (plugin, tmp) = setup();
        let tmp_path = tmp.path().to_string_lossy().to_string();
        let acl = make_acl(vec![(&tmp_path, true, true)]);

        let abs_path = tmp.path().join("subdir").join("file.txt");
        let abs_str = abs_path.to_string_lossy().to_string();

        let result = plugin.validate_path_with_acl(&abs_str, &acl, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_acl_validation_absolute_path_outside_all_dirs() {
        let (plugin, _tmp) = setup();
        let acl = make_acl(vec![("/allowed/dir", true, true)]);

        let result = plugin.validate_path_with_acl("/other/dir/file.txt", &acl, false);
        assert!(result.is_err());
    }

    // --- ファイル読み書き（ACLなし = フォールバック） ---

    #[test]
    fn test_write_and_read_file_no_acl() {
        let (plugin, _tmp) = setup();

        let write_result = plugin.write_file("hello.txt", "Hello, World!", &None);
        assert!(write_result.is_ok());

        let read_result = plugin.read_file("hello.txt", &None);
        assert_eq!(read_result.unwrap(), "Hello, World!");
    }

    #[test]
    fn test_write_creates_subdirectories() {
        let (plugin, _tmp) = setup();

        let write_result = plugin.write_file("sub/dir/file.txt", "nested content", &None);
        assert!(write_result.is_ok());

        let read_result = plugin.read_file("sub/dir/file.txt", &None);
        assert_eq!(read_result.unwrap(), "nested content");
    }

    #[test]
    fn test_read_nonexistent_file() {
        let (plugin, _tmp) = setup();
        let result = plugin.read_file("nonexistent.txt", &None);
        assert!(result.is_err());
    }

    // --- ファイル読み書き（ACLあり） ---

    #[test]
    fn test_write_and_read_file_with_acl() {
        let (plugin, tmp) = setup();
        let tmp_path = tmp.path().to_string_lossy().to_string();
        let acl = Some(make_acl(vec![(&tmp_path, true, true)]));

        let write_result = plugin.write_file("acl_test.txt", "ACL content", &acl);
        assert!(write_result.is_ok());

        let read_result = plugin.read_file("acl_test.txt", &acl);
        assert_eq!(read_result.unwrap(), "ACL content");
    }

    #[test]
    fn test_write_denied_by_acl() {
        let (plugin, tmp) = setup();
        let tmp_path = tmp.path().to_string_lossy().to_string();
        // read-only
        let acl = Some(make_acl(vec![(&tmp_path, true, false)]));

        let write_result = plugin.write_file("denied.txt", "content", &acl);
        assert!(write_result.is_err());
    }

    // --- list_directory ---

    #[test]
    fn test_list_directory() {
        let (plugin, tmp) = setup();
        // ファイルを作成
        std::fs::write(tmp.path().join("a.txt"), "a").unwrap();
        std::fs::write(tmp.path().join("b.txt"), "b").unwrap();
        std::fs::create_dir(tmp.path().join("subdir")).unwrap();

        let tmp_str = tmp.path().to_string_lossy().to_string();
        let result = plugin.list_directory(&tmp_str, &None);
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("a.txt"));
        assert!(content.contains("b.txt"));
        assert!(content.contains("[DIR] subdir"));
    }

    // --- search_files ---

    #[test]
    fn test_search_files() {
        let (plugin, tmp) = setup();
        std::fs::write(tmp.path().join("hello.rs"), "fn main() {}").unwrap();
        std::fs::write(tmp.path().join("world.txt"), "world").unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub").join("nested.rs"), "mod test;").unwrap();

        let tmp_str = tmp.path().to_string_lossy().to_string();
        let result = plugin.search_files(&tmp_str, ".rs", &None);
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("hello.rs"));
        assert!(content.contains("nested.rs"));
        assert!(!content.contains("world.txt"));
    }

    // --- extract_acl ---

    #[test]
    fn test_extract_acl_none_context() {
        let result = FileOpsPlugin::extract_acl(&None);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_acl_with_valid_config() {
        let ctx = Some(ToolExecutionContext {
            session_id: Some("session-1".to_string()),
            plugin_config_json: Some(
                r#"{"directories":[{"path":"/test","allow_read":true,"allow_write":false}]}"#
                    .to_string(),
            ),
        });
        let result = FileOpsPlugin::extract_acl(&ctx);
        assert!(result.is_some());
        let dirs = result.unwrap();
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].path, "/test");
        assert!(dirs[0].allow_read);
        assert!(!dirs[0].allow_write);
    }

    #[test]
    fn test_extract_acl_empty_directories() {
        let ctx = Some(ToolExecutionContext {
            session_id: Some("session-1".to_string()),
            plugin_config_json: Some(r#"{"directories":[]}"#.to_string()),
        });
        let result = FileOpsPlugin::extract_acl(&ctx);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_acl_invalid_json() {
        let ctx = Some(ToolExecutionContext {
            session_id: Some("session-1".to_string()),
            plugin_config_json: Some("not valid json".to_string()),
        });
        let result = FileOpsPlugin::extract_acl(&ctx);
        assert!(result.is_none());
    }

    // --- execute メソッド ---

    #[tokio::test]
    async fn test_execute_read_file() {
        let app = make_mock_app();
        let (plugin, _tmp) = setup();

        plugin.write_file("test.txt", "test content", &None).unwrap();

        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            arguments: json!({ "path": "test.txt" }),
            context: None,
        };

        let result = plugin.execute(&tool_call, app.handle()).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.content, "test content");
    }

    #[tokio::test]
    async fn test_execute_write_file() {
        let app = make_mock_app();
        let (plugin, _tmp) = setup();

        let tool_call = ToolCall {
            id: "call-2".to_string(),
            name: "write_file".to_string(),
            arguments: json!({ "path": "output.txt", "content": "written via execute" }),
            context: None,
        };

        let result = plugin.execute(&tool_call, app.handle()).await.unwrap();
        assert!(!result.is_error);

        let content = plugin.read_file("output.txt", &None).unwrap();
        assert_eq!(content, "written via execute");
    }

    #[tokio::test]
    async fn test_execute_with_acl_context() {
        let app = make_mock_app();
        let (plugin, tmp) = setup();
        let tmp_path = tmp.path().to_string_lossy().to_string();

        // ACLコンテキスト付きで書き込み
        let config_json = format!(
            r#"{{"directories":[{{"path":"{}","allow_read":true,"allow_write":true}}]}}"#,
            tmp_path.replace('\\', "\\\\")
        );

        let tool_call = ToolCall {
            id: "call-acl".to_string(),
            name: "write_file".to_string(),
            arguments: json!({ "path": "acl_exec.txt", "content": "acl write" }),
            context: Some(ToolExecutionContext {
                session_id: Some("s1".to_string()),
                plugin_config_json: Some(config_json),
            }),
        };

        let result = plugin.execute(&tool_call, app.handle()).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_execute_list_directory() {
        let app = make_mock_app();
        let (plugin, tmp) = setup();
        std::fs::write(tmp.path().join("file1.txt"), "content").unwrap();

        let tmp_str = tmp.path().to_string_lossy().to_string();
        let tool_call = ToolCall {
            id: "call-ls".to_string(),
            name: "list_directory".to_string(),
            arguments: json!({ "path": tmp_str }),
            context: None,
        };

        let result = plugin.execute(&tool_call, app.handle()).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("file1.txt"));
    }

    #[tokio::test]
    async fn test_execute_search_files() {
        let app = make_mock_app();
        let (plugin, tmp) = setup();
        std::fs::write(tmp.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(tmp.path().join("readme.md"), "# readme").unwrap();

        let tmp_str = tmp.path().to_string_lossy().to_string();
        let tool_call = ToolCall {
            id: "call-search".to_string(),
            name: "search_files".to_string(),
            arguments: json!({ "path": tmp_str, "pattern": ".rs" }),
            context: None,
        };

        let result = plugin.execute(&tool_call, app.handle()).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("main.rs"));
        assert!(!result.content.contains("readme.md"));
    }

    #[tokio::test]
    async fn test_execute_unknown_tool() {
        let app = make_mock_app();
        let (plugin, _tmp) = setup();

        let tool_call = ToolCall {
            id: "call-3".to_string(),
            name: "delete_file".to_string(),
            arguments: json!({ "path": "test.txt" }),
            context: None,
        };

        let result = plugin.execute(&tool_call, app.handle()).await;
        assert!(result.is_err());
    }

    // --- read_file で画像自動判定 ---

    #[test]
    fn test_read_file_image_returns_base64_prefix() {
        let (plugin, tmp) = setup();
        // PNG ヘッダーのダミーバイト列を画像ファイルとして書き込み
        let fake_png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        std::fs::write(tmp.path().join("test.png"), &fake_png).unwrap();

        let result = plugin.read_file("test.png", &None);
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.starts_with("[IMAGE_BASE64]:"));

        // Base64部分をデコードして元データと一致するか確認
        let b64_part = content.strip_prefix("[IMAGE_BASE64]:").unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b64_part)
            .unwrap();
        assert_eq!(decoded, fake_png);
    }

    #[test]
    fn test_read_file_image_nonexistent() {
        let (plugin, _tmp) = setup();
        let result = plugin.read_file("nonexistent.png", &None);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_file_image_acl_denied() {
        let (plugin, _tmp) = setup();
        let acl = Some(make_acl(vec![("/other/dir", true, false)]));
        let result = plugin.read_file("test.png", &acl);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("アクセス拒否"));
    }

    #[tokio::test]
    async fn test_execute_read_file_image() {
        let app = make_mock_app();
        let (plugin, tmp) = setup();
        let fake_img = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG ヘッダー
        std::fs::write(tmp.path().join("photo.jpg"), &fake_img).unwrap();

        let tool_call = ToolCall {
            id: "call-img".to_string(),
            name: "read_file".to_string(),
            arguments: json!({ "path": "photo.jpg" }),
            context: None,
        };

        let result = plugin.execute(&tool_call, app.handle()).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.starts_with("[IMAGE_BASE64]:"));
    }

    #[test]
    fn test_read_file_text_not_base64() {
        let (plugin, tmp) = setup();
        std::fs::write(tmp.path().join("hello.txt"), "Hello World").unwrap();

        let result = plugin.read_file("hello.txt", &None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello World");
        // テキストファイルは [IMAGE_BASE64]: プレフィックスがつかない
    }
}
