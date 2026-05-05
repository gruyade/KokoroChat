// LLM Client - OpenAI互換API通信

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppError;
use crate::models::{ToolCall, ToolDefinition};

/// LLMクライアント接続設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMClientConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub temperature: f32,
}

/// メッセージロール
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// チャットメッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// LLMレスポンス — テキストまたはtool_call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LLMResponse {
    Text(String),
    ToolCalls(Vec<ToolCall>),
}

/// LLMクライアントtrait
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// 通常のチャット補完リクエスト（ツール定義オプション付き）
    async fn chat(
        &self,
        messages: &[ChatMessage],
        config: &LLMClientConfig,
        tools: Option<&[ToolDefinition]>,
    ) -> Result<LLMResponse, AppError>;

    /// ストリーミングチャット補完（コールバックでチャンクを受信）
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        config: &LLMClientConfig,
        callback: Box<dyn Fn(String) + Send>,
    ) -> Result<String, AppError>;

    /// 接続テスト
    async fn test_connection(&self, config: &LLMClientConfig) -> Result<(), AppError>;
}

/// OpenAI互換APIクライアント実装
pub struct OpenAICompatibleClient {
    http_client: Client,
}

impl OpenAICompatibleClient {
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
        }
    }

    /// リクエストボディを構築
    pub(crate) fn build_request_body(
        &self,
        messages: &[ChatMessage],
        config: &LLMClientConfig,
        tools: Option<&[ToolDefinition]>,
        stream: bool,
    ) -> Value {
        let messages_json: Vec<Value> = messages
            .iter()
            .map(|msg| {
                let mut obj = serde_json::json!({
                    "role": msg.role,
                    "content": msg.content,
                });
                if let Some(ref tool_call_id) = msg.tool_call_id {
                    obj["tool_call_id"] = Value::String(tool_call_id.clone());
                }
                obj
            })
            .collect();

        let mut body = serde_json::json!({
            "model": config.model,
            "messages": messages_json,
            "temperature": config.temperature,
            "stream": stream,
        });

        if let Some(tool_defs) = tools {
            if !tool_defs.is_empty() {
                let tools_json: Vec<Value> = tool_defs
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.parameters,
                            }
                        })
                    })
                    .collect();
                body["tools"] = Value::Array(tools_json);
            }
        }

        body
    }

    /// APIエンドポイントURLを構築
    fn build_url(config: &LLMClientConfig) -> String {
        let base = config.base_url.trim_end_matches('/');
        format!("{}/chat/completions", base)
    }

    /// レスポンスからLLMResponseをパース
    fn parse_response(response_body: &Value) -> Result<LLMResponse, AppError> {
        let choices = response_body["choices"]
            .as_array()
            .ok_or_else(|| AppError::LlmApi("Invalid response: missing choices".to_string()))?;

        if choices.is_empty() {
            return Err(AppError::LlmApi(
                "Invalid response: empty choices".to_string(),
            ));
        }

        let message = &choices[0]["message"];

        // tool_callsが存在する場合
        if let Some(tool_calls_arr) = message["tool_calls"].as_array() {
            if !tool_calls_arr.is_empty() {
                let tool_calls: Vec<ToolCall> = tool_calls_arr
                    .iter()
                    .map(|tc| {
                        let id = tc["id"].as_str().unwrap_or("").to_string();
                        let function = &tc["function"];
                        let name = function["name"].as_str().unwrap_or("").to_string();
                        let arguments_str = function["arguments"].as_str().unwrap_or("{}");
                        let arguments: Value =
                            serde_json::from_str(arguments_str).unwrap_or(Value::Object(
                                serde_json::Map::new(),
                            ));
                        ToolCall {
                            id,
                            name,
                            arguments,
                        }
                    })
                    .collect();
                return Ok(LLMResponse::ToolCalls(tool_calls));
            }
        }

        // テキストレスポンス
        let content = message["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(LLMResponse::Text(content))
    }

    /// SSEストリームからテキストをパース
    fn parse_sse_line(line: &str) -> Option<String> {
        let data = line.strip_prefix("data: ")?;

        if data.trim() == "[DONE]" {
            return None;
        }

        let json: Value = serde_json::from_str(data).ok()?;
        let delta = &json["choices"][0]["delta"];
        delta["content"].as_str().map(|s| s.to_string())
    }
}

impl Default for OpenAICompatibleClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMClient for OpenAICompatibleClient {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        config: &LLMClientConfig,
        tools: Option<&[ToolDefinition]>,
    ) -> Result<LLMResponse, AppError> {
        let url = Self::build_url(config);
        let body = self.build_request_body(messages, config, tools, false);

        let mut request = self.http_client.post(&url).json(&body);

        if let Some(ref api_key) = config.api_key {
            if !api_key.is_empty() {
                request = request.header("Authorization", format!("Bearer {}", api_key));
            }
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::LlmApi(format!(
                "API returned status {}: {}",
                status, error_body
            )));
        }

        let response_body: Value = response.json().await?;
        Self::parse_response(&response_body)
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        config: &LLMClientConfig,
        callback: Box<dyn Fn(String) + Send>,
    ) -> Result<String, AppError> {
        let url = Self::build_url(config);
        let body = self.build_request_body(messages, config, None, true);

        let mut request = self.http_client.post(&url).json(&body);

        if let Some(ref api_key) = config.api_key {
            if !api_key.is_empty() {
                request = request.header("Authorization", format!("Bearer {}", api_key));
            }
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::LlmApi(format!(
                "API returned status {}: {}",
                status, error_body
            )));
        }

        let mut full_text = String::new();
        let mut buffer = String::new();

        // バイトストリームとしてSSEを読み取り
        let bytes = response.bytes().await?;
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(chunk) = Self::parse_sse_line(line) {
                full_text.push_str(&chunk);
                buffer.push_str(&chunk);
                callback(buffer.clone());
                buffer.clear();
            }
        }

        Ok(full_text)
    }

    async fn test_connection(&self, config: &LLMClientConfig) -> Result<(), AppError> {
        let _messages = vec![ChatMessage {
            role: MessageRole::User,
            content: "Hi".to_string(),
            tool_call_id: None,
        }];

        let url = Self::build_url(config);
        let body = serde_json::json!({
            "model": config.model,
            "messages": [{"role": "user", "content": "Hi"}],
            "temperature": config.temperature,
            "stream": false,
            "max_tokens": 1,
        });

        let mut request = self.http_client.post(&url).json(&body);

        if let Some(ref api_key) = config.api_key {
            if !api_key.is_empty() {
                request = request.header("Authorization", format!("Bearer {}", api_key));
            }
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::LlmApi(format!(
                "Connection test failed (status {}): {}",
                status, error_body
            )));
        }

        // レスポンスが正常に返ればOK（内容は問わない）
        let _: Value = response.json().await.map_err(|e| {
            AppError::LlmApi(format!("Connection test: invalid response format: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ToolDefinition;

    #[test]
    fn test_build_request_body_basic() {
        let client = OpenAICompatibleClient::new();
        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: "You are a helpful assistant.".to_string(),
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_call_id: None,
            },
        ];
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            temperature: 0.7,
        };

        let body = client.build_request_body(&messages, &config, None, false);

        assert_eq!(body["model"], "gpt-4");
        let temp = body["temperature"].as_f64().unwrap();
        assert!((temp - 0.7f64).abs() < 1e-5, "temperature mismatch: {}", temp);
        assert_eq!(body["stream"], false);

        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "You are a helpful assistant.");
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[1]["content"], "Hello");

        // toolsフィールドは含まれない
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn test_build_request_body_with_tools() {
        let client = OpenAICompatibleClient::new();
        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: "What is 2+2?".to_string(),
            tool_call_id: None,
        }];
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: Some("sk-test".to_string()),
            temperature: 0.5,
        };
        let tools = vec![ToolDefinition {
            name: "calculator".to_string(),
            description: "Perform calculations".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string"}
                },
                "required": ["expression"]
            }),
        }];

        let body = client.build_request_body(&messages, &config, Some(&tools), false);

        assert_eq!(body["model"], "gpt-4");
        let tools_arr = body["tools"].as_array().unwrap();
        assert_eq!(tools_arr.len(), 1);
        assert_eq!(tools_arr[0]["type"], "function");
        assert_eq!(tools_arr[0]["function"]["name"], "calculator");
        assert_eq!(
            tools_arr[0]["function"]["description"],
            "Perform calculations"
        );
    }

    #[test]
    fn test_build_request_body_with_tool_call_id() {
        let client = OpenAICompatibleClient::new();
        let messages = vec![ChatMessage {
            role: MessageRole::Tool,
            content: "4".to_string(),
            tool_call_id: Some("call_123".to_string()),
        }];
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            temperature: 0.7,
        };

        let body = client.build_request_body(&messages, &config, None, false);

        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "tool");
        assert_eq!(msgs[0]["content"], "4");
        assert_eq!(msgs[0]["tool_call_id"], "call_123");
    }

    #[test]
    fn test_build_request_body_stream() {
        let client = OpenAICompatibleClient::new();
        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
            tool_call_id: None,
        }];
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            temperature: 0.7,
        };

        let body = client.build_request_body(&messages, &config, None, true);
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn test_build_url() {
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            temperature: 0.7,
        };
        assert_eq!(
            OpenAICompatibleClient::build_url(&config),
            "http://localhost:8080/v1/chat/completions"
        );

        // trailing slashの処理
        let config2 = LLMClientConfig {
            base_url: "http://localhost:8080/v1/".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            temperature: 0.7,
        };
        assert_eq!(
            OpenAICompatibleClient::build_url(&config2),
            "http://localhost:8080/v1/chat/completions"
        );
    }

    #[test]
    fn test_parse_response_text() {
        let response = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you?"
                }
            }]
        });

        let result = OpenAICompatibleClient::parse_response(&response).unwrap();
        match result {
            LLMResponse::Text(text) => assert_eq!(text, "Hello! How can I help you?"),
            _ => panic!("Expected LLMResponse::Text"),
        }
    }

    #[test]
    fn test_parse_response_tool_calls() {
        let response = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "calculator",
                            "arguments": "{\"expression\": \"2+2\"}"
                        }
                    }]
                }
            }]
        });

        let result = OpenAICompatibleClient::parse_response(&response).unwrap();
        match result {
            LLMResponse::ToolCalls(calls) => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].id, "call_abc123");
                assert_eq!(calls[0].name, "calculator");
                assert_eq!(calls[0].arguments["expression"], "2+2");
            }
            _ => panic!("Expected LLMResponse::ToolCalls"),
        }
    }

    #[test]
    fn test_parse_response_empty_choices() {
        let response = serde_json::json!({
            "choices": []
        });

        let result = OpenAICompatibleClient::parse_response(&response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_response_missing_choices() {
        let response = serde_json::json!({
            "id": "chatcmpl-123"
        });

        let result = OpenAICompatibleClient::parse_response(&response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_sse_line_content() {
        let line = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        let result = OpenAICompatibleClient::parse_sse_line(line);
        assert_eq!(result, Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_sse_line_done() {
        let line = "data: [DONE]";
        let result = OpenAICompatibleClient::parse_sse_line(line);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_sse_line_no_content() {
        let line = r#"data: {"choices":[{"delta":{}}]}"#;
        let result = OpenAICompatibleClient::parse_sse_line(line);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_sse_line_not_data() {
        let line = "event: message";
        let result = OpenAICompatibleClient::parse_sse_line(line);
        assert_eq!(result, None);
    }

    #[test]
    fn test_message_role_serialization() {
        let msg = ChatMessage {
            role: MessageRole::System,
            content: "test".to_string(),
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"system\""));

        let msg = ChatMessage {
            role: MessageRole::Assistant,
            content: "test".to_string(),
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"assistant\""));
    }

    #[test]
    fn test_chat_message_skip_none_tool_call_id() {
        let msg = ChatMessage {
            role: MessageRole::User,
            content: "hello".to_string(),
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("tool_call_id"));
    }

    #[test]
    fn test_chat_message_include_tool_call_id() {
        let msg = ChatMessage {
            role: MessageRole::Tool,
            content: "result".to_string(),
            tool_call_id: Some("call_123".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"tool_call_id\":\"call_123\""));
    }

    #[test]
    fn test_llm_client_config_serialization() {
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: Some("sk-test".to_string()),
            temperature: 0.7,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["base_url"], "http://localhost:8080/v1");
        assert_eq!(json["model"], "gpt-4");
        assert_eq!(json["api_key"], "sk-test");
        let temp = json["temperature"].as_f64().unwrap();
        assert!((temp - 0.7f64).abs() < 1e-5, "temperature mismatch: {}", temp);
    }

    #[test]
    fn test_llm_response_text_serialization() {
        let resp = LLMResponse::Text("Hello".to_string());
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("Text"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_llm_response_tool_calls_serialization() {
        let resp = LLMResponse::ToolCalls(vec![ToolCall {
            id: "call_1".to_string(),
            name: "search".to_string(),
            arguments: serde_json::json!({"query": "test"}),
        }]);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("ToolCalls"));
        assert!(json.contains("search"));
    }

    #[test]
    fn test_build_request_body_empty_tools() {
        let client = OpenAICompatibleClient::new();
        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
            tool_call_id: None,
        }];
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            temperature: 0.7,
        };
        let tools: Vec<ToolDefinition> = vec![];

        // 空のtools配列を渡した場合、toolsフィールドは含まれない
        let body = client.build_request_body(&messages, &config, Some(&tools), false);
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn test_parse_response_multiple_tool_calls() {
        let response = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        {
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "search",
                                "arguments": "{\"query\": \"rust\"}"
                            }
                        },
                        {
                            "id": "call_2",
                            "type": "function",
                            "function": {
                                "name": "calculator",
                                "arguments": "{\"expression\": \"1+1\"}"
                            }
                        }
                    ]
                }
            }]
        });

        let result = OpenAICompatibleClient::parse_response(&response).unwrap();
        match result {
            LLMResponse::ToolCalls(calls) => {
                assert_eq!(calls.len(), 2);
                assert_eq!(calls[0].name, "search");
                assert_eq!(calls[1].name, "calculator");
            }
            _ => panic!("Expected LLMResponse::ToolCalls"),
        }
    }
}
