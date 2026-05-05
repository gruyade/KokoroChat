// ファイル読み書きプラグイン

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::models::plugin::{ToolCall, ToolDefinition, ToolResult};
use crate::plugin::system::PluginHandler;

/// ファイル操作プラグイン — ファイルの読み書きを行う（サンドボックス付き）
pub struct FileOpsPlugin {
    /// サンドボックスのベースディレクトリ（このディレクトリ配下のみアクセス許可）
    base_dir: PathBuf,
}

impl FileOpsPlugin {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// パスがサンドボックス内かどうか検証
    /// - ".." によるトラバーサルを禁止
    /// - base_dir 配下のみ許可
    fn validate_path(&self, path_str: &str) -> Result<PathBuf, String> {
        let path = Path::new(path_str);

        // ".." コンポーネントを含むパスを拒否
        for component in path.components() {
            if let std::path::Component::ParentDir = component {
                return Err("パスに '..' を含めることはできない".to_string());
            }
        }

        // 絶対パスの場合はbase_dir配下か確認
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_dir.join(path)
        };

        // 正規化してbase_dir配下か確認
        // canonicalize はファイルが存在しないと失敗するため、
        // コンポーネントベースで検証
        let base_str = self.base_dir.to_string_lossy().to_string();
        let resolved_str = resolved.to_string_lossy().to_string();

        if !resolved_str.starts_with(&base_str) {
            return Err(format!(
                "アクセス拒否: '{}' はサンドボックス外",
                path_str
            ));
        }

        Ok(resolved)
    }

    fn read_file(&self, path_str: &str) -> Result<String, String> {
        let path = self.validate_path(path_str)?;
        std::fs::read_to_string(&path)
            .map_err(|e| format!("ファイル読み込みエラー: {}", e))
    }

    fn write_file(&self, path_str: &str, content: &str) -> Result<String, String> {
        let path = self.validate_path(path_str)?;

        // 親ディレクトリが存在しない場合は作成
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("ディレクトリ作成エラー: {}", e))?;
        }

        std::fs::write(&path, content)
            .map_err(|e| format!("ファイル書き込みエラー: {}", e))?;

        Ok(format!("ファイルを書き込み完了: {}", path.display()))
    }
}

#[async_trait]
impl PluginHandler for FileOpsPlugin {
    fn name(&self) -> &str {
        "file_ops"
    }

    fn description(&self) -> &str {
        "ファイルの読み書きを行う"
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "read_file".to_string(),
                description: "ファイルを読み込む".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "読み込むファイルのパス"
                        }
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "ファイルに書き込む".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "書き込み先ファイルのパス"
                        },
                        "content": {
                            "type": "string",
                            "description": "書き込む内容"
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
        ]
    }

    async fn execute(&self, tool_call: &ToolCall) -> Result<ToolResult, AppError> {
        match tool_call.name.as_str() {
            "read_file" => {
                let path = tool_call
                    .arguments
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        AppError::Plugin("'path' パラメータが必要".to_string())
                    })?;

                let result = match self.read_file(path) {
                    Ok(content) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content,
                        is_error: false,
                    },
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
                    .ok_or_else(|| {
                        AppError::Plugin("'path' パラメータが必要".to_string())
                    })?;
                let content = tool_call
                    .arguments
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        AppError::Plugin("'content' パラメータが必要".to_string())
                    })?;

                let result = match self.write_file(path, content) {
                    Ok(msg) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content: msg,
                        is_error: false,
                    },
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
        let plugin = FileOpsPlugin::new(tmp.path().to_path_buf());
        (plugin, tmp)
    }

    #[test]
    fn test_plugin_metadata() {
        let (plugin, _tmp) = setup();
        assert_eq!(plugin.name(), "file_ops");
        assert_eq!(plugin.description(), "ファイルの読み書きを行う");
        assert_eq!(plugin.tools().len(), 2);

        let tools = plugin.tools();
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"write_file"));
    }

    #[test]
    fn test_path_validation_relative() {
        let (plugin, _tmp) = setup();
        // 相対パスはbase_dir配下に解決される
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

    #[test]
    fn test_write_and_read_file() {
        let (plugin, _tmp) = setup();

        let write_result = plugin.write_file("hello.txt", "Hello, World!");
        assert!(write_result.is_ok());

        let read_result = plugin.read_file("hello.txt");
        assert_eq!(read_result.unwrap(), "Hello, World!");
    }

    #[test]
    fn test_write_creates_subdirectories() {
        let (plugin, _tmp) = setup();

        let write_result = plugin.write_file("sub/dir/file.txt", "nested content");
        assert!(write_result.is_ok());

        let read_result = plugin.read_file("sub/dir/file.txt");
        assert_eq!(read_result.unwrap(), "nested content");
    }

    #[test]
    fn test_read_nonexistent_file() {
        let (plugin, _tmp) = setup();
        let result = plugin.read_file("nonexistent.txt");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_read_file() {
        let (plugin, _tmp) = setup();

        // まず書き込み
        plugin.write_file("test.txt", "test content").unwrap();

        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            arguments: json!({ "path": "test.txt" }),
        };

        let result = plugin.execute(&tool_call).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.content, "test content");
    }

    #[tokio::test]
    async fn test_execute_write_file() {
        let (plugin, _tmp) = setup();

        let tool_call = ToolCall {
            id: "call-2".to_string(),
            name: "write_file".to_string(),
            arguments: json!({ "path": "output.txt", "content": "written via execute" }),
        };

        let result = plugin.execute(&tool_call).await.unwrap();
        assert!(!result.is_error);

        // 書き込み確認
        let content = plugin.read_file("output.txt").unwrap();
        assert_eq!(content, "written via execute");
    }

    #[tokio::test]
    async fn test_execute_unknown_tool() {
        let (plugin, _tmp) = setup();

        let tool_call = ToolCall {
            id: "call-3".to_string(),
            name: "delete_file".to_string(),
            arguments: json!({ "path": "test.txt" }),
        };

        let result = plugin.execute(&tool_call).await;
        assert!(result.is_err());
    }
}
