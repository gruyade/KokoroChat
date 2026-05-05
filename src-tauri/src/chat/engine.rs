// Chat Engine - チャット処理エンジン

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::db::database::Database;
use crate::db::repositories::{character as char_repo, chat as chat_repo, memory as mem_repo, thought as thought_repo};
use crate::error::AppError;
use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
use crate::models::{Attachment, ChatMessageRecord, ChatRole, ChatSession, MessageAttachment, Thought};

/// ストリーミングチャットイベント
#[derive(Clone, Serialize)]
pub struct ChatStreamEvent {
    pub session_id: String,
    pub chunk: String,
    pub done: bool,
}

/// ツール実行中イベント
#[derive(Clone, Serialize)]
pub struct ToolExecutingEvent {
    pub session_id: String,
    pub tool_name: String,
}

/// ChatEngine trait — チャット機能の抽象インターフェース
#[async_trait]
pub trait ChatEngine: Send + Sync {
    /// 新規セッション作成（session_idを返す）
    async fn create_session(&self, character_id: &str) -> Result<String, AppError>;

    /// メッセージ送信（ストリーミングでイベント発火）
    async fn send_message(
        &self,
        session_id: &str,
        content: &str,
        attachments: Option<Vec<Attachment>>,
        app_handle: &AppHandle,
    ) -> Result<(), AppError>;

    /// セッションのメッセージ履歴取得
    async fn get_history(&self, session_id: &str) -> Result<Vec<ChatMessageRecord>, AppError>;

    /// キャラクターのセッション一覧取得
    async fn list_sessions(&self, character_id: &str) -> Result<Vec<ChatSession>, AppError>;

    /// セッション削除
    async fn delete_session(&self, session_id: &str) -> Result<(), AppError>;
}

/// デフォルトChatEngine実装
pub struct DefaultChatEngine {
    db: Arc<Mutex<Database>>,
    llm_client: Arc<dyn LLMClient>,
    config_manager: Arc<crate::config::model_config::ModelConfigManager>,
    /// LLMリクエスト直列化用ロック
    llm_lock: Arc<tokio::sync::Mutex<()>>,
}

impl DefaultChatEngine {
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
        }
    }

    /// 現在のChat用LLM設定を取得
    fn current_llm_config(&self) -> LLMClientConfig {
        self.config_manager
            .get_model_settings(&crate::models::config::ModelPurpose::Chat)
            .map(|s| LLMClientConfig {
                base_url: s.base_url,
                model: s.model,
                api_key: s.api_key,
                temperature: s.temperature,
            })
            .unwrap_or(LLMClientConfig {
                base_url: String::new(),
                model: String::new(),
                api_key: None,
                temperature: 0.7,
            })
    }

    /// コンテキストメッセージ配列を組み立て
    /// [system_prompt, ...thought_context, ...memory_context, ...chat_history, user_message]
    pub(crate) fn build_context(
        &self,
        system_prompt: &str,
        memories: &[crate::models::Memory],
        thoughts: &[Thought],
        history: &[ChatMessageRecord],
        user_content: &str,
        attachment_text: Option<&str>,
    ) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        // 1. System Prompt（思考コンテキストがあれば付加）
        let system_content = if thoughts.is_empty() {
            system_prompt.to_string()
        } else {
            let thought_lines: Vec<String> = thoughts
                .iter()
                .map(|t| format!("- {}", t.content))
                .collect();
            format!(
                "{}\n\n## Recent Thoughts\n{}",
                system_prompt,
                thought_lines.join("\n")
            )
        };

        messages.push(ChatMessage {
            role: MessageRole::System,
            content: system_content,
            tool_call_id: None,
        });

        // 2. Memory context（システムメッセージとして挿入）
        for memory in memories {
            messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!("[Memory] {}", memory.content),
                tool_call_id: None,
            });
        }

        // 3. Chat History
        for msg in history {
            let role = match msg.role {
                ChatRole::User => MessageRole::User,
                ChatRole::Assistant => MessageRole::Assistant,
                ChatRole::Spontaneous => MessageRole::Assistant,
                ChatRole::Tool => MessageRole::Tool,
            };
            messages.push(ChatMessage {
                role,
                content: msg.content.clone(),
                tool_call_id: msg.tool_call_id.clone(),
            });
        }

        // 4. User message（添付テキストがあれば含める）
        let final_content = match attachment_text {
            Some(text) => format!("{}\n\n[Attached Files]\n{}", user_content, text),
            None => user_content.to_string(),
        };
        messages.push(ChatMessage {
            role: MessageRole::User,
            content: final_content,
            tool_call_id: None,
        });

        messages
    }

    /// 添付ファイルから抽出テキストを結合
    pub(crate) fn extract_attachment_text(attachments: &[Attachment]) -> Option<String> {
        let texts: Vec<String> = attachments
            .iter()
            .filter_map(|a| {
                a.extracted_text
                    .as_ref()
                    .map(|t| format!("--- {} ---\n{}", a.file_name, t))
            })
            .collect();

        if texts.is_empty() {
            None
        } else {
            Some(texts.join("\n\n"))
        }
    }

    /// 添付ファイル情報をMessageAttachment形式に変換
    pub(crate) fn to_message_attachments(attachments: &[Attachment]) -> Vec<MessageAttachment> {
        attachments
            .iter()
            .map(|a| {
                let type_str = match a.attachment_type {
                    crate::models::AttachmentType::Text => "text",
                    crate::models::AttachmentType::Pdf => "pdf",
                    crate::models::AttachmentType::Image => "image",
                };
                MessageAttachment {
                    file_name: a.file_name.clone(),
                    attachment_type: type_str.to_string(),
                    extracted_text: a.extracted_text.clone(),
                    base64_data: a.base64_data.clone(),
                }
            })
            .collect()
    }
}

#[async_trait]
impl ChatEngine for DefaultChatEngine {
    async fn create_session(&self, character_id: &str) -> Result<String, AppError> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let session = ChatSession {
            id: session_id.clone(),
            character_id: character_id.to_string(),
            title: None,
            last_message_at: None,
            last_message_preview: None,
            created_at: now,
        };

        let db = self.db.lock().map_err(|e| {
            AppError::Database(format!("Failed to acquire DB lock: {}", e))
        })?;
        chat_repo::insert_session(db.connection(), &session)?;

        Ok(session_id)
    }

    async fn send_message(
        &self,
        session_id: &str,
        content: &str,
        attachments: Option<Vec<Attachment>>,
        app_handle: &AppHandle,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().to_rfc3339();
        let user_msg_id = uuid::Uuid::new_v4().to_string();

        // 添付ファイル処理
        let attachment_text = attachments
            .as_ref()
            .and_then(|a| Self::extract_attachment_text(a));
        let message_attachments = attachments
            .as_ref()
            .map(|a| Self::to_message_attachments(a));

        // 1. ユーザーメッセージをDB保存
        let user_message = ChatMessageRecord {
            id: user_msg_id,
            session_id: session_id.to_string(),
            role: ChatRole::User,
            content: content.to_string(),
            attachments: message_attachments,
            tool_calls: None,
            tool_call_id: None,
            created_at: now.clone(),
        };

        // DB操作（ロック範囲を最小化）
        let (system_prompt, memories, thoughts, history) = {
            let db = self.db.lock().map_err(|e| {
                AppError::Database(format!("Failed to acquire DB lock: {}", e))
            })?;
            let conn = db.connection();

            // ユーザーメッセージ保存
            chat_repo::insert_message(conn, &user_message)?;

            // セッション情報取得
            let session = chat_repo::get_session(conn, session_id)?
                .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

            // キャラクター取得
            let character = char_repo::get_character(conn, &session.character_id)?
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "Character not found: {}",
                        session.character_id
                    ))
                })?;

            // メモリ取得（現時点では全メモリ取得）
            let memories = mem_repo::list_memories(conn, &session.character_id)?;

            // 閾値内の最近の思考を取得
            let threshold_minutes = self.config_manager.get_config().thought.auto_delete_threshold_minutes;
            let thoughts = if threshold_minutes > 0 {
                let since = chrono::Utc::now() - chrono::Duration::minutes(threshold_minutes as i64);
                let since_str = since.to_rfc3339();
                thought_repo::get_recent_thoughts(conn, &session.character_id, &since_str)?
            } else {
                // threshold=0: 全思考を取得（自動削除無効 = 全保持）
                thought_repo::get_thoughts(conn, &session.character_id, None)?
            };

            // チャット履歴取得
            let history = chat_repo::get_messages(conn, session_id)?;

            (character.system_prompt, memories, thoughts, history)
        };

        // 2. コンテキスト組み立て
        let llm_messages = self.build_context(
            &system_prompt,
            &memories,
            &thoughts,
            &history,
            content,
            attachment_text.as_deref(),
        );

        // 3. LLMストリーミング呼び出し（ロック取得→完了まで保持）
        let session_id_owned = session_id.to_string();
        let app_handle_clone = app_handle.clone();

        let session_id_for_callback = session_id_owned.clone();
        let callback = Box::new(move |chunk: String| {
            let _ = app_handle_clone.emit(
                "chat:stream",
                ChatStreamEvent {
                    session_id: session_id_for_callback.clone(),
                    chunk,
                    done: false,
                },
            );
        });

        let _llm_guard = self.llm_lock.lock().await;
        let full_response = self
            .llm_client
            .chat_stream(&llm_messages, &self.current_llm_config(), callback)
            .await?;
        drop(_llm_guard);

        // 4. ストリーミング完了イベント
        app_handle.emit(
            "chat:stream",
            ChatStreamEvent {
                session_id: session_id_owned.clone(),
                chunk: String::new(),
                done: true,
            },
        ).map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

        // 5. tool_call判定（非ストリーミングで再度呼び出し）
        // ストリーミングではtool_callを検出できないため、
        // レスポンスがtool_call形式かどうかを確認
        // 現時点ではストリーミングレスポンスをテキストとして保存
        let assistant_msg_id = uuid::Uuid::new_v4().to_string();
        let assistant_now = chrono::Utc::now().to_rfc3339();

        let assistant_message = ChatMessageRecord {
            id: assistant_msg_id,
            session_id: session_id_owned.clone(),
            role: ChatRole::Assistant,
            content: full_response.clone(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: assistant_now.clone(),
        };

        // 6. アシスタントメッセージ保存 & セッションメタデータ更新
        {
            let db = self.db.lock().map_err(|e| {
                AppError::Database(format!("Failed to acquire DB lock: {}", e))
            })?;
            let conn = db.connection();

            chat_repo::insert_message(conn, &assistant_message)?;

            // セッションメタデータ更新
            let preview = truncate_str(&full_response, 50);
            chat_repo::update_session_metadata(conn, session_id, &assistant_now, &preview)?;
        }

        Ok(())
    }

    async fn get_history(&self, session_id: &str) -> Result<Vec<ChatMessageRecord>, AppError> {
        let db = self.db.lock().map_err(|e| {
            AppError::Database(format!("Failed to acquire DB lock: {}", e))
        })?;
        chat_repo::get_messages(db.connection(), session_id)
    }

    async fn list_sessions(&self, character_id: &str) -> Result<Vec<ChatSession>, AppError> {
        let db = self.db.lock().map_err(|e| {
            AppError::Database(format!("Failed to acquire DB lock: {}", e))
        })?;
        chat_repo::list_sessions(db.connection(), character_id)
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), AppError> {
        let db = self.db.lock().map_err(|e| {
            AppError::Database(format!("Failed to acquire DB lock: {}", e))
        })?;
        chat_repo::delete_session(db.connection(), session_id)
    }
}

/// tool_call対応のsend_message（非ストリーミング版）
/// Plugin Systemが実装された後に使用
impl DefaultChatEngine {
    /// 非ストリーミングでLLM呼び出し（tool_call検出用）
    pub async fn send_message_with_tools(
        &self,
        session_id: &str,
        content: &str,
        attachments: Option<Vec<Attachment>>,
        app_handle: &AppHandle,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().to_rfc3339();
        let user_msg_id = uuid::Uuid::new_v4().to_string();

        let attachment_text = attachments
            .as_ref()
            .and_then(|a| Self::extract_attachment_text(a));
        let message_attachments = attachments
            .as_ref()
            .map(|a| Self::to_message_attachments(a));

        let user_message = ChatMessageRecord {
            id: user_msg_id,
            session_id: session_id.to_string(),
            role: ChatRole::User,
            content: content.to_string(),
            attachments: message_attachments,
            tool_calls: None,
            tool_call_id: None,
            created_at: now.clone(),
        };

        let (system_prompt, memories, thoughts, history) = {
            let db = self.db.lock().map_err(|e| {
                AppError::Database(format!("Failed to acquire DB lock: {}", e))
            })?;
            let conn = db.connection();

            chat_repo::insert_message(conn, &user_message)?;

            let session = chat_repo::get_session(conn, session_id)?
                .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

            let character = char_repo::get_character(conn, &session.character_id)?
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "Character not found: {}",
                        session.character_id
                    ))
                })?;

            let memories = mem_repo::list_memories(conn, &session.character_id)?;

            // 閾値内の最近の思考を取得
            let threshold_minutes = self.config_manager.get_config().thought.auto_delete_threshold_minutes;
            let thoughts = if threshold_minutes > 0 {
                let since = chrono::Utc::now() - chrono::Duration::minutes(threshold_minutes as i64);
                let since_str = since.to_rfc3339();
                thought_repo::get_recent_thoughts(conn, &session.character_id, &since_str)?
            } else {
                thought_repo::get_thoughts(conn, &session.character_id, None)?
            };

            let history = chat_repo::get_messages(conn, session_id)?;

            (character.system_prompt, memories, thoughts, history)
        };

        let llm_messages = self.build_context(
            &system_prompt,
            &memories,
            &thoughts,
            &history,
            content,
            attachment_text.as_deref(),
        );

        // 非ストリーミング呼び出し（tool_call検出可能）— ロック取得
        let _llm_guard = self.llm_lock.lock().await;
        let response = self
            .llm_client
            .chat(&llm_messages, &self.current_llm_config(), None)
            .await?;
        drop(_llm_guard);

        let assistant_msg_id = uuid::Uuid::new_v4().to_string();
        let assistant_now = chrono::Utc::now().to_rfc3339();

        match response {
            LLMResponse::Text(text) => {
                // テキストレスポンス
                app_handle.emit(
                    "chat:stream",
                    ChatStreamEvent {
                        session_id: session_id.to_string(),
                        chunk: text.clone(),
                        done: false,
                    },
                ).map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

                app_handle.emit(
                    "chat:stream",
                    ChatStreamEvent {
                        session_id: session_id.to_string(),
                        chunk: String::new(),
                        done: true,
                    },
                ).map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

                let assistant_message = ChatMessageRecord {
                    id: assistant_msg_id,
                    session_id: session_id.to_string(),
                    role: ChatRole::Assistant,
                    content: text.clone(),
                    attachments: None,
                    tool_calls: None,
                    tool_call_id: None,
                    created_at: assistant_now.clone(),
                };

                let db = self.db.lock().map_err(|e| {
                    AppError::Database(format!("Failed to acquire DB lock: {}", e))
                })?;
                let conn = db.connection();

                chat_repo::insert_message(conn, &assistant_message)?;

                let preview = truncate_str(&text, 50);
                chat_repo::update_session_metadata(conn, session_id, &assistant_now, &preview)?;
            }
            LLMResponse::ToolCalls(tool_calls) => {
                // tool_callレスポンス — イベント発火してDB保存
                for tc in &tool_calls {
                    app_handle.emit(
                        "tool:executing",
                        ToolExecutingEvent {
                            session_id: session_id.to_string(),
                            tool_name: tc.name.clone(),
                        },
                    ).map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;
                }

                let assistant_message = ChatMessageRecord {
                    id: assistant_msg_id,
                    session_id: session_id.to_string(),
                    role: ChatRole::Assistant,
                    content: String::new(),
                    attachments: None,
                    tool_calls: Some(tool_calls),
                    tool_call_id: None,
                    created_at: assistant_now.clone(),
                };

                let db = self.db.lock().map_err(|e| {
                    AppError::Database(format!("Failed to acquire DB lock: {}", e))
                })?;
                let conn = db.connection();

                chat_repo::insert_message(conn, &assistant_message)?;
                chat_repo::update_session_metadata(
                    conn,
                    session_id,
                    &assistant_now,
                    "[Tool Call]",
                )?;
            }
        }

        Ok(())
    }
}

/// UTF-8安全な文字列切り詰め（文字境界を考慮）
fn truncate_str(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    if truncated.len() < s.len() {
        format!("{}...", truncated)
    } else {
        truncated
    }
}
