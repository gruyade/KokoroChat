// カスタムツール実行基盤 — HTTP Webhook / CLI ハンドラ

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::error::AppError;
use crate::models::plugin::{ToolCall, ToolDefinition, ToolResult};
use crate::models::{CliToolConfig, CustomToolRecord, CustomToolType, HttpToolConfig};
use crate::plugin::system::PluginHandler;

/// HTTP タイムアウト（秒）
const HTTP_TIMEOUT_SECS: u64 = 30;
/// CLI タイムアウト（秒）
const CLI_TIMEOUT_SECS: u64 = 60;

/// HTTP Webhook 方式のカスタムツールハンドラ
pub struct HttpToolHandler {
    record: CustomToolRecord,
    config: HttpToolConfig,
    client: reqwest::Client,
}

impl HttpToolHandler {
    pub fn new(record: CustomToolRecord) -> Result<Self, AppError> {
        let config: HttpToolConfig = serde_json::from_value(record.config_json.clone())
            .map_err(|e| AppError::Plugin(format!("HTTP tool config parse error: {}", e)))?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .map_err(|e| AppError::Plugin(format!("HTTP client build error: {}", e)))?;

        Ok(Self {
            record,
            config,
            client,
        })
    }
}

#[async_trait]
impl PluginHandler for HttpToolHandler {
    fn name(&self) -> &str {
        &self.record.name
    }

    fn description(&self) -> &str {
        &self.record.description
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: self.record.name.clone(),
            description: self.record.description.clone(),
            parameters: self.record.parameters_schema.clone(),
        }]
    }

    async fn execute(&self, tool_call: &ToolCall, _app_handle: &tauri::AppHandle) -> Result<ToolResult, AppError> {
        let payload = json!({
            "tool_call_id": tool_call.id,
            "name": tool_call.name,
            "arguments": tool_call.arguments,
        });

        let mut request = match self.config.method.to_uppercase().as_str() {
            "POST" => self.client.post(&self.config.url),
            "PUT" => self.client.put(&self.config.url),
            "GET" => self.client.get(&self.config.url),
            _ => self.client.post(&self.config.url),
        };

        // ヘッダー追加
        for (key, value) in &self.config.headers {
            request = request.header(key.as_str(), value.as_str());
        }

        // JSON ボディ送信
        let response = request.json(&payload).send().await.map_err(|e| {
            if e.is_timeout() {
                AppError::Plugin(format!(
                    "HTTP tool '{}' timed out after {}s",
                    self.record.name, HTTP_TIMEOUT_SECS
                ))
            } else if e.is_connect() {
                AppError::Plugin(format!(
                    "HTTP tool '{}' connection failed: {}",
                    self.record.name, e
                ))
            } else {
                AppError::Plugin(format!(
                    "HTTP tool '{}' request failed: {}",
                    self.record.name, e
                ))
            }
        })?;

        let status = response.status();
        let body = response.text().await.map_err(|e| {
            AppError::Plugin(format!(
                "HTTP tool '{}' response read error: {}",
                self.record.name, e
            ))
        })?;

        if status.is_success() {
            Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: body,
                is_error: false,
            })
        } else {
            Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: format!("HTTP {} error: {}", status.as_u16(), body),
                is_error: true,
            })
        }
    }
}

/// CLI 方式のカスタムツールハンドラ
pub struct CliToolHandler {
    record: CustomToolRecord,
    config: CliToolConfig,
}

impl CliToolHandler {
    pub fn new(record: CustomToolRecord) -> Result<Self, AppError> {
        let config: CliToolConfig = serde_json::from_value(record.config_json.clone())
            .map_err(|e| AppError::Plugin(format!("CLI tool config parse error: {}", e)))?;

        Ok(Self { record, config })
    }

    /// テンプレート引数を展開する（`{{key}}` → 実際の値に置換）
    fn expand_args(&self, args: &[String], arguments: &Value) -> Vec<String> {
        args.iter()
            .map(|arg| {
                let mut result = arg.clone();
                if let Value::Object(map) = arguments {
                    for (key, value) in map {
                        let placeholder = format!("{{{{{}}}}}", key);
                        let replacement = match value {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        result = result.replace(&placeholder, &replacement);
                    }
                }
                result
            })
            .collect()
    }
}

#[async_trait]
impl PluginHandler for CliToolHandler {
    fn name(&self) -> &str {
        &self.record.name
    }

    fn description(&self) -> &str {
        &self.record.description
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: self.record.name.clone(),
            description: self.record.description.clone(),
            parameters: self.record.parameters_schema.clone(),
        }]
    }

    async fn execute(&self, tool_call: &ToolCall, _app_handle: &tauri::AppHandle) -> Result<ToolResult, AppError> {
        let expanded_args = self.expand_args(&self.config.args, &tool_call.arguments);

        let mut child = Command::new(&self.config.command)
            .args(&expanded_args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                AppError::Plugin(format!(
                    "CLI tool '{}' spawn failed: {}",
                    self.record.name, e
                ))
            })?;

        // stdin に引数 JSON を書き込む
        if let Some(mut stdin) = child.stdin.take() {
            let json_bytes = serde_json::to_vec(&tool_call.arguments).unwrap_or_default();
            let _ = stdin.write_all(&json_bytes).await;
            // stdin を閉じてプロセスに EOF を通知
            drop(stdin);
        }

        // タイムアウト付きで完了を待機
        let output = tokio::time::timeout(
            Duration::from_secs(CLI_TIMEOUT_SECS),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| {
            AppError::Plugin(format!(
                "CLI tool '{}' timed out after {}s",
                self.record.name, CLI_TIMEOUT_SECS
            ))
        })?
        .map_err(|e| {
            AppError::Plugin(format!(
                "CLI tool '{}' execution error: {}",
                self.record.name, e
            ))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: stdout,
                is_error: false,
            })
        } else {
            let error_msg = if stderr.is_empty() {
                format!(
                    "Process exited with code {}. stdout: {}",
                    output.status.code().unwrap_or(-1),
                    stdout
                )
            } else {
                format!(
                    "Process exited with code {}. stderr: {}",
                    output.status.code().unwrap_or(-1),
                    stderr
                )
            };
            Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: error_msg,
                is_error: true,
            })
        }
    }
}

/// カスタムツールエグゼキュータ — DBレコードから動的にハンドラを生成
pub struct CustomToolExecutor;

impl CustomToolExecutor {
    /// CustomToolRecord から適切な PluginHandler を生成
    pub fn create_handler(record: CustomToolRecord) -> Result<Box<dyn PluginHandler>, AppError> {
        match record.tool_type {
            CustomToolType::Http => Ok(Box::new(HttpToolHandler::new(record)?)),
            CustomToolType::Cli => Ok(Box::new(CliToolHandler::new(record)?)),
        }
    }

    /// 複数のレコードからハンドラを一括生成
    pub fn create_handlers(
        records: Vec<CustomToolRecord>,
    ) -> Vec<Result<Box<dyn PluginHandler>, AppError>> {
        records.into_iter().map(Self::create_handler).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CustomToolRecord;

    fn make_http_record() -> CustomToolRecord {
        CustomToolRecord {
            id: "test-http-001".to_string(),
            name: "test_http_tool".to_string(),
            tool_type: CustomToolType::Http,
            description: "Test HTTP tool".to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
            config_json: json!({
                "url": "https://httpbin.org/post",
                "method": "POST",
                "headers": {}
            }),
            enabled: true,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_cli_record() -> CustomToolRecord {
        CustomToolRecord {
            id: "test-cli-001".to_string(),
            name: "test_cli_tool".to_string(),
            tool_type: CustomToolType::Cli,
            description: "Test CLI tool".to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            }),
            config_json: json!({
                "command": "echo",
                "args": ["{{message}}"]
            }),
            enabled: true,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_http_handler_creation() {
        let record = make_http_record();
        let handler = HttpToolHandler::new(record).unwrap();
        assert_eq!(handler.name(), "test_http_tool");
        assert_eq!(handler.tools().len(), 1);
        assert_eq!(handler.tools()[0].name, "test_http_tool");
    }

    #[test]
    fn test_http_handler_invalid_config() {
        let mut record = make_http_record();
        record.config_json = json!("invalid");
        let result = HttpToolHandler::new(record);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_handler_creation() {
        let record = make_cli_record();
        let handler = CliToolHandler::new(record).unwrap();
        assert_eq!(handler.name(), "test_cli_tool");
        assert_eq!(handler.tools().len(), 1);
    }

    #[test]
    fn test_cli_handler_invalid_config() {
        let mut record = make_cli_record();
        record.config_json = json!(123);
        let result = CliToolHandler::new(record);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_expand_args() {
        let record = make_cli_record();
        let handler = CliToolHandler::new(record).unwrap();

        let args = vec![
            "--name".to_string(),
            "{{name}}".to_string(),
            "--count".to_string(),
            "{{count}}".to_string(),
        ];
        let arguments = json!({"name": "hello", "count": 42});

        let expanded = handler.expand_args(&args, &arguments);
        assert_eq!(expanded, vec!["--name", "hello", "--count", "42"]);
    }

    #[test]
    fn test_cli_expand_args_no_placeholders() {
        let record = make_cli_record();
        let handler = CliToolHandler::new(record).unwrap();

        let args = vec!["--verbose".to_string(), "fixed_arg".to_string()];
        let arguments = json!({"key": "value"});

        let expanded = handler.expand_args(&args, &arguments);
        assert_eq!(expanded, vec!["--verbose", "fixed_arg"]);
    }

    #[test]
    fn test_custom_tool_executor_create_http_handler() {
        let record = make_http_record();
        let handler = CustomToolExecutor::create_handler(record);
        assert!(handler.is_ok());
        assert_eq!(handler.unwrap().name(), "test_http_tool");
    }

    #[test]
    fn test_custom_tool_executor_create_cli_handler() {
        let record = make_cli_record();
        let handler = CustomToolExecutor::create_handler(record);
        assert!(handler.is_ok());
        assert_eq!(handler.unwrap().name(), "test_cli_tool");
    }

    #[test]
    fn test_custom_tool_executor_create_handlers_batch() {
        let records = vec![make_http_record(), make_cli_record()];
        let handlers = CustomToolExecutor::create_handlers(records);
        assert_eq!(handlers.len(), 2);
        assert!(handlers[0].is_ok());
        assert!(handlers[1].is_ok());
    }

    #[tokio::test]
    async fn test_cli_handler_execute_echo() {
        let app = tauri::test::mock_builder().build(tauri::generate_context!()).unwrap();
        let mut record = make_cli_record();
        // Windows では cmd /C echo を使用、Unix では echo を直接使用
        #[cfg(target_os = "windows")]
        {
            record.config_json = json!({
                "command": "cmd",
                "args": ["/C", "echo", "{{message}}"]
            });
        }
        #[cfg(not(target_os = "windows"))]
        {
            record.config_json = json!({
                "command": "echo",
                "args": ["{{message}}"]
            });
        }
        let handler = CliToolHandler::new(record).unwrap();

        let tool_call = ToolCall {
            id: "call-001".to_string(),
            name: "test_cli_tool".to_string(),
            arguments: json!({"message": "hello world"}),
            context: None,
        };

        let result = handler.execute(&tool_call, app.handle()).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("hello world"));
    }

    #[tokio::test]
    async fn test_cli_handler_execute_nonexistent_command() {
        let app = tauri::test::mock_builder().build(tauri::generate_context!()).unwrap();
        let mut record = make_cli_record();
        record.config_json = json!({
            "command": "nonexistent_command_xyz_12345",
            "args": []
        });
        let handler = CliToolHandler::new(record).unwrap();

        let tool_call = ToolCall {
            id: "call-002".to_string(),
            name: "test_cli_tool".to_string(),
            arguments: json!({}),
            context: None,
        };

        let result = handler.execute(&tool_call, app.handle()).await;
        assert!(result.is_err());
    }
}
