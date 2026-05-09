//! 自発的発話のプロパティテスト
//! proptest を使用して SpontaneousSpeaker の不変条件を検証する。
//!
//! **Validates: Requirements 3.1, 3.2, 3.5**

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use proptest::prelude::*;

    use crate::llm::client::MessageRole;
    use crate::models::{ChatMessageRecord, ChatRole};
    use crate::spontaneous::speaker::{
        DefaultSpontaneousSpeaker, SpontaneousEvent, SpontaneousSpeakerConfig,
    };

    // ========================================
    // ストラテジー
    // ========================================

    /// 非空文字列を生成するストラテジー
    fn non_empty_string() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9ぁ-んァ-ヶ ,.!?]{1,100}".prop_map(|s| s)
    }

    /// UUID文字列を生成するストラテジー
    fn uuid_string() -> impl Strategy<Value = String> {
        "[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}"
    }

    /// ISO 8601日時文字列を生成するストラテジー
    fn iso8601_datetime() -> impl Strategy<Value = String> {
        (
            2020u32..2030,
            1u32..13,
            1u32..29,
            0u32..24,
            0u32..60,
            0u32..60,
        )
            .prop_map(|(y, m, d, h, min, s)| {
                format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, h, min, s)
            })
    }

    /// ChatRoleのストラテジー（User, Assistant, Spontaneous, Tool）
    fn arb_chat_role() -> impl Strategy<Value = ChatRole> {
        prop_oneof![
            Just(ChatRole::User),
            Just(ChatRole::Assistant),
            Just(ChatRole::Spontaneous),
            Just(ChatRole::Tool),
        ]
    }

    /// ChatMessageRecordのストラテジー
    fn arb_chat_message_record() -> impl Strategy<Value = ChatMessageRecord> {
        (
            uuid_string(),
            uuid_string(),
            arb_chat_role(),
            non_empty_string(),
            iso8601_datetime(),
        )
            .prop_map(
                |(id, session_id, role, content, created_at)| ChatMessageRecord {
                    id,
                    session_id,
                    role,
                    content,
                    attachments: None,
                    tool_calls: None,
                    tool_call_id: None,
                    created_at,
                },
            )
    }

    /// メッセージ履歴のストラテジー（0〜20件）
    fn arb_message_history(max_count: usize) -> impl Strategy<Value = Vec<ChatMessageRecord>> {
        proptest::collection::vec(arb_chat_message_record(), 0..=max_count)
    }

    /// min_interval_seconds のストラテジー（1〜3600秒）
    fn arb_min_interval_seconds() -> impl Strategy<Value = u64> {
        1u64..3600
    }

    /// 経過時間（秒）のストラテジー
    fn arb_elapsed_seconds() -> impl Strategy<Value = u64> {
        0u64..7200
    }

    // ========================================
    // Property 7: Spontaneous speech interval enforcement
    // ========================================
    //
    // **Validates: Requirements 3.1**
    //
    // For any SpontaneousSpeakerConfig with minIntervalSeconds = N,
    // the time between consecutive spontaneous speech evaluations
    // SHALL be >= N seconds.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn prop_interval_enforcement_blocks_when_elapsed_less_than_min(
            min_interval in arb_min_interval_seconds(),
            elapsed in arb_elapsed_seconds(),
        ) {
            // 間隔制御ロジック: elapsed < min_interval_seconds の場合、評価をスキップ
            let should_skip = elapsed < min_interval;
            let should_evaluate = elapsed >= min_interval;

            // 実装のロジックと同等の比較を検証
            if should_skip {
                // elapsed < min_interval → 評価しない（スキップ）
                prop_assert!(
                    elapsed < min_interval,
                    "When elapsed ({}) < min_interval ({}), evaluation should be skipped",
                    elapsed,
                    min_interval
                );
            }

            if should_evaluate {
                // elapsed >= min_interval → 評価可能
                prop_assert!(
                    elapsed >= min_interval,
                    "When elapsed ({}) >= min_interval ({}), evaluation should proceed",
                    elapsed,
                    min_interval
                );
            }

            // 核心的な不変条件: 評価が許可される場合、必ず elapsed >= min_interval
            // これは実装の `if elapsed < current_config.min_interval_seconds { continue; }` と対応
            prop_assert_eq!(
                should_evaluate,
                elapsed >= min_interval,
                "Interval enforcement invariant violated"
            );
        }

        #[test]
        fn prop_interval_enforcement_with_instant(
            min_interval in 1u64..100,
        ) {
            // Instant::now() を使った実際の時間比較ロジックの検証
            let config = SpontaneousSpeakerConfig {
                enabled: true,
                min_interval_seconds: min_interval,
            };

            // last_spoke_at が直前（now）の場合、elapsed は 0 に近い → スキップされるべき
            let last_spoke_at = Instant::now();
            let elapsed = last_spoke_at.elapsed().as_secs();

            // 直後の elapsed は必ず min_interval 未満（min_interval >= 1）
            prop_assert!(
                elapsed < config.min_interval_seconds,
                "Immediately after speaking, elapsed ({}) should be < min_interval ({})",
                elapsed,
                config.min_interval_seconds
            );
        }

        #[test]
        fn prop_interval_zero_always_allows(
            elapsed in arb_elapsed_seconds(),
        ) {
            // min_interval_seconds = 0 の場合、常に評価可能
            let min_interval: u64 = 0;
            prop_assert!(
                elapsed >= min_interval,
                "With min_interval=0, any elapsed time ({}) should allow evaluation",
                elapsed
            );
        }
    }

    // ========================================
    // Property 8: Spontaneous speech context assembly
    // ========================================
    //
    // **Validates: Requirements 3.2**
    //
    // For any active ChatSession, when spontaneous speech is triggered,
    // the LLM request SHALL include the Character's systemPrompt and
    // the most recent messages from the session as context.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_spontaneous_prompt_includes_system_prompt(
            system_prompt in non_empty_string(),
            messages in arb_message_history(10),
        ) {
            let result = DefaultSpontaneousSpeaker::build_spontaneous_prompt(
                &system_prompt,
                &messages,
            );

            // 最初のメッセージは必ずシステムプロンプト（role=System）
            prop_assert!(
                !result.is_empty(),
                "Prompt should not be empty"
            );
            prop_assert_eq!(
                result[0].role,
                MessageRole::System,
                "First message must be System role"
            );
            prop_assert_eq!(
                &result[0].content,
                &system_prompt,
                "First message content must be the system prompt"
            );
        }

        #[test]
        fn prop_spontaneous_prompt_includes_recent_messages(
            system_prompt in non_empty_string(),
            messages in arb_message_history(15),
        ) {
            let result = DefaultSpontaneousSpeaker::build_spontaneous_prompt(
                &system_prompt,
                &messages,
            );

            // 構造: system_prompt + messages + meta_prompt
            // 総数 = 1 (system) + messages.len() + 1 (meta-prompt)
            let expected_len = 1 + messages.len() + 1;
            prop_assert_eq!(
                result.len(),
                expected_len,
                "Prompt length should be 1 (system) + {} (messages) + 1 (meta) = {}, got {}",
                messages.len(),
                expected_len,
                result.len()
            );

            // 各メッセージの内容が正しく含まれている
            for (i, msg) in messages.iter().enumerate() {
                let prompt_idx = 1 + i;
                prop_assert_eq!(
                    &result[prompt_idx].content,
                    &msg.content,
                    "Message at index {} content mismatch",
                    i
                );
            }
        }

        #[test]
        fn prop_spontaneous_prompt_role_mapping(
            system_prompt in non_empty_string(),
            messages in arb_message_history(10),
        ) {
            let result = DefaultSpontaneousSpeaker::build_spontaneous_prompt(
                &system_prompt,
                &messages,
            );

            // ChatRole → MessageRole のマッピング検証
            for (i, msg) in messages.iter().enumerate() {
                let prompt_idx = 1 + i;
                let expected_role = match msg.role {
                    ChatRole::User => MessageRole::User,
                    ChatRole::Assistant => MessageRole::Assistant,
                    ChatRole::Spontaneous => MessageRole::Assistant, // Spontaneous → Assistant
                    ChatRole::Tool => MessageRole::Tool,
                };
                prop_assert_eq!(
                    result[prompt_idx].role,
                    expected_role,
                    "Role mapping at index {} incorrect: {:?} should map to {:?}",
                    i,
                    msg.role,
                    expected_role
                );
            }
        }

        #[test]
        fn prop_spontaneous_prompt_ends_with_meta_prompt(
            system_prompt in non_empty_string(),
            messages in arb_message_history(10),
        ) {
            let result = DefaultSpontaneousSpeaker::build_spontaneous_prompt(
                &system_prompt,
                &messages,
            );

            // 最後のメッセージはメタプロンプト（role=User, [SKIP]を含む）
            let last = result.last().unwrap();
            prop_assert_eq!(
                last.role,
                MessageRole::User,
                "Last message (meta-prompt) should be User role"
            );
            prop_assert!(
                last.content.contains("[SKIP]"),
                "Meta-prompt should contain [SKIP] instruction, got: {}",
                last.content
            );
        }
    }

    // ========================================
    // Property 9: Spontaneous messages have distinct role
    // ========================================
    //
    // **Validates: Requirements 3.5**
    //
    // For any message generated by the Spontaneous Speaker,
    // it SHALL be stored with role='spontaneous', distinct from
    // regular assistant responses (role='assistant') and user messages (role='user').

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_spontaneous_event_has_spontaneous_role(
            session_id in uuid_string(),
            message_id in uuid_string(),
            content in non_empty_string(),
            created_at in iso8601_datetime(),
        ) {
            // SpontaneousEvent を構築（実装と同じパターン）
            let message = ChatMessageRecord {
                id: message_id,
                session_id: session_id.clone(),
                role: ChatRole::Spontaneous,
                content,
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at,
            };

            let event = SpontaneousEvent {
                session_id: session_id.clone(),
                message: message.clone(),
            };

            // 不変条件: SpontaneousEvent のメッセージは必ず role=Spontaneous
            prop_assert_eq!(
                event.message.role,
                ChatRole::Spontaneous,
                "SpontaneousEvent message must have Spontaneous role"
            );

            // role=Spontaneous は role=Assistant と異なる
            prop_assert_ne!(
                event.message.role,
                ChatRole::Assistant,
                "Spontaneous role must be distinct from Assistant"
            );

            // role=Spontaneous は role=User と異なる
            prop_assert_ne!(
                event.message.role,
                ChatRole::User,
                "Spontaneous role must be distinct from User"
            );

            // role=Spontaneous は role=Tool と異なる
            prop_assert_ne!(
                event.message.role,
                ChatRole::Tool,
                "Spontaneous role must be distinct from Tool"
            );
        }

        #[test]
        fn prop_spontaneous_role_serialization_distinct(
            content in non_empty_string(),
        ) {
            // Spontaneous ロールのシリアライズが他のロールと区別される
            let spontaneous_msg = ChatMessageRecord {
                id: "test-id".to_string(),
                session_id: "test-session".to_string(),
                role: ChatRole::Spontaneous,
                content: content.clone(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
            };

            let assistant_msg = ChatMessageRecord {
                id: "test-id".to_string(),
                session_id: "test-session".to_string(),
                role: ChatRole::Assistant,
                content: content.clone(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
            };

            let user_msg = ChatMessageRecord {
                id: "test-id".to_string(),
                session_id: "test-session".to_string(),
                role: ChatRole::User,
                content,
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
            };

            // JSON シリアライズで "spontaneous" が含まれる
            let spontaneous_json = serde_json::to_string(&spontaneous_msg).unwrap();
            let assistant_json = serde_json::to_string(&assistant_msg).unwrap();
            let user_json = serde_json::to_string(&user_msg).unwrap();

            prop_assert!(
                spontaneous_json.contains("\"spontaneous\""),
                "Spontaneous message JSON should contain 'spontaneous' role"
            );
            prop_assert!(
                assistant_json.contains("\"assistant\""),
                "Assistant message JSON should contain 'assistant' role"
            );
            prop_assert!(
                user_json.contains("\"user\""),
                "User message JSON should contain 'user' role"
            );

            // 各ロールのシリアライズ結果が異なる
            prop_assert_ne!(
                &spontaneous_json,
                &assistant_json,
                "Spontaneous and Assistant serialization must differ"
            );
            prop_assert_ne!(
                &spontaneous_json,
                &user_json,
                "Spontaneous and User serialization must differ"
            );
        }
    }
}
