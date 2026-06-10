// Caption Generator - LLMによるIrodori-TTS喋り方キャプション自動生成

use crate::error::AppError;
use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};

/// LLMで喋り方キャプションを生成
pub struct CaptionGenerator;

impl CaptionGenerator {
    /// テキストから喋り方キャプションを生成（単一LLM呼び出し）
    pub async fn generate(
        &self,
        text: &str,
        base_caption: &str,
        llm_client: &dyn LLMClient,
        llm_config: &LLMClientConfig,
    ) -> Result<String, AppError> {
        let system_prompt = Self::build_system_prompt(base_caption);
        let user_prompt = format!(
            "以下のテキストに適した喋り方キャプションを生成してください:\n\n{}",
            text
        );

        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: system_prompt,
                tool_call_id: None,
                images: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: user_prompt,
                tool_call_id: None,
                images: None,
            },
        ];

        let response = llm_client.chat(&messages, llm_config, None).await?;

        match response {
            LLMResponse::Text {
                content: caption, ..
            } => Ok(caption.trim().to_string()),
            LLMResponse::ToolCalls { calls: _, .. } => Err(AppError::LlmApi(
                "Unexpected tool call response from caption generation".to_string(),
            )),
        }
    }

    /// ベースキャプション + 動的キャプションを結合
    pub fn combine_captions(base_caption: &str, dynamic_caption: &str) -> String {
        format!("{} {}", base_caption, dynamic_caption)
    }

    /// システムプロンプトを構築
    fn build_system_prompt(base_caption: &str) -> String {
        format!(
            r#"あなたはテキストの内容を分析し、音声合成用の喋り方キャプションを生成するアシスタントです。

キャラクターの基本的な声の特徴: {}

与えられたテキストの内容・感情・文脈を分析し、そのテキストを読み上げる際の喋り方を短いキャプションとして生成してください。

ルール:
- キャプションのみを返してください（説明や装飾は不要）
- 日本語で、簡潔に喋り方を表現してください
- 例: 「優しく語りかけるように」「元気よく明るく」「少し悲しげに、ゆっくりと」「驚きを込めて」

キャプションのみを出力してください。"#,
            base_caption
        )
    }
}
