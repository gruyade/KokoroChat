// Thought Engine - 独自思考管理

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::Serialize;
use tauri::AppHandle;
use tauri::Emitter;

use crate::db::database::Database;
use crate::db::repositories::{
    character as char_repo, chat as chat_repo, memory as memory_repo, thought as thought_repo,
};
use crate::error::AppError;
use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
use crate::models::{ChatRole, Thought};

/// 思考生成イベント（フロントエンドへ送信）
#[derive(Clone, Serialize)]
pub struct ThoughtEvent {
    pub character_id: String,
    pub thought: Thought,
}

/// ThoughtEngine trait
#[async_trait]
pub trait ThoughtEngine: Send + Sync {
    async fn generate_thought(&self, character_id: &str) -> Result<Thought, AppError>;
    async fn get_thoughts(
        &self,
        character_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Thought>, AppError>;
    async fn delete_thought(&self, id: &str) -> Result<(), AppError>;
    async fn cleanup_old_thoughts(
        &self,
        character_id: &str,
        threshold_minutes: u64,
    ) -> Result<u32, AppError>;
    fn set_frequency(&self, character_id: &str, interval_minutes: u64);
    fn start(&self, character_id: &str, app_handle: AppHandle);
    fn stop(&self);
    fn pause(&self);
    fn resume(&self);
    fn is_paused(&self) -> bool;
}

/// 内部状態（Mutex保護）
pub(crate) struct EngineState {
    pub(crate) character_id: Option<String>,
    pub(crate) interval_minutes: u64,
    pub(crate) abort_handle: Option<tokio::task::AbortHandle>,
}

/// デフォルト実装
pub struct DefaultThoughtEngine {
    db: Arc<Mutex<Database>>,
    llm_client: Arc<dyn LLMClient>,
    config_manager: Arc<crate::config::model_config::ModelConfigManager>,
    llm_lock: Arc<tokio::sync::Mutex<()>>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) state: Arc<Mutex<EngineState>>,
}

impl DefaultThoughtEngine {
    pub fn new(
        db: Arc<Mutex<Database>>,
        llm_client: Arc<dyn LLMClient>,
        config_manager: Arc<crate::config::model_config::ModelConfigManager>,
        llm_lock: Arc<tokio::sync::Mutex<()>>,
    ) -> Self {
        Self {
            db,
            llm_client,
            config_manager,
            llm_lock,
            running: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            state: Arc::new(Mutex::new(EngineState {
                character_id: None,
                interval_minutes: 5,
                abort_handle: None,
            })),
        }
    }

    /// 現在のThought用LLM設定を取得
    fn current_llm_config(&self) -> LLMClientConfig {
        self.config_manager
            .get_model_settings(&crate::models::config::ModelPurpose::Thought)
            .map(|s| LLMClientConfig {
                base_url: s.base_url,
                model: s.model,
                api_key: s.api_key,
                temperature: s.temperature,
                provider: s.provider,
            })
            .unwrap_or(LLMClientConfig {
                base_url: String::new(),
                model: String::new(),
                api_key: None,
                temperature: 0.7,
                provider: None,
            })
    }

    /// 思考生成用のメタプロンプトを構築
    pub(crate) fn build_thought_prompt(
        system_prompt: &str,
        recent_messages: &[crate::models::ChatMessageRecord],
        memories: &[crate::models::Memory],
    ) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        // システムプロンプト
        messages.push(ChatMessage {
            role: MessageRole::System,
            content: system_prompt.to_string(),
            tool_call_id: None,
            images: None,
        });

        // 記憶コンテキスト
        if !memories.is_empty() {
            let memory_text: String = memories
                .iter()
                .map(|m| format!("- {}", m.content))
                .collect::<Vec<_>>()
                .join("\n");
            messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!("以下はあなたの記憶です:\n{}", memory_text),
                tool_call_id: None,
                images: None,
            });
        }

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

        // メタプロンプト: 内部思考を生成させる
        messages.push(ChatMessage {
            role: MessageRole::User,
            content: concat!(
                "Based on your personality, the conversation context, and your memories, ",
                "generate an internal thought. This is something you would think privately ",
                "but not necessarily say out loud. It could be a reflection, an observation, ",
                "a feeling, or an idea. Keep it concise (1-3 sentences). ",
                "Respond with only the thought itself, no prefixes or labels."
            )
            .to_string(),
            tool_call_id: None,
            images: None,
        });

        messages
    }
}

#[async_trait]
impl ThoughtEngine for DefaultThoughtEngine {
    async fn generate_thought(&self, character_id: &str) -> Result<Thought, AppError> {
        // DB操作: キャラクター情報、直近会話、記憶を取得
        let (system_prompt, recent_messages, memories) = {
            let db_guard = self.db.lock().unwrap();
            let conn = db_guard.connection();

            // キャラクターのsystem_promptを取得
            let character = char_repo::get_character(conn, character_id)?
                .ok_or_else(|| AppError::NotFound(format!("character {}", character_id)))?;

            // 最新セッションからメッセージを取得
            let sessions = chat_repo::list_sessions(conn, character_id)?;
            let recent_messages = if let Some(session) = sessions.first() {
                let msgs = chat_repo::get_messages(conn, &session.id)?;
                let len = msgs.len();
                if len > 20 {
                    msgs[len - 20..].to_vec()
                } else {
                    msgs
                }
            } else {
                Vec::new()
            };

            // 記憶を取得
            let memories = memory_repo::list_memories(conn, character_id)?;

            (character.system_prompt, recent_messages, memories)
        };

        // LLMプロンプト構築
        let prompt_messages =
            Self::build_thought_prompt(&system_prompt, &recent_messages, &memories);

        // コンテキスト概要を生成（保存用）
        let context_summary = if !recent_messages.is_empty() || !memories.is_empty() {
            let mut parts = Vec::new();
            if !recent_messages.is_empty() {
                parts.push(format!("直近会話{}件", recent_messages.len()));
            }
            if !memories.is_empty() {
                parts.push(format!("記憶{}件", memories.len()));
            }
            Some(parts.join("、"))
        } else {
            None
        };

        // LLMロック取得後に呼び出し
        let _llm_guard = self.llm_lock.lock().await;
        let response = self
            .llm_client
            .chat(&prompt_messages, &self.current_llm_config(), None)
            .await?;
        drop(_llm_guard);

        let content = match response {
            LLMResponse::Text(text) => text.trim().to_string(),
            LLMResponse::ToolCalls(_) => {
                return Err(AppError::LlmApi(
                    "Unexpected tool_calls response for thought generation".to_string(),
                ));
            }
        };

        if content.is_empty() {
            return Err(AppError::LlmApi("Empty thought generated".to_string()));
        }

        // Thought構築
        let now = chrono::Utc::now().to_rfc3339();
        let thought = Thought {
            id: uuid::Uuid::new_v4().to_string(),
            character_id: character_id.to_string(),
            content,
            context: context_summary,
            created_at: now,
        };

        // DB保存
        {
            let db_guard = self.db.lock().unwrap();
            let conn = db_guard.connection();
            thought_repo::insert_thought(conn, &thought)?;
        }

        // 自動クリーンアップ（閾値超過の古い思考を削除）
        let threshold = self
            .config_manager
            .get_config()
            .thought
            .auto_delete_threshold_minutes;
        if let Err(e) = self.cleanup_old_thoughts(character_id, threshold).await {
            println!("[thought] cleanup error (non-fatal): {}", e);
        }

        Ok(thought)
    }

    async fn get_thoughts(
        &self,
        character_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Thought>, AppError> {
        let db_guard = self.db.lock().unwrap();
        let conn = db_guard.connection();
        thought_repo::get_thoughts(conn, character_id, limit)
    }

    async fn delete_thought(&self, id: &str) -> Result<(), AppError> {
        let db_guard = self.db.lock().unwrap();
        let conn = db_guard.connection();
        let deleted = thought_repo::delete_thought(conn, id)?;
        if !deleted {
            return Err(AppError::NotFound(format!("thought {}", id)));
        }
        Ok(())
    }

    async fn cleanup_old_thoughts(
        &self,
        character_id: &str,
        threshold_minutes: u64,
    ) -> Result<u32, AppError> {
        // threshold=0 は全保持（クリーンアップスキップ）
        if threshold_minutes == 0 {
            return Ok(0);
        }

        let cutoff = chrono::Utc::now() - chrono::Duration::minutes(threshold_minutes as i64);
        let cutoff_str = cutoff.to_rfc3339();

        let db_guard = self.db.lock().unwrap();
        let conn = db_guard.connection();
        thought_repo::delete_thoughts_older_than(conn, character_id, &cutoff_str)
    }

    fn set_frequency(&self, character_id: &str, interval_minutes: u64) {
        let mut state = self.state.lock().unwrap();
        state.character_id = Some(character_id.to_string());
        state.interval_minutes = interval_minutes;
    }

    fn start(&self, character_id: &str, app_handle: AppHandle) {
        // 既存タスクがあれば停止
        self.stop();

        self.running.store(true, Ordering::SeqCst);

        {
            let mut state = self.state.lock().unwrap();
            state.character_id = Some(character_id.to_string());
        }

        let running = Arc::clone(&self.running);
        let paused = Arc::clone(&self.paused);
        let state = Arc::clone(&self.state);
        let db = Arc::clone(&self.db);
        let llm_client = Arc::clone(&self.llm_client);
        let config_manager = Arc::clone(&self.config_manager);
        let llm_lock = Arc::clone(&self.llm_lock);
        let character_id = character_id.to_string();

        let join_handle = tokio::spawn(async move {
            loop {
                // 間隔を取得
                let interval_minutes = {
                    let s = state.lock().unwrap();
                    s.interval_minutes
                };

                let interval_duration =
                    tokio::time::Duration::from_secs(interval_minutes.max(1) * 60);
                tokio::time::sleep(interval_duration).await;

                // 停止フラグチェック
                if !running.load(Ordering::SeqCst) {
                    break;
                }

                // 一時停止フラグチェック
                if paused.load(Ordering::SeqCst) {
                    continue;
                }

                // 思考生成
                let result = {
                    // DB操作: キャラクター情報、直近会話、記憶を取得
                    let (system_prompt, recent_messages, memories) = {
                        let db_guard = db.lock().unwrap();
                        let conn = db_guard.connection();

                        let character = match char_repo::get_character(conn, &character_id) {
                            Ok(Some(c)) => c,
                            _ => continue,
                        };

                        let sessions = match chat_repo::list_sessions(conn, &character_id) {
                            Ok(s) => s,
                            _ => continue,
                        };

                        let recent_messages = if let Some(session) = sessions.first() {
                            match chat_repo::get_messages(conn, &session.id) {
                                Ok(msgs) => {
                                    let len = msgs.len();
                                    if len > 20 {
                                        msgs[len - 20..].to_vec()
                                    } else {
                                        msgs
                                    }
                                }
                                _ => Vec::new(),
                            }
                        } else {
                            Vec::new()
                        };

                        let memories = memory_repo::list_memories(conn, &character_id).unwrap_or_default();

                        (character.system_prompt, recent_messages, memories)
                    };

                    // LLMプロンプト構築
                    let prompt_messages = DefaultThoughtEngine::build_thought_prompt(
                        &system_prompt,
                        &recent_messages,
                        &memories,
                    );

                    // LLMロック取得後に呼び出し
                    let _llm_guard = llm_lock.lock().await;
                    let llm_config = config_manager
                        .get_model_settings(&crate::models::config::ModelPurpose::Thought)
                        .map(|s| LLMClientConfig {
                            base_url: s.base_url,
                            model: s.model,
                            api_key: s.api_key,
                            temperature: s.temperature,
                            provider: s.provider,
                        })
                        .unwrap_or(LLMClientConfig {
                            base_url: String::new(),
                            model: String::new(),
                            api_key: None,
                            temperature: 0.7,
                            provider: None,
                        });

                    let response = match llm_client.chat(&prompt_messages, &llm_config, None).await
                    {
                        Ok(resp) => resp,
                        Err(_) => continue,
                    };
                    drop(_llm_guard);

                    let content = match response {
                        LLMResponse::Text(text) => text.trim().to_string(),
                        LLMResponse::ToolCalls(_) => continue,
                    };

                    if content.is_empty() {
                        continue;
                    }

                    // コンテキスト概要
                    let context_summary = if !recent_messages.is_empty() || !memories.is_empty() {
                        let mut parts = Vec::new();
                        if !recent_messages.is_empty() {
                            parts.push(format!("直近会話{}件", recent_messages.len()));
                        }
                        if !memories.is_empty() {
                            parts.push(format!("記憶{}件", memories.len()));
                        }
                        Some(parts.join("、"))
                    } else {
                        None
                    };

                    // Thought構築
                    let now = chrono::Utc::now().to_rfc3339();
                    let thought = Thought {
                        id: uuid::Uuid::new_v4().to_string(),
                        character_id: character_id.clone(),
                        content,
                        context: context_summary,
                        created_at: now,
                    };

                    // DB保存
                    {
                        let db_guard = db.lock().unwrap();
                        let conn = db_guard.connection();
                        if thought_repo::insert_thought(conn, &thought).is_err() {
                            continue;
                        }
                    }

                    // 自動クリーンアップ（閾値超過の古い思考を削除）
                    let threshold = config_manager
                        .get_config()
                        .thought
                        .auto_delete_threshold_minutes;
                    if threshold > 0 {
                        let cutoff =
                            chrono::Utc::now() - chrono::Duration::minutes(threshold as i64);
                        let cutoff_str = cutoff.to_rfc3339();
                        let db_guard = db.lock().unwrap();
                        let conn = db_guard.connection();
                        if let Err(e) = thought_repo::delete_thoughts_older_than(
                            conn,
                            &character_id,
                            &cutoff_str,
                        ) {
                            println!("[thought] cleanup error (non-fatal): {}", e);
                        }
                    }

                    thought
                };

                // イベント発火
                let event = ThoughtEvent {
                    character_id: character_id.clone(),
                    thought: result,
                };
                let _ = app_handle.emit("thought:generated", event);
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
        s.character_id = None;
    }

    fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
        println!("[thought] engine paused");
    }

    fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        println!("[thought] engine resumed");
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
}
