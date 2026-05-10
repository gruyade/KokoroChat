use serde::{Deserialize, Serialize};
use serde_json::Value;

/// プラグインメタデータ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub tools: Vec<ToolDefinition>,
    /// プラグイン固有設定
    pub config: Option<Value>,
}

/// カスタムツールの実行方式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CustomToolType {
    Http,
    Cli,
}

impl CustomToolType {
    pub fn as_str(&self) -> &str {
        match self {
            CustomToolType::Http => "http",
            CustomToolType::Cli => "cli",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "http" => Some(CustomToolType::Http),
            "cli" => Some(CustomToolType::Cli),
            _ => None,
        }
    }
}

/// HTTP Webhook ツールの設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpToolConfig {
    pub url: String,
    #[serde(default = "default_http_method")]
    pub method: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

fn default_http_method() -> String {
    "POST".to_string()
}

/// CLI ツールの設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliToolConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// DBに保存されるカスタムツールレコード
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomToolRecord {
    pub id: String,
    pub name: String,
    pub tool_type: CustomToolType,
    pub description: String,
    pub parameters_schema: Value,
    pub config_json: Value,
    pub enabled: bool,
    pub created_at: String,
}

/// OpenAI Function Calling互換のツール定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema形式のパラメータ定義
    pub parameters: Value,
}

/// LLMからのtool_callリクエスト
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// ツール実行結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}
