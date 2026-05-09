// Spontaneous Speaker - 自発的発話制御

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri::Emitter;

use crate::db::database::Database;
use crate::db::repositories::{character as char_repo, chat as chat_repo};
use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
use crate::models::{ChatMessageRecord, ChatRole};

/// 自発的発話設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpontaneousSpeakerConfig {
    pub enabled: bool,
    pub min_interval_seconds: u64,
}

/// 自発的発話イベント（フロントエンドへ送信）
#[derive(Clone, Serialize)]
pub struct SpontaneousEvent {
    pub session_id: String,
    pub message: ChatMessageRecord,
}

/// SpontaneousSpeaker trait
pub trait SpontaneousSpeaker: Send + Sync {
    fn start(&self, session_id: &str, config: SpontaneousSpeakerConfig, app_handle: AppHandle);
    fn stop(&self);
    fn update_config(&self, config: SpontaneousSpeakerConfig);
}

/// 内部状態（Mutex保護）
struct SpeakerState {
    config: SpontaneousSpeakerConfig,
    session_id: Option<String>,
    last_spoke_at: Option<Instant>,
    abort_handle: Option<tokio::task::AbortHandle>,
}

/// デフォルト実装
pub struct DefaultSpontaneousSpeaker {
    db: Arc<Mutex<Database>>,
    llm_client: Arc<dyn LLMClient>,
    llm_config: LLMClientConfig,
    running: Arc<AtomicBool>,
    state: Arc<Mutex<SpeakerState>>,
}

impl DefaultSpontaneousSpeaker {
    pub fn new(
        db: Arc<Mutex<Database>>,
        llm_client: Arc<dyn LLMClient>,
        llm_config: LLMClientConfig,
    ) -> Self {
        Self {
            db,
            llm_client,
            llm_config,
            running: Arc::new(AtomicBool::new(false)),
            state: Arc::new(Mutex::new(SpeakerState {
                config: SpontaneousSpeakerConfig {
                    enabled: false,
                    min_interval_seconds: 60,
                },
                session_id: None,
                last_spoke_at: None,
                abort_handle: None,
            })),
        }
    }

    /// 自発的発話用のメタプロンプトを構築
    pub(crate) fn build_spontaneous_prompt(
        system_prompt: &str,
        recent_messages: &[ChatMessageRecord],
    ) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        // システムプロンプト
        messages.push(ChatMessage {
            role: MessageRole::System,
            content: system_prompt.to_string(),
            tool_call_id: None,
            images: None,
        });

        // 直近の会話コンテキスト
        for msg in recent_messages {
            let role = match msg.role {
                ChatRole::User => MessageRole::User,
                ChatRole::Assistant | ChatRole::Spontaneous => MessageRole::Assistant,
                ChatRole::Tool => MessageRole::Tool,
            };
            messages.push(ChatMessage {
                role,
                content: msg.content.clone(),
                tool_call_id: None,
                images: None,
            });
        }

        // メタプロンプト: 自発的発話を生成するか判断させる
        messages.push(ChatMessage {
            role: MessageRole::User,
            content: concat!(
                "Based on the conversation context, generate a short spontaneous message ",
                "that the character might say. If it's not appropriate to speak now, ",
                "respond with exactly '[SKIP]'."
            )
            .to_string(),
            tool_call_id: None,
            images: None,
        });

        messages
    }
}

impl SpontaneousSpeaker for DefaultSpontaneousSpeaker {
    fn start(&self, session_id: &str, config: SpontaneousSpeakerConfig, app_handle: AppHandle) {
        // 既存タスクがあれば停止
        self.stop();

        self.running.store(true, Ordering::SeqCst);

        {
            let mut state = self.state.lock().unwrap();
            state.config = config.clone();
            state.session_id = Some(session_id.to_string());
            state.last_spoke_at = None;
        }

        if !config.enabled {
            return;
        }

        let running = Arc::clone(&self.running);
        let state = Arc::clone(&self.state);
        let db = Arc::clone(&self.db);
        let llm_client = Arc::clone(&self.llm_client);
        let llm_config = self.llm_config.clone();
        let session_id = session_id.to_string();

        let join_handle = tokio::spawn(async move {
            let interval_duration =
                tokio::time::Duration::from_secs(config.min_interval_seconds.max(1));
            let mut interval = tokio::time::interval(interval_duration);

            // 最初のtickをスキップ（即時発火を防ぐ）
            interval.tick().await;

            loop {
                interval.tick().await;

                // 停止フラグチェック
                if !running.load(Ordering::SeqCst) {
                    break;
                }

                // 設定チェック
                let (current_config, last_spoke_at) = {
                    let s = state.lock().unwrap();
                    (s.config.clone(), s.last_spoke_at)
                };

                if !current_config.enabled {
                    continue;
                }

                // 最小間隔チェック
                if let Some(last) = last_spoke_at {
                    let elapsed = last.elapsed().as_secs();
                    if elapsed < current_config.min_interval_seconds {
                        continue;
                    }
                }

                // DB操作: セッション情報とキャラクター情報を取得
                let (system_prompt, recent_messages) = {
                    let db_guard = db.lock().unwrap();
                    let conn = db_guard.connection();

                    // セッションからcharacter_idを取得
                    let session = match chat_repo::get_session(conn, &session_id) {
                        Ok(Some(s)) => s,
                        _ => continue,
                    };

                    // キャラクターのsystem_promptを取得
                    let character = match char_repo::get_character(conn, &session.character_id) {
                        Ok(Some(c)) => c,
                        _ => continue,
                    };

                    // 直近メッセージを取得（最大20件）
                    let messages = match chat_repo::get_messages(conn, &session_id) {
                        Ok(msgs) => {
                            let len = msgs.len();
                            if len > 20 {
                                msgs[len - 20..].to_vec()
                            } else {
                                msgs
                            }
                        }
                        _ => continue,
                    };

                    (character.system_prompt, messages)
                };

                // LLMプロンプト構築
                let prompt_messages =
                    Self::build_spontaneous_prompt(&system_prompt, &recent_messages);

                // LLM呼び出し
                let response = match llm_client.chat(&prompt_messages, &llm_config, None).await {
                    Ok(resp) => resp,
                    Err(_) => continue,
                };

                // レスポンス解析
                let generated_text = match response {
                    LLMResponse::Text(text) => text.trim().to_string(),
                    LLMResponse::ToolCalls(_) => continue,
                };

                // [SKIP]の場合はスキップ
                if generated_text == "[SKIP]" || generated_text.is_empty() {
                    continue;
                }

                // メッセージ保存
                let now = chrono::Utc::now().to_rfc3339();
                let message_id = uuid::Uuid::new_v4().to_string();
                let message = ChatMessageRecord {
                    id: message_id,
                    session_id: session_id.clone(),
                    role: ChatRole::Spontaneous,
                    content: generated_text,
                    attachments: None,
                    tool_calls: None,
                    tool_call_id: None,
                    created_at: now,
                };

                // DB保存
                {
                    let db_guard = db.lock().unwrap();
                    let conn = db_guard.connection();
                    if chat_repo::insert_message(conn, &message).is_err() {
                        continue;
                    }
                }

                // last_spoke_at更新
                {
                    let mut s = state.lock().unwrap();
                    s.last_spoke_at = Some(Instant::now());
                }

                // イベント発火
                let event = SpontaneousEvent {
                    session_id: session_id.clone(),
                    message,
                };
                let _ = app_handle.emit("spontaneous:message", event);
            }
        });

        // AbortHandle保存
        {
            let mut s = self.state.lock().unwrap();
            s.abort_handle = Some(join_handle.abort_handle());
        }
    }

    fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);

        let mut s = self.state.lock().unwrap();
        if let Some(handle) = s.abort_handle.take() {
            handle.abort();
        }
        s.session_id = None;
    }

    fn update_config(&self, config: SpontaneousSpeakerConfig) {
        let mut s = self.state.lock().unwrap();
        s.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spontaneous_speaker_config_serialization() {
        let config = SpontaneousSpeakerConfig {
            enabled: true,
            min_interval_seconds: 30,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SpontaneousSpeakerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.enabled, true);
        assert_eq!(deserialized.min_interval_seconds, 30);
    }

    #[test]
    fn test_spontaneous_speaker_config_disabled() {
        let config = SpontaneousSpeakerConfig {
            enabled: false,
            min_interval_seconds: 60,
        };
        assert!(!config.enabled);
        assert_eq!(config.min_interval_seconds, 60);
    }

    #[test]
    fn test_spontaneous_event_serialization() {
        let event = SpontaneousEvent {
            session_id: "sess-001".to_string(),
            message: ChatMessageRecord {
                id: "msg-001".to_string(),
                session_id: "sess-001".to_string(),
                role: ChatRole::Spontaneous,
                content: "自発的メッセージ".to_string(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("spontaneous"));
        assert!(json.contains("sess-001"));
        assert!(json.contains("自発的メッセージ"));
    }

    #[test]
    fn test_build_spontaneous_prompt_empty_messages() {
        let messages: Vec<ChatMessageRecord> = vec![];
        let prompt =
            DefaultSpontaneousSpeaker::build_spontaneous_prompt("You are a cat.", &messages);

        // system + meta-prompt = 2メッセージ
        assert_eq!(prompt.len(), 2);
        assert_eq!(prompt[0].role, MessageRole::System);
        assert_eq!(prompt[0].content, "You are a cat.");
        assert_eq!(prompt[1].role, MessageRole::User);
        assert!(prompt[1].content.contains("[SKIP]"));
    }

    #[test]
    fn test_build_spontaneous_prompt_with_messages() {
        let messages = vec![
            ChatMessageRecord {
                id: "msg-001".to_string(),
                session_id: "sess-001".to_string(),
                role: ChatRole::User,
                content: "こんにちは".to_string(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: "2024-01-01T10:00:00Z".to_string(),
            },
            ChatMessageRecord {
                id: "msg-002".to_string(),
                session_id: "sess-001".to_string(),
                role: ChatRole::Assistant,
                content: "にゃー".to_string(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: "2024-01-01T10:01:00Z".to_string(),
            },
        ];

        let prompt =
            DefaultSpontaneousSpeaker::build_spontaneous_prompt("You are a cat.", &messages);

        // system + 2 conversation messages + meta-prompt = 4
        assert_eq!(prompt.len(), 4);
        assert_eq!(prompt[0].role, MessageRole::System);
        assert_eq!(prompt[1].role, MessageRole::User);
        assert_eq!(prompt[1].content, "こんにちは");
        assert_eq!(prompt[2].role, MessageRole::Assistant);
        assert_eq!(prompt[2].content, "にゃー");
        assert_eq!(prompt[3].role, MessageRole::User);
        assert!(prompt[3].content.contains("[SKIP]"));
    }

    #[test]
    fn test_build_spontaneous_prompt_spontaneous_role_mapped_to_assistant() {
        let messages = vec![ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::Spontaneous,
            content: "自発的発話".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        }];

        let prompt =
            DefaultSpontaneousSpeaker::build_spontaneous_prompt("System prompt", &messages);

        // Spontaneous roleはAssistantにマッピングされる
        assert_eq!(prompt[1].role, MessageRole::Assistant);
    }

    #[test]
    fn test_min_interval_enforcement_logic() {
        // min_interval_seconds=60の場合、last_spoke_atから60秒未満ならスキップ
        let config = SpontaneousSpeakerConfig {
            enabled: true,
            min_interval_seconds: 60,
        };

        let last_spoke_at = Some(Instant::now());

        // 直後はスキップされるべき
        if let Some(last) = last_spoke_at {
            let elapsed = last.elapsed().as_secs();
            assert!(elapsed < config.min_interval_seconds);
        }
    }

    #[test]
    fn test_min_interval_zero_allowed() {
        // min_interval_seconds=0でも動作する（即座に発話可能）
        let config = SpontaneousSpeakerConfig {
            enabled: true,
            min_interval_seconds: 0,
        };

        let last_spoke_at = Some(Instant::now());
        if let Some(last) = last_spoke_at {
            let elapsed = last.elapsed().as_secs();
            // 0秒間隔なので常に条件を満たす
            assert!(elapsed >= config.min_interval_seconds);
        }
    }
}
