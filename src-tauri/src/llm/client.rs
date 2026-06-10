// LLM Client - OpenAI互換API通信

use async_trait::async_trait;
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppError;
use crate::llm::think_tag_buffer::ThinkTagBuffer;
use crate::models::{LLMProvider, ToolCall, ToolDefinition};

/// Anthropic redacted_thinking ブロック検出時に蓄積するマーカー文字列
pub const REDACTED_THINKING_MARKER: &str = "[REDACTED_THINKING]";

/// LLMクライアント接続設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMClientConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub temperature: f32,
    /// プロバイダー種別（API形式判定に使用）
    #[serde(default)]
    pub provider: Option<LLMProvider>,
}

/// API通信戦略
#[derive(Debug, Clone, PartialEq)]
pub enum ApiStrategy {
    OpenAI,
    Gemini,
    Anthropic,
}

/// プロバイダーとエンドポイント設定からAPI形式を決定
pub fn resolve_api_strategy(config: &LLMClientConfig) -> ApiStrategy {
    // Google: OpenAI互換エンドポイント(/v1beta/openai)を使用するためOpenAI形式
    // Anthropic: ネイティブ Messages API を使用
    match config.provider {
        Some(LLMProvider::Anthropic) => ApiStrategy::Anthropic,
        Some(LLMProvider::Google)
        | Some(LLMProvider::Openai)
        | Some(LLMProvider::OpenaiCompatible)
        | None => ApiStrategy::OpenAI,
    }
}

/// 指定プロバイダーのデフォルトエンドポイントかどうかを判定
pub fn is_default_endpoint(base_url: &str, provider: LLMProvider) -> bool {
    let url = base_url.trim_end_matches('/');
    match provider {
        LLMProvider::Google => url.is_empty() || url.contains("generativelanguage.googleapis.com"),
        LLMProvider::Anthropic => url.is_empty() || url.contains("api.anthropic.com"),
        _ => true,
    }
}

/// Gemini APIリクエストボディを構築
pub fn build_gemini_request(
    messages: &[ChatMessage],
    config: &LLMClientConfig,
    tools: Option<&[ToolDefinition]>,
) -> Value {
    let contents: Vec<Value> = messages
        .iter()
        .filter(|m| m.role != MessageRole::System)
        .map(|m| {
            let role = match m.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "model",
                _ => "user",
            };
            serde_json::json!({
                "role": role,
                "parts": [{"text": m.content}]
            })
        })
        .collect();

    let system_instruction = messages
        .iter()
        .find(|m| m.role == MessageRole::System)
        .map(|m| serde_json::json!({"parts": [{"text": m.content}]}));

    let mut body = serde_json::json!({
        "contents": contents,
        "generationConfig": {
            "temperature": config.temperature,
        }
    });

    if let Some(si) = system_instruction {
        body["systemInstruction"] = si;
    }

    if let Some(tool_defs) = tools {
        if !tool_defs.is_empty() {
            let function_declarations: Vec<Value> = tool_defs
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::json!([{
                "function_declarations": function_declarations
            }]);
        }
    }

    body
}

/// Gemini用エンドポイントURL構築
pub fn build_gemini_url(config: &LLMClientConfig) -> String {
    let base = if config.base_url.is_empty() {
        "https://generativelanguage.googleapis.com/v1beta"
    } else {
        config.base_url.trim_end_matches('/')
    };
    format!("{}/models/{}:generateContent", base, config.model)
}

/// Gemini用ストリーミングエンドポイントURL構築
pub fn build_gemini_stream_url(config: &LLMClientConfig) -> String {
    let base = if config.base_url.is_empty() {
        "https://generativelanguage.googleapis.com/v1beta"
    } else {
        config.base_url.trim_end_matches('/')
    };
    format!(
        "{}/models/{}:streamGenerateContent?alt=sse",
        base, config.model
    )
}

/// Anthropic Messages APIリクエストボディを構築
pub fn build_anthropic_request(
    messages: &[ChatMessage],
    config: &LLMClientConfig,
    tools: Option<&[ToolDefinition]>,
) -> Value {
    let system_text = messages
        .iter()
        .filter(|m| m.role == MessageRole::System)
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let api_messages: Vec<Value> = messages
        .iter()
        .filter(|m| m.role != MessageRole::System)
        .map(|m| {
            let role = match m.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                _ => "user",
            };
            serde_json::json!({
                "role": role,
                "content": m.content
            })
        })
        .collect();

    let mut body = serde_json::json!({
        "model": config.model,
        "messages": api_messages,
        "temperature": config.temperature,
        "max_tokens": 4096,
    });

    if !system_text.is_empty() {
        body["system"] = Value::String(system_text);
    }

    if let Some(tool_defs) = tools {
        if !tool_defs.is_empty() {
            let tools_json: Vec<Value> = tool_defs
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect();
            body["tools"] = Value::Array(tools_json);
        }
    }

    body
}

/// Anthropic用エンドポイントURL構築
pub fn build_anthropic_url(config: &LLMClientConfig) -> String {
    let base = if config.base_url.is_empty() {
        "https://api.anthropic.com/v1"
    } else {
        config.base_url.trim_end_matches('/')
    };
    format!("{}/messages", base)
}

/// Gemini APIレスポンスをパースしてLLMResponseを返す
pub fn parse_gemini_response(body: &Value) -> Result<LLMResponse, AppError> {
    let candidates = body["candidates"].as_array();
    if candidates.map_or(true, |c| c.is_empty()) {
        return Err(AppError::LlmApi(
            "Gemini response has no candidates (possibly filtered by safety settings)".to_string(),
        ));
    }

    // parts から thought: true のテキストを thinking として抽出し、通常テキストのみ結合
    let parts = body["candidates"][0]["content"]["parts"].as_array();
    let (text, thinking) = match parts {
        Some(parts) => {
            let text = parts
                .iter()
                .filter(|p| p.get("thought").and_then(|v| v.as_bool()) != Some(true))
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join("");
            let thinking_text = parts
                .iter()
                .filter(|p| p.get("thought").and_then(|v| v.as_bool()) == Some(true))
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join("");
            let thinking = if thinking_text.is_empty() { None } else { Some(thinking_text) };
            (text, thinking)
        }
        None => (String::new(), None),
    };

    Ok(LLMResponse::Text { content: text, thinking })
}

/// Anthropic Messages APIレスポンスをパースしてLLMResponseを返す
pub fn parse_anthropic_response(body: &Value) -> Result<LLMResponse, AppError> {
    let content = body["content"].as_array().ok_or_else(|| {
        AppError::LlmApi("Invalid Anthropic response: missing 'content' array".to_string())
    })?;

    // thinking / redacted_thinking ブロックからthinking contentを抽出（出現順序維持）
    let mut thinking_parts: Vec<String> = Vec::new();
    for block in content.iter() {
        match block["type"].as_str() {
            Some("thinking") => {
                if let Some(text) = block["thinking"].as_str() {
                    if !text.is_empty() {
                        thinking_parts.push(text.to_string());
                    }
                }
            }
            Some("redacted_thinking") => {
                thinking_parts.push(REDACTED_THINKING_MARKER.to_string());
            }
            _ => {}
        }
    }

    // type: "text" のブロックのみ抽出（thinking / redacted_thinking / tool_use を除外）
    let text = content
        .iter()
        .filter(|block| block["type"].as_str() == Some("text"))
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("");

    let thinking = if thinking_parts.is_empty() {
        None
    } else {
        Some(thinking_parts.join(""))
    };

    Ok(LLMResponse::Text { content: text, thinking })
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
    /// 画像データ（base64エンコード）のリスト — Vision API用
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub images: Option<Vec<String>>,
}

/// LLMレスポンス — テキストまたはtool_call（thinking content付き）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LLMResponse {
    Text {
        content: String,
        thinking: Option<String>,
    },
    ToolCalls {
        calls: Vec<ToolCall>,
        thinking: Option<String>,
    },
}

impl LLMResponse {
    /// テキストを取得（ToolCallsの場合は空文字列を返す）
    pub fn text(&self) -> &str {
        match self {
            LLMResponse::Text { content, .. } => content,
            LLMResponse::ToolCalls { .. } => "",
        }
    }

    /// テキストを消費して取得（ToolCallsの場合は空文字列を返す）
    pub fn into_text(self) -> String {
        match self {
            LLMResponse::Text { content, .. } => content,
            LLMResponse::ToolCalls { .. } => String::new(),
        }
    }

    /// ToolCallsかどうか
    pub fn is_tool_calls(&self) -> bool {
        matches!(self, LLMResponse::ToolCalls { .. })
    }
}

/// ストリーミングコールバック型
/// text_callback: 通常テキストチャンク
/// thinking_callback: thinking/reasoning チャンク
pub type StreamCallbacks = (
    Box<dyn Fn(String) + Send>,      // text_callback
    Box<dyn Fn(String) + Send>,      // thinking_callback
);

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
    /// テキストチャンクはcallbacks.0で逐次送信し、Tool Callはバッファリングして最終結果として返す。
    /// callbacks.1はthinking/reasoningチャンク用。
    /// 戻り値: LLMResponse::Text { content: 全テキスト, thinking } または LLMResponse::ToolCalls { calls: バッファされたツール呼び出し, thinking }
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        config: &LLMClientConfig,
        tools: Option<&[ToolDefinition]>,
        callbacks: StreamCallbacks,
    ) -> Result<LLMResponse, AppError>;

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
            .flat_map(|msg| {
                let is_tool = matches!(msg.role, MessageRole::Tool);

                if is_tool {
                    // OpenAI互換API仕様: role=tool のメッセージは文字列contentのみ許可
                    // 画像が含まれる場合は別途 role=user のメッセージとして追加する
                    let mut tool_obj = serde_json::json!({
                        "role": msg.role,
                        "content": msg.content,
                    });
                    if let Some(ref tool_call_id) = msg.tool_call_id {
                        tool_obj["tool_call_id"] = Value::String(tool_call_id.clone());
                    }

                    if let Some(ref images) = msg.images {
                        if !images.is_empty() {
                            let image_parts: Vec<Value> = images
                                .iter()
                                .map(|img_base64| {
                                    serde_json::json!({
                                        "type": "image_url",
                                        "image_url": {
                                            "url": format!("data:image/png;base64,{}", img_base64),
                                        }
                                    })
                                })
                                .collect();
                            let user_img_obj = serde_json::json!({
                                "role": "user",
                                "content": image_parts,
                            });
                            return vec![tool_obj, user_img_obj];
                        }
                    }
                    vec![tool_obj]
                } else {
                    let mut obj = if let Some(ref images) = msg.images {
                        if !images.is_empty() {
                            // Vision API形式: content を配列にする
                            let mut content_parts: Vec<Value> = Vec::new();
                            if !msg.content.is_empty() {
                                content_parts.push(serde_json::json!({
                                    "type": "text",
                                    "text": msg.content,
                                }));
                            }
                            for img_base64 in images {
                                content_parts.push(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": {
                                        "url": format!("data:image/png;base64,{}", img_base64),
                                    }
                                }));
                            }
                            serde_json::json!({
                                "role": msg.role,
                                "content": content_parts,
                            })
                        } else {
                            serde_json::json!({
                                "role": msg.role,
                                "content": msg.content,
                            })
                        }
                    } else {
                        serde_json::json!({
                            "role": msg.role,
                            "content": msg.content,
                        })
                    };
                    if let Some(ref tool_call_id) = msg.tool_call_id {
                        obj["tool_call_id"] = Value::String(tool_call_id.clone());
                    }
                    vec![obj]
                }
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
        let base = if config.base_url.is_empty() {
            match config.provider {
                Some(LLMProvider::Google) => {
                    "https://generativelanguage.googleapis.com/v1beta/openai"
                }
                Some(LLMProvider::Openai) => "https://api.openai.com/v1",
                _ => "",
            }
        } else {
            config.base_url.trim_end_matches('/')
        };
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
                        let arguments: Value = serde_json::from_str(arguments_str)
                            .unwrap_or(Value::Object(serde_json::Map::new()));
                        ToolCall {
                            id,
                            name,
                            arguments,
                            context: None,
                        }
                    })
                    .collect();
                return Ok(LLMResponse::ToolCalls { calls: tool_calls, thinking: None });
            }
        }

        // テキストレスポンス（reasoning_content フィールドは無視）
        let content = message["content"].as_str().unwrap_or("");
        let cleaned = Self::strip_think_tags(content);

        Ok(LLMResponse::Text { content: cleaned, thinking: None })
    }

    /// `<think>...</think>` タグとその内容を除去する（非ストリーミング用）。
    /// 改行を跨ぐ thinking ブロック、閉じタグのみ残っているケース、
    /// 未閉じの開始タグ以降の全除去に対応。
    fn strip_think_tags(s: &str) -> String {
        let mut result = String::new();
        let mut remaining = s;

        // 閉じタグだけ先頭に残っているケース（前のレスポンスの think が跨いだ等）
        if let Some(end) = remaining.find("</think>") {
            if remaining[..end].find("<think>").is_none() {
                remaining = &remaining[end + "</think>".len()..];
            }
        }

        loop {
            match remaining.find("<think>") {
                Some(start) => {
                    // 開始タグの前までを結果に追加
                    result.push_str(&remaining[..start]);
                    let after_open = &remaining[start + "<think>".len()..];
                    match after_open.find("</think>") {
                        Some(end) => {
                            // 完結した think ブロック → スキップして続行
                            remaining = &after_open[end + "</think>".len()..];
                        }
                        None => {
                            // 閉じタグなし → 開始タグ以降を全て除去して終了
                            break;
                        }
                    }
                }
                None => {
                    // タグなし → 残り全てを結果に追加
                    result.push_str(remaining);
                    break;
                }
            }
        }

        result
    }

    /// SSEストリームからテキストをパース（OpenAI形式）
    fn parse_sse_line(line: &str) -> Option<String> {
        let data = line.strip_prefix("data: ")?;

        if data.trim() == "[DONE]" {
            return None;
        }

        let json: Value = serde_json::from_str(data).ok()?;
        let delta = &json["choices"][0]["delta"];
        delta["content"].as_str().map(|s| s.to_string())
    }

    /// SSEストリームからテキストをパース（Gemini形式）
    fn parse_gemini_sse_line(line: &str) -> Option<String> {
        let data = line.strip_prefix("data: ")?;

        let json: Value = serde_json::from_str(data).ok()?;
        json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
    }

    /// SSEストリームからテキストをパース（Anthropic形式）
    fn parse_anthropic_sse_line(line: &str) -> Option<String> {
        let data = line.strip_prefix("data: ")?;

        let json: Value = serde_json::from_str(data).ok()?;

        // content_block_deltaイベントのdelta.textを抽出
        if json["type"].as_str() == Some("content_block_delta") {
            return json["delta"]["text"].as_str().map(|s| s.to_string());
        }

        None
    }
}

/// ストリーミング中のTool Callバッファ（OpenAI形式）
/// OpenAIのSSEでは tool_calls が delta.tool_calls 配列として index 付きで送られる
#[derive(Debug, Default)]
struct OpenAIToolCallBuffer {
    /// index -> (id, function_name, arguments_buffer)
    entries: std::collections::HashMap<usize, (String, String, String)>,
}

impl OpenAIToolCallBuffer {
    fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    /// SSEチャンクの delta.tool_calls 配列を処理してバッファに蓄積
    fn process_delta(&mut self, tool_calls_arr: &[Value]) {
        for tc in tool_calls_arr {
            let index = tc["index"].as_u64().unwrap_or(0) as usize;
            let entry = self
                .entries
                .entry(index)
                .or_insert_with(|| (String::new(), String::new(), String::new()));

            if let Some(id) = tc["id"].as_str() {
                if !id.is_empty() {
                    entry.0 = id.to_string();
                }
            }
            if let Some(name) = tc["function"]["name"].as_str() {
                if !name.is_empty() {
                    entry.1 = name.to_string();
                }
            }
            if let Some(args) = tc["function"]["arguments"].as_str() {
                entry.2.push_str(args);
            }
        }
    }

    /// バッファされたデータが存在するか
    fn has_tool_calls(&self) -> bool {
        !self.entries.is_empty()
    }

    /// バッファからToolCallのVecを生成
    fn into_tool_calls(self) -> Vec<ToolCall> {
        let mut entries: Vec<(usize, (String, String, String))> =
            self.entries.into_iter().collect();
        entries.sort_by_key(|(idx, _)| *idx);

        entries
            .into_iter()
            .map(|(_, (id, name, arguments_str))| {
                let arguments: Value = serde_json::from_str(&arguments_str)
                    .unwrap_or(Value::Object(serde_json::Map::new()));
                ToolCall {
                    id,
                    name,
                    arguments,
                    context: None,
                }
            })
            .collect()
    }
}

/// ストリーミング中のTool Callバッファ（Anthropic形式）
/// Anthropicでは content_block_start (type: tool_use) と content_block_delta (type: input_json_delta) で送られる
#[derive(Debug, Default)]
struct AnthropicToolCallBuffer {
    /// index -> (id, name, input_json_buffer)
    entries: std::collections::HashMap<usize, (String, String, String)>,
}

impl AnthropicToolCallBuffer {
    fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    /// content_block_start イベントを処理
    fn process_block_start(&mut self, index: usize, id: &str, name: &str) {
        self.entries
            .insert(index, (id.to_string(), name.to_string(), String::new()));
    }

    /// content_block_delta (input_json_delta) イベントを処理
    fn process_input_delta(&mut self, index: usize, partial_json: &str) {
        if let Some(entry) = self.entries.get_mut(&index) {
            entry.2.push_str(partial_json);
        }
    }

    /// バッファされたデータが存在するか
    fn has_tool_calls(&self) -> bool {
        !self.entries.is_empty()
    }

    /// バッファからToolCallのVecを生成
    fn into_tool_calls(self) -> Vec<ToolCall> {
        let mut entries: Vec<(usize, (String, String, String))> =
            self.entries.into_iter().collect();
        entries.sort_by_key(|(idx, _)| *idx);

        entries
            .into_iter()
            .map(|(_, (id, name, input_str))| {
                let arguments: Value = serde_json::from_str(&input_str)
                    .unwrap_or(Value::Object(serde_json::Map::new()));
                ToolCall {
                    id,
                    name,
                    arguments,
                    context: None,
                }
            })
            .collect()
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
        let strategy = resolve_api_strategy(config);

        match strategy {
            ApiStrategy::Gemini => {
                let url = build_gemini_url(config);
                let body = build_gemini_request(messages, config, tools);

                let api_key = config.api_key.as_deref().unwrap_or("");
                let url_with_key = format!("{}?key={}", url, api_key);

                let response = self
                    .http_client
                    .post(&url_with_key)
                    .json(&body)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status = response.status();
                    let error_body = response.text().await.unwrap_or_default();
                    return Err(AppError::LlmApi(format!(
                        "Gemini API returned status {}: {}",
                        status, error_body
                    )));
                }

                let response_body: Value = response.json().await?;
                parse_gemini_response(&response_body)
            }
            ApiStrategy::Anthropic => {
                let url = build_anthropic_url(config);
                let body = build_anthropic_request(messages, config, tools);

                let api_key = config.api_key.as_deref().unwrap_or("");
                let response = self
                    .http_client
                    .post(&url)
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .json(&body)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status = response.status();
                    let error_body = response.text().await.unwrap_or_default();
                    return Err(AppError::LlmApi(format!(
                        "Anthropic API returned status {}: {}",
                        status, error_body
                    )));
                }

                let response_body: Value = response.json().await?;
                parse_anthropic_response(&response_body)
            }
            ApiStrategy::OpenAI => {
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
        }
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        config: &LLMClientConfig,
        tools: Option<&[ToolDefinition]>,
        callbacks: StreamCallbacks,
    ) -> Result<LLMResponse, AppError> {
        let strategy = resolve_api_strategy(config);
        let callback = callbacks.0;
        let thinking_callback = callbacks.1;

        match strategy {
            ApiStrategy::Gemini => {
                let url = build_gemini_stream_url(config);
                let body = build_gemini_request(messages, config, tools);

                let api_key = config.api_key.as_deref().unwrap_or("");
                let url_with_key = format!("{}&key={}", url, api_key);

                let response = self
                    .http_client
                    .post(&url_with_key)
                    .json(&body)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status = response.status();
                    let error_body = response.text().await.unwrap_or_default();
                    return Err(AppError::LlmApi(format!(
                        "Gemini streaming API returned status {}: {}",
                        status, error_body
                    )));
                }

                let mut full_text = String::new();
                let mut full_thinking = String::new();
                let mut tool_calls: Vec<ToolCall> = Vec::new();
                let bytes = response.bytes().await?;
                let text = String::from_utf8_lossy(&bytes);

                for line in text.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    // Gemini: parts を解析（thinking part 検出 + functionCall 検出）
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                            if let Some(parts) =
                                json["candidates"][0]["content"]["parts"].as_array()
                            {
                                for part in parts {
                                    // functionCall 検出
                                    if let Some(fc) = part.get("functionCall") {
                                        let name = fc["name"].as_str().unwrap_or("").to_string();
                                        let args = fc
                                            .get("args")
                                            .cloned()
                                            .unwrap_or(Value::Object(serde_json::Map::new()));
                                        tool_calls.push(ToolCall {
                                            id: format!("gemini_call_{}", tool_calls.len()),
                                            name,
                                            arguments: args,
                                            context: None,
                                        });
                                        continue;
                                    }

                                    // Thinking part（thought: true フラグ）→ thinking_callback で通知
                                    if part.get("thought").and_then(|v| v.as_bool()) == Some(true)
                                    {
                                        if let Some(thought_text) = part["text"].as_str() {
                                            if !thought_text.is_empty() {
                                                full_thinking.push_str(thought_text);
                                                thinking_callback(thought_text.to_string());
                                            }
                                        }
                                        continue;
                                    }

                                    // 通常テキストパーツ
                                    if let Some(text_chunk) = part["text"].as_str() {
                                        if !text_chunk.is_empty() {
                                            full_text.push_str(text_chunk);
                                            callback(text_chunk.to_string());
                                        }
                                    }
                                }
                                continue;
                            }
                        }
                    }

                    // フォールバック: parse_gemini_sse_line（parts ベースで処理できなかったケース）
                    if let Some(chunk) = Self::parse_gemini_sse_line(line) {
                        full_text.push_str(&chunk);
                        callback(chunk);
                    }
                }

                let thinking = if full_thinking.is_empty() { None } else { Some(full_thinking) };
                if !tool_calls.is_empty() {
                    Ok(LLMResponse::ToolCalls { calls: tool_calls, thinking })
                } else {
                    Ok(LLMResponse::Text { content: full_text, thinking })
                }
            }
            ApiStrategy::Anthropic => {
                let url = build_anthropic_url(config);
                let mut body = build_anthropic_request(messages, config, tools);
                // ストリーミング有効化
                body["stream"] = Value::Bool(true);

                let api_key = config.api_key.as_deref().unwrap_or("");
                let response = self
                    .http_client
                    .post(&url)
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .json(&body)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status = response.status();
                    let error_body = response.text().await.unwrap_or_default();
                    return Err(AppError::LlmApi(format!(
                        "Anthropic streaming API returned status {}: {}",
                        status, error_body
                    )));
                }

                let mut full_text = String::new();
                let mut full_thinking = String::new();
                let mut tool_buffer = AnthropicToolCallBuffer::new();
                // Thinking ブロックのインデックスを追跡（自動判定）
                let mut thinking_block_indices: std::collections::HashSet<usize> =
                    std::collections::HashSet::new();
                let bytes = response.bytes().await?;
                let text = String::from_utf8_lossy(&bytes);

                for line in text.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    // Tool Call & Thinking ブロック処理
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                            let event_type = json["type"].as_str().unwrap_or("");

                            match event_type {
                                "content_block_start" => {
                                    let index = json["index"].as_u64().unwrap_or(0) as usize;
                                    let content_block = &json["content_block"];
                                    let block_type =
                                        content_block["type"].as_str().unwrap_or("");

                                    match block_type {
                                        "tool_use" => {
                                            let id = content_block["id"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string();
                                            let name = content_block["name"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string();
                                            tool_buffer.process_block_start(index, &id, &name);
                                        }
                                        "thinking" => {
                                            thinking_block_indices.insert(index);
                                        }
                                        "redacted_thinking" => {
                                            thinking_block_indices.insert(index);
                                            // redacted_thinking ブロック検出時にマーカーを即座に通知
                                            let marker = REDACTED_THINKING_MARKER.to_string();
                                            full_thinking.push_str(&marker);
                                            thinking_callback(marker);
                                        }
                                        _ => {}
                                    }
                                    continue;
                                }
                                "content_block_delta" => {
                                    let index = json["index"].as_u64().unwrap_or(0) as usize;
                                    let delta = &json["delta"];
                                    let delta_type = delta["type"].as_str().unwrap_or("");

                                    match delta_type {
                                        "input_json_delta" => {
                                            let partial =
                                                delta["partial_json"].as_str().unwrap_or("");
                                            tool_buffer.process_input_delta(index, partial);
                                            continue;
                                        }
                                        "thinking_delta" => {
                                            // thinking ブロックのテキストデルタを thinking_callback で通知
                                            if let Some(thinking_text) = delta["thinking"].as_str() {
                                                if !thinking_text.is_empty() {
                                                    full_thinking.push_str(thinking_text);
                                                    thinking_callback(thinking_text.to_string());
                                                }
                                            }
                                            continue;
                                        }
                                        "signature_delta" => {
                                            // signature はスキップ
                                            continue;
                                        }
                                        "text_delta" => {
                                            // Thinking ブロックに属するテキストはスキップ
                                            if thinking_block_indices.contains(&index) {
                                                continue;
                                            }
                                            // 通常テキスト: フォールスルーして parse_anthropic_sse_line で処理
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    if let Some(chunk) = Self::parse_anthropic_sse_line(line) {
                        full_text.push_str(&chunk);
                        callback(chunk);
                    }
                }

                let thinking = if full_thinking.is_empty() {
                    None
                } else {
                    Some(full_thinking)
                };

                if tool_buffer.has_tool_calls() {
                    Ok(LLMResponse::ToolCalls { calls: tool_buffer.into_tool_calls(), thinking })
                } else {
                    Ok(LLMResponse::Text { content: full_text, thinking })
                }
            }
            ApiStrategy::OpenAI => {
                let url = Self::build_url(config);
                let body = self.build_request_body(messages, config, tools, true);

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
                let mut thinking_buffer = String::new();
                let mut tool_buffer = OpenAIToolCallBuffer::new();
                // <think>タグベースのthinking content検出用バッファ
                let mut think_tag_buffer = ThinkTagBuffer::new();

                let bytes = response.bytes().await?;
                let text = String::from_utf8_lossy(&bytes);

                for line in text.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    // Tool Call バッファリング & reasoning 検出処理
                    if let Some(data) = line.strip_prefix("data: ") {
                        debug!("[SSE raw] data: {}", crate::utils::safe_truncate_bytes(data, 500));
                        if data.trim() != "[DONE]" {
                            if let Ok(json) = serde_json::from_str::<Value>(data) {
                                let delta = &json["choices"][0]["delta"];

                                // Tool Call 検出
                                if let Some(tool_calls_arr) = delta["tool_calls"].as_array() {
                                    tool_buffer.process_delta(tool_calls_arr);
                                    continue;
                                }

                                // reasoning_content / reasoning フィールドの検出
                                // 対応プロバイダー: OpenAI o3/o4-mini, DeepSeek, LM Studio等
                                let reasoning_chunk = delta
                                    .get("reasoning_content")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                    .or_else(|| {
                                        delta
                                            .get("reasoning")
                                            .and_then(|v| v.as_str())
                                            .filter(|s| !s.is_empty())
                                    });

                                if let Some(reasoning) = reasoning_chunk {
                                    // thinking_callbackで通知し、バッファに蓄積
                                    thinking_callback(reasoning.to_string());
                                    thinking_buffer.push_str(reasoning);

                                    // 同一チャンクに content が存在する場合はテキストとして処理続行
                                    let has_content = delta
                                        .get("content")
                                        .and_then(|v| v.as_str())
                                        .is_some_and(|s| !s.is_empty());
                                    if !has_content {
                                        continue;
                                    }
                                }
                            }
                        } else {
                            debug!("[SSE] received [DONE]");
                        }
                    }

                    if let Some(chunk) = Self::parse_sse_line(line) {
                        // <think>...</think> タグによるthinking content検出
                        // ThinkTagBufferを使用してチャンク境界をまたぐタグに対応
                        let (text_parts, thinking_parts) = think_tag_buffer.process_chunk(&chunk);

                        // デバッグログ: ストリーム受信内容の追跡
                        if !chunk.is_empty() {
                            debug!("[ThinkTagBuffer] input chunk({} bytes, inside_think={}): {:?}", chunk.len(), think_tag_buffer.is_inside_think(), crate::utils::safe_truncate_bytes(&chunk, 200));
                        }
                        if !thinking_parts.is_empty() {
                            debug!("[ThinkTagBuffer] -> thinking_parts: {:?}", thinking_parts.iter().map(|s| crate::utils::safe_truncate_bytes(s, 100)).collect::<Vec<_>>());
                        }
                        if !text_parts.is_empty() {
                            debug!("[ThinkTagBuffer] -> text_parts: {:?}", text_parts.iter().map(|s| crate::utils::safe_truncate_bytes(s, 100)).collect::<Vec<_>>());
                        }
                        if text_parts.is_empty() && thinking_parts.is_empty() && !chunk.is_empty() {
                            debug!("[ThinkTagBuffer] -> ALL PENDING (nothing emitted!) inside_think={}", think_tag_buffer.is_inside_think());
                        }

                        for thinking_part in thinking_parts {
                            thinking_callback(thinking_part.clone());
                            thinking_buffer.push_str(&thinking_part);
                        }

                        for text_part in text_parts {
                            full_text.push_str(&text_part);
                            callback(text_part);
                        }
                    }
                }

                // ストリーム終了: ThinkTagBufferのフラッシュ
                let (flush_text_parts, flush_thinking_parts) = think_tag_buffer.flush();
                if !flush_text_parts.is_empty() || !flush_thinking_parts.is_empty() {
                    debug!("[ThinkTagBuffer] FLUSH: text_parts={:?}, thinking_parts={:?}", flush_text_parts, flush_thinking_parts);
                }
                for thinking_part in flush_thinking_parts {
                    thinking_callback(thinking_part.clone());
                    thinking_buffer.push_str(&thinking_part);
                }
                for text_part in flush_text_parts {
                    full_text.push_str(&text_part);
                    callback(text_part);
                }

                let thinking = if thinking_buffer.is_empty() {
                    None
                } else {
                    Some(thinking_buffer)
                };

                debug!("[OpenAI stream] DONE. full_text({} bytes): {:?}", full_text.len(), crate::utils::safe_truncate_bytes(&full_text, 300));
                debug!("[OpenAI stream] thinking: {:?}", thinking.as_ref().map(|s| crate::utils::safe_truncate_bytes(s, 200)));

                // Thinking-onlyレスポンス検出: 本文が空でthinkingのみの場合はエラーを返す
                if !tool_buffer.has_tool_calls() && full_text.trim().is_empty() && thinking.is_some() {
                    return Err(AppError::LlmApi(
                        "LLMの応答に本文が含まれていません（思考のみ）。モデルの最大トークン数（max_tokens）を増やすか、コンテキスト長の設定を見直してください。".to_string()
                    ));
                }

                if tool_buffer.has_tool_calls() {
                    Ok(LLMResponse::ToolCalls { calls: tool_buffer.into_tool_calls(), thinking })
                } else {
                    Ok(LLMResponse::Text { content: full_text, thinking })
                }
            }
        }
    }

    async fn test_connection(&self, config: &LLMClientConfig) -> Result<(), AppError> {
        let strategy = resolve_api_strategy(config);

        match strategy {
            ApiStrategy::Gemini => {
                let url = build_gemini_url(config);
                let api_key = config.api_key.as_deref().unwrap_or("");
                let url_with_key = format!("{}?key={}", url, api_key);

                // 最小限のgenerateContentリクエスト
                let body = serde_json::json!({
                    "contents": [{"role": "user", "parts": [{"text": "Hi"}]}],
                    "generationConfig": {
                        "temperature": config.temperature,
                        "maxOutputTokens": 1,
                    }
                });

                let response = self
                    .http_client
                    .post(&url_with_key)
                    .json(&body)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status = response.status();
                    let error_body = response.text().await.unwrap_or_default();
                    return Err(AppError::LlmApi(format!(
                        "Gemini connection test failed (status {}): {}",
                        status, error_body
                    )));
                }

                let _: Value = response.json().await.map_err(|e| {
                    AppError::LlmApi(format!(
                        "Gemini connection test: invalid response format: {}",
                        e
                    ))
                })?;

                Ok(())
            }
            ApiStrategy::Anthropic => {
                let url = build_anthropic_url(config);
                let api_key = config.api_key.as_deref().unwrap_or("");

                // 最小限のmessagesリクエスト
                let body = serde_json::json!({
                    "model": config.model,
                    "messages": [{"role": "user", "content": "Hi"}],
                    "temperature": config.temperature,
                    "max_tokens": 1,
                });

                let response = self
                    .http_client
                    .post(&url)
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .json(&body)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status = response.status();
                    let error_body = response.text().await.unwrap_or_default();
                    return Err(AppError::LlmApi(format!(
                        "Anthropic connection test failed (status {}): {}",
                        status, error_body
                    )));
                }

                let _: Value = response.json().await.map_err(|e| {
                    AppError::LlmApi(format!(
                        "Anthropic connection test: invalid response format: {}",
                        e
                    ))
                })?;

                Ok(())
            }
            ApiStrategy::OpenAI => {
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

                let _: Value = response.json().await.map_err(|e| {
                    AppError::LlmApi(format!("Connection test: invalid response format: {}", e))
                })?;

                Ok(())
            }
        }
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
                images: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_call_id: None,
                images: None,
            },
        ];
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: None,
        };

        let body = client.build_request_body(&messages, &config, None, false);

        assert_eq!(body["model"], "gpt-4");
        let temp = body["temperature"].as_f64().unwrap();
        assert!(
            (temp - 0.7f64).abs() < 1e-5,
            "temperature mismatch: {}",
            temp
        );
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
            images: None,
        }];
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: Some("sk-test".to_string()),
            temperature: 0.5,
            provider: None,
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
            images: None,
        }];
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: None,
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
            images: None,
        }];
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: None,
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
            provider: None,
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
            provider: None,
        };
        assert_eq!(
            OpenAICompatibleClient::build_url(&config2),
            "http://localhost:8080/v1/chat/completions"
        );
    }

    #[test]
    fn test_build_url_empty_base_url_google() {
        let config = LLMClientConfig {
            base_url: "".to_string(),
            model: "gemini-2.0-flash".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: Some(LLMProvider::Google),
        };
        assert_eq!(
            OpenAICompatibleClient::build_url(&config),
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        );
    }

    #[test]
    fn test_build_url_empty_base_url_openai() {
        let config = LLMClientConfig {
            base_url: "".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: Some(LLMProvider::Openai),
        };
        assert_eq!(
            OpenAICompatibleClient::build_url(&config),
            "https://api.openai.com/v1/chat/completions"
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
            LLMResponse::Text { content: text, .. } => assert_eq!(text, "Hello! How can I help you?"),
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
            LLMResponse::ToolCalls { calls, .. } => {
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
            images: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"system\""));

        let msg = ChatMessage {
            role: MessageRole::Assistant,
            content: "test".to_string(),
            tool_call_id: None,
            images: None,
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
            images: None,
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
            images: None,
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
            provider: None,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["base_url"], "http://localhost:8080/v1");
        assert_eq!(json["model"], "gpt-4");
        assert_eq!(json["api_key"], "sk-test");
        let temp = json["temperature"].as_f64().unwrap();
        assert!(
            (temp - 0.7f64).abs() < 1e-5,
            "temperature mismatch: {}",
            temp
        );
    }

    #[test]
    fn test_llm_response_text_serialization() {
        let resp = LLMResponse::Text { content: "Hello".to_string(), thinking: None };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("Text"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_llm_response_tool_calls_serialization() {
        let resp = LLMResponse::ToolCalls { calls: vec![ToolCall {
            id: "call_1".to_string(),
            name: "search".to_string(),
            arguments: serde_json::json!({"query": "test"}),
            context: None,
        }], thinking: None };
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
            images: None,
        }];
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: None,
        };
        let tools: Vec<ToolDefinition> = vec![];

        // 空のtools配列を渡した場合、toolsフィールドは含まれない
        let body = client.build_request_body(&messages, &config, Some(&tools), false);
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn test_build_gemini_request_basic() {
        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: "You are a helpful assistant.".to_string(),
                tool_call_id: None,
                images: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_call_id: None,
                images: None,
            },
            ChatMessage {
                role: MessageRole::Assistant,
                content: "Hi there!".to_string(),
                tool_call_id: None,
                images: None,
            },
        ];
        let config = LLMClientConfig {
            base_url: "".to_string(),
            model: "gemini-pro".to_string(),
            api_key: Some("test-key".to_string()),
            temperature: 0.9,
            provider: Some(LLMProvider::Google),
        };

        let body = build_gemini_request(&messages, &config, None);

        // contentsにはSystemメッセージが含まれない
        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "Hello");
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(contents[1]["parts"][0]["text"], "Hi there!");

        // systemInstructionが設定される
        assert_eq!(
            body["systemInstruction"]["parts"][0]["text"],
            "You are a helpful assistant."
        );

        // generationConfig.temperature
        let temp = body["generationConfig"]["temperature"].as_f64().unwrap();
        assert!((temp - 0.9f64).abs() < 1e-5);
    }

    #[test]
    fn test_build_gemini_request_no_system_message() {
        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
            tool_call_id: None,
            images: None,
        }];
        let config = LLMClientConfig {
            base_url: "".to_string(),
            model: "gemini-pro".to_string(),
            api_key: None,
            temperature: 0.5,
            provider: Some(LLMProvider::Google),
        };

        let body = build_gemini_request(&messages, &config, None);

        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");

        // systemInstructionは存在しない
        assert!(body.get("systemInstruction").is_none());
    }

    #[test]
    fn test_build_gemini_url_default_endpoint() {
        let config = LLMClientConfig {
            base_url: "".to_string(),
            model: "gemini-1.5-pro".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: Some(LLMProvider::Google),
        };

        let url = build_gemini_url(&config);
        assert_eq!(
            url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-pro:generateContent"
        );
    }

    #[test]
    fn test_build_gemini_url_custom_endpoint() {
        let config = LLMClientConfig {
            base_url: "https://custom-proxy.example.com/v1".to_string(),
            model: "gemini-pro".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: Some(LLMProvider::Google),
        };

        let url = build_gemini_url(&config);
        assert_eq!(
            url,
            "https://custom-proxy.example.com/v1/models/gemini-pro:generateContent"
        );
    }

    #[test]
    fn test_build_gemini_url_trailing_slash() {
        let config = LLMClientConfig {
            base_url: "https://custom-proxy.example.com/v1/".to_string(),
            model: "gemini-pro".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: Some(LLMProvider::Google),
        };

        let url = build_gemini_url(&config);
        assert_eq!(
            url,
            "https://custom-proxy.example.com/v1/models/gemini-pro:generateContent"
        );
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
            LLMResponse::ToolCalls { calls, .. } => {
                assert_eq!(calls.len(), 2);
                assert_eq!(calls[0].name, "search");
                assert_eq!(calls[1].name, "calculator");
            }
            _ => panic!("Expected LLMResponse::ToolCalls"),
        }
    }

    #[test]
    fn test_build_anthropic_request_basic() {
        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: "You are a helpful assistant.".to_string(),
                tool_call_id: None,
                images: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_call_id: None,
                images: None,
            },
            ChatMessage {
                role: MessageRole::Assistant,
                content: "Hi there!".to_string(),
                tool_call_id: None,
                images: None,
            },
        ];
        let config = LLMClientConfig {
            base_url: "".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            api_key: Some("sk-ant-test".to_string()),
            temperature: 0.7,
            provider: Some(LLMProvider::Anthropic),
        };

        let body = build_anthropic_request(&messages, &config, None);

        // modelが設定される
        assert_eq!(body["model"], "claude-3-5-sonnet-20241022");

        // messagesにはSystemメッセージが含まれない
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "Hello");
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[1]["content"], "Hi there!");

        // systemフィールドにシステムメッセージが設定される
        assert_eq!(body["system"], "You are a helpful assistant.");

        // temperature
        let temp = body["temperature"].as_f64().unwrap();
        assert!((temp - 0.7f64).abs() < 1e-5);

        // max_tokens
        assert_eq!(body["max_tokens"], 4096);
    }

    #[test]
    fn test_build_anthropic_request_no_system_message() {
        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
            tool_call_id: None,
            images: None,
        }];
        let config = LLMClientConfig {
            base_url: "".to_string(),
            model: "claude-3-haiku-20240307".to_string(),
            api_key: None,
            temperature: 0.5,
            provider: Some(LLMProvider::Anthropic),
        };

        let body = build_anthropic_request(&messages, &config, None);

        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");

        // systemフィールドは存在しない
        assert!(body.get("system").is_none());
    }

    #[test]
    fn test_build_anthropic_request_multiple_system_messages() {
        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: "First instruction.".to_string(),
                tool_call_id: None,
                images: None,
            },
            ChatMessage {
                role: MessageRole::System,
                content: "Second instruction.".to_string(),
                tool_call_id: None,
                images: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_call_id: None,
                images: None,
            },
        ];
        let config = LLMClientConfig {
            base_url: "".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: Some(LLMProvider::Anthropic),
        };

        let body = build_anthropic_request(&messages, &config, None);

        // 複数のシステムメッセージが改行で結合される
        assert_eq!(body["system"], "First instruction.\nSecond instruction.");

        // messagesにはuser/assistantのみ
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
    }

    #[test]
    fn test_build_anthropic_url_default_endpoint() {
        let config = LLMClientConfig {
            base_url: "".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: Some(LLMProvider::Anthropic),
        };

        let url = build_anthropic_url(&config);
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn test_build_anthropic_url_custom_endpoint() {
        let config = LLMClientConfig {
            base_url: "https://custom-proxy.example.com/v1".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: Some(LLMProvider::Anthropic),
        };

        let url = build_anthropic_url(&config);
        assert_eq!(url, "https://custom-proxy.example.com/v1/messages");
    }

    #[test]
    fn test_build_anthropic_url_trailing_slash() {
        let config = LLMClientConfig {
            base_url: "https://custom-proxy.example.com/v1/".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: Some(LLMProvider::Anthropic),
        };

        let url = build_anthropic_url(&config);
        assert_eq!(url, "https://custom-proxy.example.com/v1/messages");
    }

    // --- parse_gemini_response tests ---

    #[test]
    fn test_parse_gemini_response_basic() {
        let body = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello from Gemini!"}],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        });

        let result = parse_gemini_response(&body).unwrap();
        match result {
            LLMResponse::Text { content: text, .. } => assert_eq!(text, "Hello from Gemini!"),
            _ => panic!("Expected LLMResponse::Text"),
        }
    }

    #[test]
    fn test_parse_gemini_response_empty_candidates() {
        let body = serde_json::json!({
            "candidates": []
        });

        let result = parse_gemini_response(&body);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("no candidates"));
    }

    #[test]
    fn test_parse_gemini_response_missing_candidates() {
        let body = serde_json::json!({
            "promptFeedback": {
                "blockReason": "SAFETY"
            }
        });

        let result = parse_gemini_response(&body);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_gemini_response_empty_text_with_candidate() {
        // candidatesは存在するがtextが空の場合 → 空テキストを返す（エラーではない）
        let body = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": ""}],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        });

        let result = parse_gemini_response(&body).unwrap();
        match result {
            LLMResponse::Text { content: text, .. } => assert_eq!(text, ""),
            _ => panic!("Expected LLMResponse::Text"),
        }
    }

    // --- parse_anthropic_response tests ---

    #[test]
    fn test_parse_anthropic_response_basic() {
        let body = serde_json::json!({
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Hello from Claude!"}
            ],
            "stop_reason": "end_turn"
        });

        let result = parse_anthropic_response(&body).unwrap();
        match result {
            LLMResponse::Text { content: text, .. } => assert_eq!(text, "Hello from Claude!"),
            _ => panic!("Expected LLMResponse::Text"),
        }
    }

    #[test]
    fn test_parse_anthropic_response_multiple_text_blocks() {
        let body = serde_json::json!({
            "id": "msg_456",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "First part. "},
                {"type": "text", "text": "Second part."}
            ],
            "stop_reason": "end_turn"
        });

        let result = parse_anthropic_response(&body).unwrap();
        match result {
            LLMResponse::Text { content: text, .. } => assert_eq!(text, "First part. Second part."),
            _ => panic!("Expected LLMResponse::Text"),
        }
    }

    #[test]
    fn test_parse_anthropic_response_missing_content() {
        let body = serde_json::json!({
            "id": "msg_789",
            "type": "error",
            "error": {"type": "invalid_request_error", "message": "bad request"}
        });

        let result = parse_anthropic_response(&body);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("missing 'content' array"));
    }

    #[test]
    fn test_parse_anthropic_response_empty_content() {
        let body = serde_json::json!({
            "id": "msg_000",
            "type": "message",
            "role": "assistant",
            "content": [],
            "stop_reason": "end_turn"
        });

        let result = parse_anthropic_response(&body).unwrap();
        match result {
            LLMResponse::Text { content: text, .. } => assert_eq!(text, ""),
            _ => panic!("Expected LLMResponse::Text"),
        }
    }

    #[test]
    fn test_parse_anthropic_response_mixed_content_types() {
        // tool_useブロックが混在する場合、textブロックのみ抽出
        let body = serde_json::json!({
            "id": "msg_mixed",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Let me search for that. "},
                {"type": "tool_use", "id": "toolu_123", "name": "search", "input": {"query": "test"}},
                {"type": "text", "text": "Here are the results."}
            ],
            "stop_reason": "tool_use"
        });

        let result = parse_anthropic_response(&body).unwrap();
        match result {
            LLMResponse::Text { content: text, .. } => {
                assert_eq!(text, "Let me search for that. Here are the results.")
            }
            _ => panic!("Expected LLMResponse::Text"),
        }
    }

    // --- build_gemini_stream_url tests ---

    #[test]
    fn test_build_gemini_stream_url_default_endpoint() {
        let config = LLMClientConfig {
            base_url: "".to_string(),
            model: "gemini-1.5-pro".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: Some(LLMProvider::Google),
        };

        let url = build_gemini_stream_url(&config);
        assert_eq!(
            url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-pro:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn test_build_gemini_stream_url_custom_endpoint() {
        let config = LLMClientConfig {
            base_url: "https://custom-proxy.example.com/v1".to_string(),
            model: "gemini-pro".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: Some(LLMProvider::Google),
        };

        let url = build_gemini_stream_url(&config);
        assert_eq!(
            url,
            "https://custom-proxy.example.com/v1/models/gemini-pro:streamGenerateContent?alt=sse"
        );
    }

    // --- parse_gemini_sse_line tests ---

    #[test]
    fn test_parse_gemini_sse_line_content() {
        let line =
            r#"data: {"candidates":[{"content":{"parts":[{"text":"Hello"}],"role":"model"}}]}"#;
        let result = OpenAICompatibleClient::parse_gemini_sse_line(line);
        assert_eq!(result, Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_gemini_sse_line_no_data_prefix() {
        let line = "event: message";
        let result = OpenAICompatibleClient::parse_gemini_sse_line(line);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_gemini_sse_line_empty_text() {
        let line = r#"data: {"candidates":[{"content":{"parts":[{"text":""}],"role":"model"}}]}"#;
        let result = OpenAICompatibleClient::parse_gemini_sse_line(line);
        assert_eq!(result, Some("".to_string()));
    }

    // --- parse_anthropic_sse_line tests ---

    #[test]
    fn test_parse_anthropic_sse_line_content_block_delta() {
        let line = r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let result = OpenAICompatibleClient::parse_anthropic_sse_line(line);
        assert_eq!(result, Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_anthropic_sse_line_message_start() {
        let line = r#"data: {"type":"message_start","message":{"id":"msg_123","type":"message","role":"assistant"}}"#;
        let result = OpenAICompatibleClient::parse_anthropic_sse_line(line);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_anthropic_sse_line_message_stop() {
        let line = r#"data: {"type":"message_stop"}"#;
        let result = OpenAICompatibleClient::parse_anthropic_sse_line(line);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_anthropic_sse_line_no_data_prefix() {
        let line = "event: content_block_delta";
        let result = OpenAICompatibleClient::parse_anthropic_sse_line(line);
        assert_eq!(result, None);
    }

    // --- strip_think_tags tests ---

    #[test]
    fn test_strip_think_tags_basic() {
        let input = "<think>I need to think about this</think>Hello!";
        assert_eq!(OpenAICompatibleClient::strip_think_tags(input), "Hello!");
    }

    #[test]
    fn test_strip_think_tags_multiple() {
        let input = "A<think>thought1</think>B<think>thought2</think>C";
        assert_eq!(OpenAICompatibleClient::strip_think_tags(input), "ABC");
    }

    #[test]
    fn test_strip_think_tags_multiline() {
        let input = "<think>\nline1\nline2\n</think>Result";
        assert_eq!(OpenAICompatibleClient::strip_think_tags(input), "Result");
    }

    #[test]
    fn test_strip_think_tags_unclosed() {
        let input = "Before<think>still thinking...";
        assert_eq!(OpenAICompatibleClient::strip_think_tags(input), "Before");
    }

    #[test]
    fn test_strip_think_tags_close_only() {
        // 閉じタグだけ先頭に残っているケース（前レスポンスから跨いだ）
        let input = "thinking content</think>Actual response";
        assert_eq!(
            OpenAICompatibleClient::strip_think_tags(input),
            "Actual response"
        );
    }

    #[test]
    fn test_strip_think_tags_no_tags() {
        let input = "Normal text without any tags";
        assert_eq!(
            OpenAICompatibleClient::strip_think_tags(input),
            "Normal text without any tags"
        );
    }

    #[test]
    fn test_strip_think_tags_empty() {
        assert_eq!(OpenAICompatibleClient::strip_think_tags(""), "");
    }

    #[test]
    fn test_strip_think_tags_only_thinking() {
        let input = "<think>All thinking no output</think>";
        assert_eq!(OpenAICompatibleClient::strip_think_tags(input), "");
    }

    // --- parse_response with reasoning_content tests ---

    #[test]
    fn test_parse_response_with_reasoning_content() {
        // OpenAI o3/o4-mini 形式: reasoning_content フィールドを無視して content のみ抽出
        let response = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "The answer is 42.",
                    "reasoning_content": "Let me think step by step..."
                }
            }]
        });

        let result = OpenAICompatibleClient::parse_response(&response).unwrap();
        match result {
            LLMResponse::Text { content: text, .. } => assert_eq!(text, "The answer is 42."),
            _ => panic!("Expected LLMResponse::Text"),
        }
    }

    #[test]
    fn test_parse_response_content_with_think_tags_and_reasoning() {
        // LM Studio: content に <think> タグ混入 + reasoning_content も存在するケース
        let response = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "<think>internal monologue</think>Final answer.",
                    "reasoning_content": "step by step"
                }
            }]
        });

        let result = OpenAICompatibleClient::parse_response(&response).unwrap();
        match result {
            LLMResponse::Text { content: text, .. } => assert_eq!(text, "Final answer."),
            _ => panic!("Expected LLMResponse::Text"),
        }
    }

    // --- Gemini thinking part filter tests ---

    #[test]
    fn test_parse_gemini_response_filters_thinking_parts() {
        let body = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"text": "Let me think...", "thought": true},
                        {"text": "The answer is 42."}
                    ],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        });

        let result = parse_gemini_response(&body).unwrap();
        match result {
            LLMResponse::Text { content: text, .. } => assert_eq!(text, "The answer is 42."),
            _ => panic!("Expected LLMResponse::Text"),
        }
    }

    // --- Anthropic thinking block filter tests ---

    #[test]
    fn test_parse_anthropic_response_filters_thinking_blocks() {
        let body = serde_json::json!({
            "id": "msg_think",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": "Let me reason about this..."},
                {"type": "text", "text": "Here is my answer."}
            ],
            "stop_reason": "end_turn"
        });

        let result = parse_anthropic_response(&body).unwrap();
        match result {
            LLMResponse::Text { content: text, .. } => assert_eq!(text, "Here is my answer."),
            _ => panic!("Expected LLMResponse::Text"),
        }
    }

    #[test]
    fn test_parse_anthropic_response_filters_redacted_thinking() {
        let body = serde_json::json!({
            "id": "msg_redacted",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "redacted_thinking", "data": "encrypted_data_here"},
                {"type": "text", "text": "My response."}
            ],
            "stop_reason": "end_turn"
        });

        let result = parse_anthropic_response(&body).unwrap();
        match result {
            LLMResponse::Text { content: text, .. } => assert_eq!(text, "My response."),
            _ => panic!("Expected LLMResponse::Text"),
        }
    }
}
