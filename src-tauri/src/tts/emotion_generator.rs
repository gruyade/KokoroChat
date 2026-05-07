// Emotion Generator - LLMによるVoicePeak感情パラメータ自動生成

use std::collections::HashMap;

use crate::error::AppError;
use crate::llm::client::{ChatMessage, LLMClientConfig, LLMResponse, MessageRole};
use crate::llm::client::LLMClient;
use crate::models::tts::{EmotionParams, TTSConfig};

/// LLMで感情パラメータを生成
pub struct EmotionGenerator;

/// 生成された感情パラメータ
#[derive(Debug, Clone)]
pub struct GeneratedEmotionParams {
    pub emotion: EmotionParams,
    pub speed: Option<f32>,
    pub pitch: Option<f32>,
}

impl EmotionGenerator {
    /// テキストから感情パラメータを生成（単一LLM呼び出し）
    pub async fn generate(
        &self,
        text: &str,
        base_config: &TTSConfig,
        llm_client: &dyn LLMClient,
        llm_config: &LLMClientConfig,
    ) -> Result<GeneratedEmotionParams, AppError> {
        let system_prompt = Self::build_system_prompt(base_config);
        let user_prompt = format!("以下のテキストの感情を分析してJSON形式で返してください:\n\n{}", text);

        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: system_prompt,
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: user_prompt,
                tool_call_id: None,
            },
        ];

        let response = llm_client.chat(&messages, llm_config, None).await?;

        match response {
            LLMResponse::Text(json_str) => Self::parse_and_validate(&json_str, base_config),
            LLMResponse::ToolCalls(_) => Err(AppError::LlmApi(
                "Unexpected tool call response from emotion generation".to_string(),
            )),
        }
    }

    /// LLMレスポンスJSONをパースしてバリデーション
    /// 範囲外の値はクランプする:
    /// - emotion (動的キー): 0–キャラクター設定の上限値
    /// - speed: 50–200
    /// - pitch: -300–300
    pub fn parse_and_validate(
        json_str: &str,
        base_config: &TTSConfig,
    ) -> Result<GeneratedEmotionParams, AppError> {
        // JSONブロック内のテキストを抽出（```json ... ``` 形式対応）
        let cleaned = Self::extract_json(json_str);

        let raw: HashMap<String, serde_json::Value> = serde_json::from_str(&cleaned).map_err(|e| {
            AppError::Tts(format!("Failed to parse emotion JSON: {} (input: {})", e, cleaned))
        })?;

        let mut emotion = EmotionParams::new();
        let mut speed: Option<f32> = None;
        let mut pitch: Option<f32> = None;

        for (key, value) in &raw {
            if let Some(num) = value.as_f64() {
                match key.as_str() {
                    "speed" => speed = Some(clamp_f32(num as f32, 50.0, 200.0)),
                    "pitch" => pitch = Some(clamp_f32(num as f32, -300.0, 300.0)),
                    _ => {
                        // 感情パラメータ: 0〜キャラクター設定の上限値にクランプ
                        let max_value = base_config.emotion.as_ref()
                            .and_then(|e| e.get(key).copied())
                            .unwrap_or(100);
                        emotion.insert(key.clone(), clamp_i32(num as i32, 0, max_value));
                    }
                }
            }
        }

        Ok(GeneratedEmotionParams {
            emotion,
            speed,
            pitch,
        })
    }

    /// システムプロンプトを構築
    fn build_system_prompt(base_config: &TTSConfig) -> String {
        let emotion_info = if let Some(ref emotion) = base_config.emotion {
            if emotion.is_empty() {
                "利用可能な感情キー: happy, fun, angry, sad\n各感情キーの値は0-100の整数です。".to_string()
            } else {
                let keys_with_max: Vec<String> = emotion.iter()
                    .map(|(k, v)| format!("  - {}: 0〜{}", k, v))
                    .collect();
                format!("利用可能な感情キーと上限値:\n{}\n各値は0から上限値までの整数で設定してください。上限値を超えないこと。", keys_with_max.join("\n"))
            }
        } else {
            "利用可能な感情キー: happy, fun, angry, sad\n各感情キーの値は0-100の整数です。".to_string()
        };

        format!(
            r#"あなたはテキストの感情分析を行い、音声合成パラメータを生成するアシスタントです。

与えられたテキストを分析し、以下のJSON形式で感情パラメータを返してください。
JSON以外のテキストは含めないでください。

{}

追加パラメータ:
- speed: 読み上げ速度（100=標準, 50=最遅, 200=最速）
- pitch: ピッチ（0=標準, -300=最低, 300=最高）

テキストの内容に合わせて適切な値を設定してください。"#,
            emotion_info
        )
    }

    /// JSON文字列を抽出（マークダウンコードブロック対応）
    fn extract_json(input: &str) -> String {
        let trimmed = input.trim();

        // ```json ... ``` 形式
        if let Some(start) = trimmed.find("```json") {
            let after_marker = &trimmed[start + 7..];
            if let Some(end) = after_marker.find("```") {
                return after_marker[..end].trim().to_string();
            }
        }

        // ``` ... ``` 形式
        if let Some(start) = trimmed.find("```") {
            let after_marker = &trimmed[start + 3..];
            if let Some(end) = after_marker.find("```") {
                return after_marker[..end].trim().to_string();
            }
        }

        // { ... } を探す
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                if end > start {
                    return trimmed[start..=end].to_string();
                }
            }
        }

        trimmed.to_string()
    }
}

/// i32値を指定範囲にクランプ
fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    value.max(min).min(max)
}

/// f32値を指定範囲にクランプ
fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    value.max(min).min(max)
}
