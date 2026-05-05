//! チャットコンテキスト組み立てのプロパティテスト
//! proptest を使用して build_context の不変条件を検証する。
//!
//! **Validates: Requirements 2.2, 5.3**

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use proptest::prelude::*;

    use crate::chat::engine::DefaultChatEngine;
    use crate::db::database::Database;
    use crate::error::AppError;
    use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
    use crate::models::{ChatMessageRecord, ChatRole, Memory, ToolDefinition};

    use async_trait::async_trait;

    // ========================================
    // テスト用MockLLMClient
    // ========================================

    struct MockLLMClient;

    #[async_trait]
    impl LLMClient for MockLLMClient {
        async fn chat(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<LLMResponse, AppError> {
            Ok(LLMResponse::Text("mock".to_string()))
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _callback: Box<dyn Fn(String) + Send>,
        ) -> Result<String, AppError> {
            Ok("mock".to_string())
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    // ========================================
    // ストラテジー
    // ========================================

    /// 非空文字列を生成するストラテジー
    fn non_empty_string() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9ぁ-んァ-ヶ ,.!?]{1,50}".prop_map(|s| s)
    }

    /// UUID文字列を生成するストラテジー
    fn uuid_string() -> impl Strategy<Value = String> {
        "[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}"
    }

    /// ISO 8601日時文字列を生成するストラテジー
    fn iso8601_datetime() -> impl Strategy<Value = String> {
        (2020u32..2030, 1u32..13, 1u32..29, 0u32..24, 0u32..60, 0u32..60).prop_map(
            |(y, m, d, h, min, s)| {
                format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, h, min, s)
            },
        )
    }

    /// Memory のストラテジー（0〜5件）
    fn arb_memories(max_count: usize) -> impl Strategy<Value = Vec<Memory>> {
        proptest::collection::vec(
            (uuid_string(), uuid_string(), non_empty_string(), iso8601_datetime(), iso8601_datetime())
                .prop_map(|(id, char_id, content, created_at, updated_at)| Memory {
                    id,
                    character_id: char_id,
                    content,
                    source_session_id: None,
                    source_message_from: None,
                    source_message_to: None,
                    created_at,
                    updated_at,
                }),
            0..=max_count,
        )
    }

    /// ChatRole（user/assistant交互）のストラテジー
    fn alternating_role(index: usize) -> ChatRole {
        if index % 2 == 0 {
            ChatRole::User
        } else {
            ChatRole::Assistant
        }
    }

    /// チャット履歴のストラテジー（0〜10件、user/assistant交互）
    fn arb_chat_history(max_count: usize) -> impl Strategy<Value = Vec<ChatMessageRecord>> {
        proptest::collection::vec(
            (uuid_string(), uuid_string(), non_empty_string(), iso8601_datetime()),
            0..=max_count,
        )
        .prop_map(|items| {
            items
                .into_iter()
                .enumerate()
                .map(|(i, (id, session_id, content, created_at))| ChatMessageRecord {
                    id,
                    session_id,
                    role: alternating_role(i),
                    content,
                    attachments: None,
                    tool_calls: None,
                    tool_call_id: None,
                    created_at,
                })
                .collect()
        })
    }

    /// DefaultChatEngine インスタンスを作成（build_contextテスト用、DB不要だがコンストラクタに必要）
    fn create_engine() -> DefaultChatEngine {
        let db = Database::open_in_memory().unwrap();
        let db = Arc::new(Mutex::new(db));
        let llm_client: Arc<dyn LLMClient> = Arc::new(MockLLMClient);
        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            temperature: 0.7,
        };
        DefaultChatEngine::new(db, llm_client, config)
    }

    // ========================================
    // Property 4: Chat context assembly includes system prompt, history, and memories
    // ========================================
    //
    // **Validates: Requirements 2.2, 5.3**
    //
    // For any chat message sent in a session belonging to a Character with existing Memories,
    // the LLM request SHALL contain:
    // 1. The Character's systemPrompt as the first message (role=system)
    // 2. All relevant Memories in the prompt (as system messages with [Memory] prefix)
    // 3. The full chat history in chronological order
    // 4. The user's new message as the last message

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_context_assembly_structure(
            system_prompt in non_empty_string(),
            memories in arb_memories(5),
            history in arb_chat_history(10),
            user_message in non_empty_string(),
        ) {
            let engine = create_engine();

            let result = engine.build_context(
                &system_prompt,
                &memories,
                &history,
                &user_message,
                None,
            );

            let num_memories = memories.len();
            let num_history = history.len();
            let expected_total = 1 + num_memories + num_history + 1;

            // 総メッセージ数 = 1 (system) + num_memories + num_history + 1 (user)
            prop_assert_eq!(
                result.len(),
                expected_total,
                "Total message count mismatch: expected {}, got {}",
                expected_total,
                result.len()
            );

            // 1. 最初のメッセージはシステムプロンプト（role=system）
            prop_assert_eq!(
                result[0].role,
                MessageRole::System,
                "First message should be System role"
            );
            prop_assert_eq!(
                result[0].content,
                system_prompt,
                "First message content should be the system prompt"
            );

            // 2. メモリメッセージ（[Memory]プレフィックス付きsystemメッセージ）
            for i in 0..num_memories {
                let msg_idx = 1 + i;
                prop_assert_eq!(
                    result[msg_idx].role,
                    MessageRole::System,
                    "Memory message at index {} should be System role",
                    msg_idx
                );
                prop_assert!(
                    result[msg_idx].content.starts_with("[Memory]"),
                    "Memory message at index {} should start with [Memory] prefix, got: {}",
                    msg_idx,
                    &result[msg_idx].content
                );
                // メモリの内容が含まれている
                prop_assert!(
                    result[msg_idx].content.contains(&memories[i].content),
                    "Memory message at index {} should contain memory content",
                    msg_idx
                );
            }

            // 3. 履歴メッセージが順序通り
            for i in 0..num_history {
                let msg_idx = 1 + num_memories + i;
                prop_assert_eq!(
                    result[msg_idx].content,
                    history[i].content,
                    "History message at index {} content mismatch",
                    msg_idx
                );
            }

            // 4. 最後のメッセージはユーザーの新規メッセージ（role=user）
            let last_idx = result.len() - 1;
            prop_assert_eq!(
                result[last_idx].role,
                MessageRole::User,
                "Last message should be User role"
            );
            prop_assert_eq!(
                result[last_idx].content,
                user_message,
                "Last message content should be the user's new message"
            );
        }

        #[test]
        fn prop_context_assembly_with_attachment(
            system_prompt in non_empty_string(),
            memories in arb_memories(3),
            history in arb_chat_history(5),
            user_message in non_empty_string(),
            attachment_text in non_empty_string(),
        ) {
            let engine = create_engine();

            let result = engine.build_context(
                &system_prompt,
                &memories,
                &history,
                &user_message,
                Some(&attachment_text),
            );

            let num_memories = memories.len();
            let num_history = history.len();
            let expected_total = 1 + num_memories + num_history + 1;

            // 添付テキストがあっても総メッセージ数は変わらない（ユーザーメッセージに結合）
            prop_assert_eq!(
                result.len(),
                expected_total,
                "Total message count should not change with attachment"
            );

            // 最後のメッセージにユーザーメッセージと添付テキストの両方が含まれる
            let last_idx = result.len() - 1;
            prop_assert_eq!(result[last_idx].role, MessageRole::User);
            prop_assert!(
                result[last_idx].content.contains(&user_message),
                "Last message should contain user message"
            );
            prop_assert!(
                result[last_idx].content.contains(&attachment_text),
                "Last message should contain attachment text"
            );
            prop_assert!(
                result[last_idx].content.contains("[Attached Files]"),
                "Last message should contain [Attached Files] marker"
            );
        }

        #[test]
        fn prop_context_history_role_mapping(
            system_prompt in non_empty_string(),
            history in arb_chat_history(10),
            user_message in non_empty_string(),
        ) {
            let engine = create_engine();

            let result = engine.build_context(
                &system_prompt,
                &[],
                &history,
                &user_message,
                None,
            );

            // 履歴メッセージのロールマッピングが正しい
            for i in 0..history.len() {
                let msg_idx = 1 + i; // system_prompt の次から
                let expected_role = match history[i].role {
                    ChatRole::User => MessageRole::User,
                    ChatRole::Assistant => MessageRole::Assistant,
                    ChatRole::Spontaneous => MessageRole::Assistant,
                    ChatRole::Tool => MessageRole::Tool,
                };
                prop_assert_eq!(
                    result[msg_idx].role,
                    expected_role,
                    "History message at index {} role mapping incorrect: {:?} -> expected {:?}",
                    i,
                    history[i].role,
                    expected_role
                );
            }
        }
    }
}
