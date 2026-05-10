// Chat Engine - チャット処理エンジン

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::db::database::Database;
use crate::db::repositories::{
    character as char_repo, chat as chat_repo, chat_tool_permission as perm_repo,
    memory as mem_repo, thought as thought_repo,
};
use crate::error::AppError;
use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
use crate::models::tts::{TTSCompleteEvent, TTSErrorEvent, TTSGeneratingEvent};
use crate::models::{
    Attachment, ChatMessageRecord, ChatRole, ChatSession, MessageAttachment, Thought,
};
use crate::plugin::system::PluginSystem;
use crate::tts::connector::TTSConnector;
use crate::tts::flow_controller::TTSFlowController;

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
    /// `partial_content_accumulator` が Some の場合、ストリーミング中に部分コンテンツを蓄積する
    async fn send_message(
        &self,
        session_id: &str,
        content: &str,
        attachments: Option<Vec<Attachment>>,
        app_handle: &AppHandle,
        partial_content_accumulator: Option<Arc<Mutex<String>>>,
    ) -> Result<(), AppError>;

    /// メッセージ再生成（対象assistantメッセージ削除→直前userメッセージで再送信）
    /// `partial_content_accumulator` が Some の場合、ストリーミング中に部分コンテンツを蓄積する
    async fn regenerate(
        &self,
        session_id: &str,
        target_message_id: &str,
        app_handle: &AppHandle,
        partial_content_accumulator: Option<Arc<Mutex<String>>>,
    ) -> Result<(), AppError>;

    /// セッションのメッセージ履歴取得
    async fn get_history(&self, session_id: &str) -> Result<Vec<ChatMessageRecord>, AppError>;

    /// キャラクターのセッション一覧取得
    async fn list_sessions(&self, character_id: &str) -> Result<Vec<ChatSession>, AppError>;

    /// セッション削除
    async fn delete_session(&self, session_id: &str) -> Result<(), AppError>;

    /// メッセージ編集＋再送信（後続メッセージ削除 → 内容更新 → 再送信）
    /// `partial_content_accumulator` が Some の場合、ストリーミング中に部分コンテンツを蓄積する
    async fn edit_and_resend(
        &self,
        session_id: &str,
        message_id: &str,
        new_content: &str,
        app_handle: &AppHandle,
        partial_content_accumulator: Option<Arc<Mutex<String>>>,
    ) -> Result<(), AppError>;
}

/// デフォルトChatEngine実装
pub struct DefaultChatEngine {
    db: Arc<Mutex<Database>>,
    llm_client: Arc<dyn LLMClient>,
    config_manager: Arc<crate::config::model_config::ModelConfigManager>,
    /// LLMリクエスト直列化用ロック
    llm_lock: Arc<tokio::sync::Mutex<()>>,
    /// TTS音声合成コネクタ（将来のTTS直接呼び出し用に保持）
    #[allow(dead_code)]
    tts_connector: Arc<dyn TTSConnector>,
    /// TTS Flow Controller（TTS有効時の音声生成オーケストレーター）
    tts_flow_controller: Option<Arc<TTSFlowController>>,
    /// プラグインシステム（ツール実行ディスパッチ）
    plugin_system: Option<Arc<dyn PluginSystem>>,
}

impl DefaultChatEngine {
    pub fn new(
        db: Arc<Mutex<Database>>,
        llm_client: Arc<dyn LLMClient>,
        config_manager: Arc<crate::config::model_config::ModelConfigManager>,
        llm_lock: Arc<tokio::sync::Mutex<()>>,
        tts_connector: Arc<dyn TTSConnector>,
        tts_flow_controller: Option<Arc<TTSFlowController>>,
        plugin_system: Option<Arc<dyn PluginSystem>>,
    ) -> Self {
        Self {
            db,
            llm_client,
            config_manager,
            llm_lock,
            tts_connector,
            tts_flow_controller,
            plugin_system,
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

    /// TTS用LLM応答をパース: {"display": "...", "speech": "..."}
    /// パース失敗時はフォールバックとして全文を両方に使用
    fn parse_tts_response(response: &str) -> (String, String) {
        // JSONブロック抽出（```json...```対応）
        let json_str = if let Some(start) = response.find("```json") {
            let after = &response[start + 7..];
            if let Some(end) = after.find("```") {
                after[..end].trim()
            } else {
                response.trim()
            }
        } else if let Some(start) = response.find("```") {
            let after = &response[start + 3..];
            if let Some(end) = after.find("```") {
                after[..end].trim()
            } else {
                response.trim()
            }
        } else if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response.trim()
            }
        } else {
            response.trim()
        };

        #[derive(serde::Deserialize)]
        struct TtsResponse {
            display: Option<String>,
            speech: Option<String>,
        }

        match serde_json::from_str::<TtsResponse>(json_str) {
            Ok(parsed) => {
                let display = parsed.display.unwrap_or_else(|| response.to_string());
                let speech = parsed.speech.unwrap_or_else(|| display.clone());
                (display, speech)
            }
            Err(e) => {
                println!(
                    "[TTS] JSON parse failed ({}), using full text as fallback",
                    e
                );
                (response.to_string(), response.to_string())
            }
        }
    }

    /// 圧縮済みメッセージを履歴から除外するフィルタ
    /// memoriesから現在のセッションに対応する最新のMemoryを探し、
    /// そのsource_message_to以前のメッセージを除外する
    pub(crate) fn filter_compressed_history(
        history: &[ChatMessageRecord],
        memories: &[crate::models::Memory],
        session_id: &str,
    ) -> Vec<ChatMessageRecord> {
        // memoriesはDESC順なので、最初にマッチしたものが最新
        let last_compressed_message_id = memories
            .iter()
            .filter(|m| m.source_session_id.as_deref() == Some(session_id))
            .filter_map(|m| m.source_message_to.as_deref())
            .next();

        if let Some(last_id) = last_compressed_message_id {
            if let Some(pos) = history.iter().position(|m| m.id == last_id) {
                // pos+1以降のメッセージのみ返す（圧縮済み範囲を除外）
                return history[pos + 1..].to_vec();
            }
        }

        // フィルタ不要（圧縮Memoryなし or 該当メッセージが見つからない）
        history.to_vec()
    }

    /// コンテキストメッセージ配列を組み立て
    /// [system_prompt, ...thought_context, ...memory_context, ...chat_history, user_message]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_context(
        &self,
        system_prompt: &str,
        memories: &[crate::models::Memory],
        thoughts: &[Thought],
        history: &[ChatMessageRecord],
        user_content: &str,
        attachment_text: Option<&str>,
        attachment_images: Option<Vec<String>>,
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
            images: None,
        });

        // 2. Memory context（システムメッセージとして挿入）
        for memory in memories {
            messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!("[Memory] {}", memory.content),
                tool_call_id: None,
                images: None,
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
                images: None,
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
            images: attachment_images,
        });

        messages
    }

    /// セッション単位のツール許可設定を加味して、LLMに渡すツール定義をフィルタリング
    /// design.md の許可優先順位:
    ///   1. グローバルで disabled → 常に使用不可（get_enabled_tools() で既に除外済み）
    ///   2. グローバルで enabled かつ チャットで disabled → 使用不可
    ///   3. グローバルで enabled かつ チャットで enabled → 使用可能
    pub(crate) fn filter_tools_by_session_permissions(
        &self,
        session_id: &str,
        global_tools: Vec<crate::models::ToolDefinition>,
    ) -> Vec<crate::models::ToolDefinition> {
        let permissions = {
            let db = match self.db.lock() {
                Ok(db) => db,
                Err(_) => return global_tools, // ロック取得失敗時はフィルタなしで返す
            };
            match perm_repo::get_session_tool_permissions(db.connection(), session_id) {
                Ok(perms) => perms,
                Err(_) => return global_tools, // DB読み取り失敗時はフィルタなしで返す
            }
        };

        // 許可設定が空（未初期化）の場合はグローバル設定をそのまま使用
        if permissions.is_empty() {
            return global_tools;
        }

        // セッションで無効化されたツールを除外
        let disabled_tools: std::collections::HashSet<&str> = permissions
            .iter()
            .filter(|p| !p.is_enabled)
            .map(|p| p.tool_name.as_str())
            .collect();

        global_tools
            .into_iter()
            .filter(|t| !disabled_tools.contains(t.name.as_str()))
            .collect()
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

    /// 添付ファイルから画像のbase64データを抽出
    pub(crate) fn extract_attachment_images(attachments: &[Attachment]) -> Option<Vec<String>> {
        let images: Vec<String> = attachments
            .iter()
            .filter_map(|a| a.base64_data.clone())
            .collect();

        if images.is_empty() {
            None
        } else {
            Some(images)
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

        let db = self
            .db
            .lock()
            .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
        chat_repo::insert_session(db.connection(), &session)?;

        Ok(session_id)
    }

    async fn send_message(
        &self,
        session_id: &str,
        content: &str,
        attachments: Option<Vec<Attachment>>,
        app_handle: &AppHandle,
        partial_content_accumulator: Option<Arc<Mutex<String>>>,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().to_rfc3339();
        let user_msg_id = uuid::Uuid::new_v4().to_string();

        // 添付ファイル処理
        let attachment_text = attachments
            .as_ref()
            .and_then(|a| Self::extract_attachment_text(a));
        let attachment_images = attachments
            .as_ref()
            .and_then(|a| Self::extract_attachment_images(a));
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
        let (system_prompt, memories, thoughts, history, tts_config) = {
            let db = self
                .db
                .lock()
                .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
            let conn = db.connection();

            // ユーザーメッセージ保存
            chat_repo::insert_message(conn, &user_message)?;

            // セッション情報取得
            let session = chat_repo::get_session(conn, session_id)?
                .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

            // キャラクター取得
            let character =
                char_repo::get_character(conn, &session.character_id)?.ok_or_else(|| {
                    AppError::NotFound(format!("Character not found: {}", session.character_id))
                })?;

            // メモリ取得（現時点では全メモリ取得）
            let memories = mem_repo::list_memories(conn, &session.character_id)?;

            // 閾値内の最近の思考を取得
            let threshold_minutes = self
                .config_manager
                .get_config()
                .thought
                .auto_delete_threshold_minutes;
            let thoughts = if threshold_minutes > 0 {
                let since =
                    chrono::Utc::now() - chrono::Duration::minutes(threshold_minutes as i64);
                let since_str = since.to_rfc3339();
                thought_repo::get_recent_thoughts(conn, &session.character_id, &since_str)?
            } else {
                // threshold=0: 全思考を取得（自動削除無効 = 全保持）
                thought_repo::get_thoughts(conn, &session.character_id, None)?
            };

            // チャット履歴取得
            let history = chat_repo::get_messages(conn, session_id)?;

            (
                character.system_prompt,
                memories,
                thoughts,
                history,
                character.tts_config,
            )
        };

        // 2. コンテキスト組み立て
        // 履歴から圧縮済みメッセージを除外
        let filtered_history = Self::filter_compressed_history(&history, &memories, session_id);

        // 履歴末尾のuserメッセージを除外（build_contextが末尾にuser_contentを追加するため）
        let history_without_last_user: Vec<_> = {
            let mut h = filtered_history;
            if let Some(last) = h.last() {
                if last.role == ChatRole::User {
                    h.pop();
                }
            }
            h
        };

        // TTS有効判定: グローバル設定 AND キャラクター個別TTS設定あり
        let tts_enabled = self.config_manager.get_config().tts.enabled && tts_config.is_some();

        // TTS有効時はSystem Promptに出力フォーマットルールを付加
        let effective_system_prompt = if tts_enabled {
            format!("{}\n\n## 出力フォーマットルール（必ず守ること）\n応答は必ず以下のJSON形式で返してください。JSON以外のテキストは含めないでください。\n```\n{{\"display\": \"表示用テキスト（地の文・動作描写・効果音を含む全文）\", \"speech\": \"声に出して話すセリフと心の声のみ（動作描写・効果音・擬音・ナレーションは含めない）\"}}\n```\n重要: speechには実際に口から発する言葉と心の中で思っていることだけを入れてください。\n- 含める: セリフ、呼びかけ、返事、質問、心の声、独り言\n- 含めない: *動作描写*, 効果音, 擬音語, ナレーション, 状況説明\n例:\n```\n{{\"display\": \"*嬉しそうに手を振りながら* おはよう！今日も一緒に遊ぼうね！ *ぴょんぴょん跳ねる*\", \"speech\": \"おはよう！今日も一緒に遊ぼうね！\"}}\n```", system_prompt)
        } else {
            system_prompt.clone()
        };

        let llm_messages = self.build_context(
            &effective_system_prompt,
            &memories,
            &thoughts,
            &history_without_last_user,
            content,
            attachment_text.as_deref(),
            attachment_images,
        );

        let session_id_owned = session_id.to_string();

        if tts_enabled {
            // === TTS有効パス: ストリーミングチャンクをフロントに送らず内部蓄積 ===
            let accumulator = partial_content_accumulator.clone();
            let callback = Box::new(move |chunk: String| {
                // 部分コンテンツ蓄積のみ（フロントへのchat:streamイベントは発行しない）
                if let Some(ref acc) = accumulator {
                    if let Ok(mut content) = acc.lock() {
                        content.push_str(&chunk);
                    }
                }
            });

            let _llm_guard = self.llm_lock.lock().await;
            let response = self
                .llm_client
                .chat_stream(&llm_messages, &self.current_llm_config(), None, callback)
                .await?
                .into_text();
            drop(_llm_guard);

            // LLM応答をJSONパース: {"display": "...", "speech": "..."}
            let (display_text, speech_text) = Self::parse_tts_response(&response);
            println!(
                "[TTS] Parsed - display: {} chars, speech: {} chars",
                display_text.len(),
                speech_text.len()
            );

            // tts:generating イベント発行
            println!("[TTS] LLM response complete, starting TTS generation...");
            app_handle
                .emit(
                    "tts:generating",
                    TTSGeneratingEvent {
                        session_id: session_id_owned.clone(),
                    },
                )
                .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

            // TTS Flow Controller で音声生成（speechテキストを使用）
            let char_tts_config = tts_config.as_ref().unwrap();
            let app_tts_config = self.config_manager.get_config().tts.clone();
            let voicepeak_path = app_tts_config.voicepeak_path.as_deref();
            let timeout_seconds = app_tts_config.timeout_seconds;
            println!(
                "[TTS] voicepeak_path: {:?}, timeout: {}s",
                voicepeak_path, timeout_seconds
            );
            println!("[TTS] provider: {:?}", char_tts_config.provider);

            if let Some(ref flow_controller) = self.tts_flow_controller {
                match flow_controller
                    .process(
                        &speech_text,
                        char_tts_config,
                        voicepeak_path,
                        timeout_seconds,
                    )
                    .await
                {
                    Ok(tts_result) => {
                        println!(
                            "[TTS] Success! Audio size: {} bytes",
                            tts_result.audio_data.len()
                        );
                        let audio_base64 = base64::engine::general_purpose::STANDARD
                            .encode(&tts_result.audio_data);
                        println!(
                            "[TTS] Emitting tts:complete, base64 length: {}",
                            audio_base64.len()
                        );
                        app_handle
                            .emit(
                                "tts:complete",
                                TTSCompleteEvent {
                                    session_id: session_id_owned.clone(),
                                    text: display_text.clone(),
                                    audio: audio_base64,
                                },
                            )
                            .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;
                        println!("[TTS] tts:complete emitted successfully");
                    }
                    Err(e) => {
                        println!("[TTS] Error: {}", e);
                        app_handle
                            .emit(
                                "tts:error",
                                TTSErrorEvent {
                                    session_id: session_id_owned.clone(),
                                    text: display_text.clone(),
                                    error: e.to_string(),
                                },
                            )
                            .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;
                    }
                }
            } else {
                println!("[TTS] Error: Flow Controller not initialized");
                app_handle
                    .emit(
                        "tts:error",
                        TTSErrorEvent {
                            session_id: session_id_owned.clone(),
                            text: display_text.clone(),
                            error: "TTS Flow Controller is not initialized".to_string(),
                        },
                    )
                    .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;
            }

            // アシスタントメッセージ保存（TTS時はdisplayテキスト）
            let assistant_msg_id = uuid::Uuid::new_v4().to_string();
            let assistant_now = chrono::Utc::now().to_rfc3339();

            let assistant_message = ChatMessageRecord {
                id: assistant_msg_id,
                session_id: session_id_owned.clone(),
                role: ChatRole::Assistant,
                content: display_text.clone(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: assistant_now.clone(),
            };

            {
                let db = self
                    .db
                    .lock()
                    .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
                let conn = db.connection();

                chat_repo::insert_message(conn, &assistant_message)?;

                let preview = truncate_str(&display_text, 50);
                chat_repo::update_session_metadata(conn, session_id, &assistant_now, &preview)?;
            }

            return Ok(());
        } else {
            // === TTS無効パス: ツール実行ループ付きストリーミングフロー ===
            const MAX_TOOL_ITERATIONS: usize = 10;

            // 有効なツール定義を取得（セッション単位の許可設定でフィルタ）
            let tool_definitions = {
                let global = self
                    .plugin_system
                    .as_ref()
                    .map(|ps| ps.get_enabled_tools())
                    .unwrap_or_default();
                self.filter_tools_by_session_permissions(&session_id_owned, global)
            };
            let tools_for_llm: Option<&[crate::models::ToolDefinition]> =
                if tool_definitions.is_empty() {
                    None
                } else {
                    Some(&tool_definitions)
                };

            // ツール実行ループ用のコンテキスト（可変）
            let mut loop_messages = llm_messages;
            let mut iteration = 0;

            loop {
                iteration += 1;
                if iteration > MAX_TOOL_ITERATIONS {
                    println!(
                        "[ToolLoop] Max iterations ({}) reached, stopping",
                        MAX_TOOL_ITERATIONS
                    );
                    break;
                }

                let app_handle_clone = app_handle.clone();
                let session_id_for_callback = session_id_owned.clone();
                let accumulator = partial_content_accumulator.clone();
                let callback = Box::new(move |chunk: String| {
                    // 部分コンテンツ蓄積
                    if let Some(ref acc) = accumulator {
                        if let Ok(mut content) = acc.lock() {
                            content.push_str(&chunk);
                        }
                    }
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
                let llm_response = self
                    .llm_client
                    .chat_stream(
                        &loop_messages,
                        &self.current_llm_config(),
                        tools_for_llm,
                        callback,
                    )
                    .await?;
                drop(_llm_guard);

                match llm_response {
                    LLMResponse::Text(text) => {
                        // テキスト応答 — ストリーミング完了イベントを送信してループ終了
                        app_handle
                            .emit(
                                "chat:stream",
                                ChatStreamEvent {
                                    session_id: session_id_owned.clone(),
                                    chunk: String::new(),
                                    done: true,
                                },
                            )
                            .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

                        // アシスタントメッセージ保存 & セッションメタデータ更新
                        let assistant_msg_id = uuid::Uuid::new_v4().to_string();
                        let assistant_now = chrono::Utc::now().to_rfc3339();

                        let assistant_message = ChatMessageRecord {
                            id: assistant_msg_id,
                            session_id: session_id_owned.clone(),
                            role: ChatRole::Assistant,
                            content: text.clone(),
                            attachments: None,
                            tool_calls: None,
                            tool_call_id: None,
                            created_at: assistant_now.clone(),
                        };

                        {
                            let db = self.db.lock().map_err(|e| {
                                AppError::Database(format!("Failed to acquire DB lock: {}", e))
                            })?;
                            let conn = db.connection();

                            chat_repo::insert_message(conn, &assistant_message)?;

                            let preview = truncate_str(&text, 50);
                            chat_repo::update_session_metadata(
                                conn,
                                session_id,
                                &assistant_now,
                                &preview,
                            )?;
                        }

                        return Ok(());
                    }
                    LLMResponse::ToolCalls(tool_calls) => {
                        // ツール呼び出し応答 — 実行してループ継続
                        println!(
                            "[ToolLoop] Iteration {}: {} tool call(s)",
                            iteration,
                            tool_calls.len()
                        );

                        // 1. tool:executing イベントをフロントエンドに送信
                        for tc in &tool_calls {
                            app_handle
                                .emit(
                                    "tool:executing",
                                    ToolExecutingEvent {
                                        session_id: session_id_owned.clone(),
                                        tool_name: tc.name.clone(),
                                    },
                                )
                                .map_err(|e| {
                                    AppError::Io(format!("Failed to emit event: {}", e))
                                })?;
                        }

                        // 2. アシスタントメッセージ（tool_calls含む）をDB保存
                        let tc_msg_id = uuid::Uuid::new_v4().to_string();
                        let tc_now = chrono::Utc::now().to_rfc3339();

                        let tc_assistant_message = ChatMessageRecord {
                            id: tc_msg_id,
                            session_id: session_id_owned.clone(),
                            role: ChatRole::Assistant,
                            content: String::new(),
                            attachments: None,
                            tool_calls: Some(tool_calls.clone()),
                            tool_call_id: None,
                            created_at: tc_now.clone(),
                        };

                        {
                            let db = self.db.lock().map_err(|e| {
                                AppError::Database(format!("Failed to acquire DB lock: {}", e))
                            })?;
                            let conn = db.connection();
                            chat_repo::insert_message(conn, &tc_assistant_message)?;
                        }

                        // コンテキストにアシスタントのtool_callメッセージを追加
                        // （tool_callsの内容をJSON文字列としてcontentに含める）
                        let tool_calls_json =
                            serde_json::to_string(&tool_calls).unwrap_or_default();
                        loop_messages.push(ChatMessage {
                            role: MessageRole::Assistant,
                            content: tool_calls_json,
                            tool_call_id: None,
                            images: None,
                        });

                        // 3. PluginSystem::handle_tool_calls でツール実行
                        let tool_results = if let Some(ref ps) = self.plugin_system {
                            ps.handle_tool_calls(&tool_calls).await?
                        } else {
                            // PluginSystem未設定の場合はエラー結果を返す
                            tool_calls
                                .iter()
                                .map(|tc| crate::models::plugin::ToolResult {
                                    tool_call_id: tc.id.clone(),
                                    content: "Plugin system is not available".to_string(),
                                    is_error: true,
                                })
                                .collect()
                        };

                        // 4. ツール実行結果をDB保存 & コンテキストに追加
                        for result in &tool_results {
                            let tool_msg_id = uuid::Uuid::new_v4().to_string();
                            let tool_now = chrono::Utc::now().to_rfc3339();

                            let tool_message = ChatMessageRecord {
                                id: tool_msg_id,
                                session_id: session_id_owned.clone(),
                                role: ChatRole::Tool,
                                content: result.content.clone(),
                                attachments: None,
                                tool_calls: None,
                                tool_call_id: Some(result.tool_call_id.clone()),
                                created_at: tool_now,
                            };

                            {
                                let db = self.db.lock().map_err(|e| {
                                    AppError::Database(format!("Failed to acquire DB lock: {}", e))
                                })?;
                                let conn = db.connection();
                                chat_repo::insert_message(conn, &tool_message)?;
                            }

                            // コンテキストにツール結果を追加
                            loop_messages.push(ChatMessage {
                                role: MessageRole::Tool,
                                content: result.content.clone(),
                                tool_call_id: Some(result.tool_call_id.clone()),
                                images: None,
                            });
                        }

                        // ループ継続 — 再度LLMを呼び出す
                    }
                }
            }

            // MAX_TOOL_ITERATIONS到達時のフォールバック: ストリーミング完了を通知
            app_handle
                .emit(
                    "chat:stream",
                    ChatStreamEvent {
                        session_id: session_id_owned.clone(),
                        chunk: String::new(),
                        done: true,
                    },
                )
                .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

            // 最大反復到達時はエラーメッセージをアシスタントとして保存
            let fallback_content = "[Tool execution limit reached. Please try again.]".to_string();
            let assistant_msg_id = uuid::Uuid::new_v4().to_string();
            let assistant_now = chrono::Utc::now().to_rfc3339();

            let assistant_message = ChatMessageRecord {
                id: assistant_msg_id,
                session_id: session_id_owned.clone(),
                role: ChatRole::Assistant,
                content: fallback_content.clone(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: assistant_now.clone(),
            };

            {
                let db = self
                    .db
                    .lock()
                    .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
                let conn = db.connection();

                chat_repo::insert_message(conn, &assistant_message)?;

                let preview = truncate_str(&fallback_content, 50);
                chat_repo::update_session_metadata(conn, session_id, &assistant_now, &preview)?;
            }
        }

        Ok(())
    }

    async fn regenerate(
        &self,
        session_id: &str,
        target_message_id: &str,
        app_handle: &AppHandle,
        partial_content_accumulator: Option<Arc<Mutex<String>>>,
    ) -> Result<(), AppError> {
        // 1. 対象メッセージを取得し、直前のuserメッセージを特定
        let user_content = {
            let db = self
                .db
                .lock()
                .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
            let conn = db.connection();

            // メッセージ履歴取得
            let messages = chat_repo::get_messages(conn, session_id)?;

            // 対象メッセージの位置を特定
            let target_idx = messages
                .iter()
                .position(|m| m.id == target_message_id)
                .ok_or_else(|| {
                    AppError::NotFound(format!("Message not found: {}", target_message_id))
                })?;

            // 直前のuserメッセージを探す
            let preceding_user_msg = messages[..target_idx]
                .iter()
                .rev()
                .find(|m| m.role == ChatRole::User);

            let user_content = preceding_user_msg
                .ok_or_else(|| {
                    AppError::Validation(
                        "No preceding user message found for regeneration".to_string(),
                    )
                })?
                .content
                .clone();

            // 対象assistantメッセージをDBから削除
            chat_repo::delete_message(conn, target_message_id)?;

            user_content
        };

        // 2. 直前のuserメッセージのcontentで再送信（send_messageと同様のストリーミング処理）
        let (system_prompt, memories, thoughts, history, tts_config) = {
            let db = self
                .db
                .lock()
                .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
            let conn = db.connection();

            let session = chat_repo::get_session(conn, session_id)?
                .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

            let character =
                char_repo::get_character(conn, &session.character_id)?.ok_or_else(|| {
                    AppError::NotFound(format!("Character not found: {}", session.character_id))
                })?;

            let memories = mem_repo::list_memories(conn, &session.character_id)?;

            let threshold_minutes = self
                .config_manager
                .get_config()
                .thought
                .auto_delete_threshold_minutes;
            let thoughts = if threshold_minutes > 0 {
                let since =
                    chrono::Utc::now() - chrono::Duration::minutes(threshold_minutes as i64);
                let since_str = since.to_rfc3339();
                thought_repo::get_recent_thoughts(conn, &session.character_id, &since_str)?
            } else {
                thought_repo::get_thoughts(conn, &session.character_id, None)?
            };

            let history = chat_repo::get_messages(conn, session_id)?;

            (
                character.system_prompt,
                memories,
                thoughts,
                history,
                character.tts_config,
            )
        };

        // 履歴から圧縮済みメッセージを除外
        let filtered_history = Self::filter_compressed_history(&history, &memories, session_id);

        // 履歴末尾のuserメッセージを除外（build_contextが末尾にuser_contentを追加するため）
        let history_without_last_user: Vec<_> = {
            let mut h = filtered_history;
            if let Some(last) = h.last() {
                if last.role == ChatRole::User {
                    h.pop();
                }
            }
            h
        };

        let llm_messages = self.build_context(
            &system_prompt,
            &memories,
            &thoughts,
            &history_without_last_user,
            &user_content,
            None,
            None,
        );

        // 3. TTS有効判定
        let tts_enabled = self.config_manager.get_config().tts.enabled && tts_config.is_some();

        let session_id_owned = session_id.to_string();

        if tts_enabled {
            // === TTS有効パス: ストリーミングチャンクをフロントに送らず内部蓄積 ===
            let accumulator = partial_content_accumulator.clone();
            let callback = Box::new(move |chunk: String| {
                if let Some(ref acc) = accumulator {
                    if let Ok(mut content) = acc.lock() {
                        content.push_str(&chunk);
                    }
                }
            });

            let _llm_guard = self.llm_lock.lock().await;
            let response = self
                .llm_client
                .chat_stream(&llm_messages, &self.current_llm_config(), None, callback)
                .await?
                .into_text();
            drop(_llm_guard);

            // tts:generating イベント発行
            app_handle
                .emit(
                    "tts:generating",
                    TTSGeneratingEvent {
                        session_id: session_id_owned.clone(),
                    },
                )
                .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

            // TTS Flow Controller で音声生成
            let char_tts_config = tts_config.as_ref().unwrap();
            let app_tts_config = self.config_manager.get_config().tts.clone();
            let voicepeak_path = app_tts_config.voicepeak_path.as_deref();
            let timeout_seconds = app_tts_config.timeout_seconds;

            if let Some(ref flow_controller) = self.tts_flow_controller {
                match flow_controller
                    .process(&response, char_tts_config, voicepeak_path, timeout_seconds)
                    .await
                {
                    Ok(tts_result) => {
                        let audio_base64 = base64::engine::general_purpose::STANDARD
                            .encode(&tts_result.audio_data);
                        app_handle
                            .emit(
                                "tts:complete",
                                TTSCompleteEvent {
                                    session_id: session_id_owned.clone(),
                                    text: response.clone(),
                                    audio: audio_base64,
                                },
                            )
                            .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;
                    }
                    Err(e) => {
                        app_handle
                            .emit(
                                "tts:error",
                                TTSErrorEvent {
                                    session_id: session_id_owned.clone(),
                                    text: response.clone(),
                                    error: e.to_string(),
                                },
                            )
                            .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;
                    }
                }
            } else {
                app_handle
                    .emit(
                        "tts:error",
                        TTSErrorEvent {
                            session_id: session_id_owned.clone(),
                            text: response.clone(),
                            error: "TTS Flow Controller is not initialized".to_string(),
                        },
                    )
                    .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;
            }

            // アシスタントメッセージ保存（TTS時）
            let assistant_msg_id = uuid::Uuid::new_v4().to_string();
            let assistant_now = chrono::Utc::now().to_rfc3339();

            let assistant_message = ChatMessageRecord {
                id: assistant_msg_id,
                session_id: session_id_owned.clone(),
                role: ChatRole::Assistant,
                content: response.clone(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: assistant_now.clone(),
            };

            {
                let db = self
                    .db
                    .lock()
                    .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
                let conn = db.connection();

                chat_repo::insert_message(conn, &assistant_message)?;

                let preview = truncate_str(&response, 50);
                chat_repo::update_session_metadata(conn, session_id, &assistant_now, &preview)?;
            }

            return Ok(());
        } else {
            // === TTS無効パス: ツール実行ループ付きストリーミングフロー ===
            const MAX_TOOL_ITERATIONS: usize = 10;

            // 有効なツール定義を取得（セッション単位の許可設定でフィルタ）
            let tool_definitions = {
                let global = self
                    .plugin_system
                    .as_ref()
                    .map(|ps| ps.get_enabled_tools())
                    .unwrap_or_default();
                self.filter_tools_by_session_permissions(&session_id_owned, global)
            };
            let tools_for_llm: Option<&[crate::models::ToolDefinition]> =
                if tool_definitions.is_empty() {
                    None
                } else {
                    Some(&tool_definitions)
                };

            // ツール実行ループ用のコンテキスト（可変）
            let mut loop_messages = llm_messages;
            let mut iteration = 0;

            loop {
                iteration += 1;
                if iteration > MAX_TOOL_ITERATIONS {
                    println!(
                        "[ToolLoop] Max iterations ({}) reached, stopping",
                        MAX_TOOL_ITERATIONS
                    );
                    break;
                }

                let app_handle_clone = app_handle.clone();
                let session_id_for_callback = session_id_owned.clone();
                let accumulator = partial_content_accumulator.clone();
                let callback = Box::new(move |chunk: String| {
                    // 部分コンテンツ蓄積
                    if let Some(ref acc) = accumulator {
                        if let Ok(mut content) = acc.lock() {
                            content.push_str(&chunk);
                        }
                    }
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
                let llm_response = self
                    .llm_client
                    .chat_stream(
                        &loop_messages,
                        &self.current_llm_config(),
                        tools_for_llm,
                        callback,
                    )
                    .await?;
                drop(_llm_guard);

                match llm_response {
                    LLMResponse::Text(text) => {
                        // テキスト応答 — ストリーミング完了イベントを送信してループ終了
                        app_handle
                            .emit(
                                "chat:stream",
                                ChatStreamEvent {
                                    session_id: session_id_owned.clone(),
                                    chunk: String::new(),
                                    done: true,
                                },
                            )
                            .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

                        // アシスタントメッセージ保存 & セッションメタデータ更新
                        let assistant_msg_id = uuid::Uuid::new_v4().to_string();
                        let assistant_now = chrono::Utc::now().to_rfc3339();

                        let assistant_message = ChatMessageRecord {
                            id: assistant_msg_id,
                            session_id: session_id_owned.clone(),
                            role: ChatRole::Assistant,
                            content: text.clone(),
                            attachments: None,
                            tool_calls: None,
                            tool_call_id: None,
                            created_at: assistant_now.clone(),
                        };

                        {
                            let db = self.db.lock().map_err(|e| {
                                AppError::Database(format!("Failed to acquire DB lock: {}", e))
                            })?;
                            let conn = db.connection();

                            chat_repo::insert_message(conn, &assistant_message)?;

                            let preview = truncate_str(&text, 50);
                            chat_repo::update_session_metadata(
                                conn,
                                session_id,
                                &assistant_now,
                                &preview,
                            )?;
                        }

                        return Ok(());
                    }
                    LLMResponse::ToolCalls(tool_calls) => {
                        // ツール呼び出し応答 — 実行してループ継続
                        println!(
                            "[ToolLoop] regenerate iteration {}: {} tool call(s)",
                            iteration,
                            tool_calls.len()
                        );

                        // 1. tool:executing イベントをフロントエンドに送信
                        for tc in &tool_calls {
                            app_handle
                                .emit(
                                    "tool:executing",
                                    ToolExecutingEvent {
                                        session_id: session_id_owned.clone(),
                                        tool_name: tc.name.clone(),
                                    },
                                )
                                .map_err(|e| {
                                    AppError::Io(format!("Failed to emit event: {}", e))
                                })?;
                        }

                        // 2. アシスタントメッセージ（tool_calls含む）をDB保存
                        let tc_msg_id = uuid::Uuid::new_v4().to_string();
                        let tc_now = chrono::Utc::now().to_rfc3339();

                        let tc_assistant_message = ChatMessageRecord {
                            id: tc_msg_id,
                            session_id: session_id_owned.clone(),
                            role: ChatRole::Assistant,
                            content: String::new(),
                            attachments: None,
                            tool_calls: Some(tool_calls.clone()),
                            tool_call_id: None,
                            created_at: tc_now.clone(),
                        };

                        {
                            let db = self.db.lock().map_err(|e| {
                                AppError::Database(format!("Failed to acquire DB lock: {}", e))
                            })?;
                            let conn = db.connection();
                            chat_repo::insert_message(conn, &tc_assistant_message)?;
                        }

                        // コンテキストにアシスタントのtool_callメッセージを追加
                        let tool_calls_json =
                            serde_json::to_string(&tool_calls).unwrap_or_default();
                        loop_messages.push(ChatMessage {
                            role: MessageRole::Assistant,
                            content: tool_calls_json,
                            tool_call_id: None,
                            images: None,
                        });

                        // 3. PluginSystem::handle_tool_calls でツール実行
                        let tool_results = if let Some(ref ps) = self.plugin_system {
                            ps.handle_tool_calls(&tool_calls).await?
                        } else {
                            tool_calls
                                .iter()
                                .map(|tc| crate::models::plugin::ToolResult {
                                    tool_call_id: tc.id.clone(),
                                    content: "Plugin system is not available".to_string(),
                                    is_error: true,
                                })
                                .collect()
                        };

                        // 4. ツール実行結果をDB保存 & コンテキストに追加
                        for result in &tool_results {
                            let tool_msg_id = uuid::Uuid::new_v4().to_string();
                            let tool_now = chrono::Utc::now().to_rfc3339();

                            let tool_message = ChatMessageRecord {
                                id: tool_msg_id,
                                session_id: session_id_owned.clone(),
                                role: ChatRole::Tool,
                                content: result.content.clone(),
                                attachments: None,
                                tool_calls: None,
                                tool_call_id: Some(result.tool_call_id.clone()),
                                created_at: tool_now,
                            };

                            {
                                let db = self.db.lock().map_err(|e| {
                                    AppError::Database(format!("Failed to acquire DB lock: {}", e))
                                })?;
                                let conn = db.connection();
                                chat_repo::insert_message(conn, &tool_message)?;
                            }

                            // コンテキストにツール結果を追加
                            loop_messages.push(ChatMessage {
                                role: MessageRole::Tool,
                                content: result.content.clone(),
                                tool_call_id: Some(result.tool_call_id.clone()),
                                images: None,
                            });
                        }

                        // ループ継続 — 再度LLMを呼び出す
                    }
                }
            }

            // MAX_TOOL_ITERATIONS到達時のフォールバック: ストリーミング完了を通知
            app_handle
                .emit(
                    "chat:stream",
                    ChatStreamEvent {
                        session_id: session_id_owned.clone(),
                        chunk: String::new(),
                        done: true,
                    },
                )
                .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

            // 最大反復到達時はエラーメッセージをアシスタントとして保存
            let fallback_content = "[Tool execution limit reached. Please try again.]".to_string();
            let assistant_msg_id = uuid::Uuid::new_v4().to_string();
            let assistant_now = chrono::Utc::now().to_rfc3339();

            let assistant_message = ChatMessageRecord {
                id: assistant_msg_id,
                session_id: session_id_owned.clone(),
                role: ChatRole::Assistant,
                content: fallback_content.clone(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: assistant_now.clone(),
            };

            {
                let db = self
                    .db
                    .lock()
                    .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
                let conn = db.connection();

                chat_repo::insert_message(conn, &assistant_message)?;

                let preview = truncate_str(&fallback_content, 50);
                chat_repo::update_session_metadata(conn, session_id, &assistant_now, &preview)?;
            }
        }

        Ok(())
    }

    async fn get_history(&self, session_id: &str) -> Result<Vec<ChatMessageRecord>, AppError> {
        let db = self
            .db
            .lock()
            .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
        chat_repo::get_messages(db.connection(), session_id)
    }

    async fn list_sessions(&self, character_id: &str) -> Result<Vec<ChatSession>, AppError> {
        let db = self
            .db
            .lock()
            .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
        chat_repo::list_sessions(db.connection(), character_id)
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), AppError> {
        let db = self
            .db
            .lock()
            .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
        chat_repo::delete_session(db.connection(), session_id)
    }

    async fn edit_and_resend(
        &self,
        session_id: &str,
        message_id: &str,
        new_content: &str,
        app_handle: &AppHandle,
        partial_content_accumulator: Option<Arc<Mutex<String>>>,
    ) -> Result<(), AppError> {
        // 1. 対象メッセージの検証、後続メッセージ削除、内容更新
        {
            let db = self
                .db
                .lock()
                .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
            let conn = db.connection();

            // 対象メッセージを取得して role=User であることを確認
            let messages = chat_repo::get_messages(conn, session_id)?;
            let target_msg = messages
                .iter()
                .find(|m| m.id == message_id)
                .ok_or_else(|| AppError::NotFound(format!("Message not found: {}", message_id)))?;

            if target_msg.role != ChatRole::User {
                return Err(AppError::Validation(
                    "Only user messages can be edited".to_string(),
                ));
            }

            // 対象メッセージ以降の全メッセージを削除
            chat_repo::delete_messages_after(conn, session_id, message_id)?;

            // 対象メッセージの content を更新
            chat_repo::update_message_content(conn, message_id, new_content)?;
        }

        // 2. 更新後のコンテキストを組み立てて再送信
        let (system_prompt, memories, thoughts, history, tts_config) = {
            let db = self
                .db
                .lock()
                .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
            let conn = db.connection();

            let session = chat_repo::get_session(conn, session_id)?
                .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

            let character =
                char_repo::get_character(conn, &session.character_id)?.ok_or_else(|| {
                    AppError::NotFound(format!("Character not found: {}", session.character_id))
                })?;

            let memories = mem_repo::list_memories(conn, &session.character_id)?;

            let threshold_minutes = self
                .config_manager
                .get_config()
                .thought
                .auto_delete_threshold_minutes;
            let thoughts = if threshold_minutes > 0 {
                let since =
                    chrono::Utc::now() - chrono::Duration::minutes(threshold_minutes as i64);
                let since_str = since.to_rfc3339();
                thought_repo::get_recent_thoughts(conn, &session.character_id, &since_str)?
            } else {
                thought_repo::get_thoughts(conn, &session.character_id, None)?
            };

            let history = chat_repo::get_messages(conn, session_id)?;

            (
                character.system_prompt,
                memories,
                thoughts,
                history,
                character.tts_config,
            )
        };

        // 履歴から圧縮済みメッセージを除外
        let filtered_history = Self::filter_compressed_history(&history, &memories, session_id);

        // 履歴末尾のuserメッセージ（編集済み）を除外し、user_contentとして渡す
        let history_without_last_user: Vec<_> = {
            let mut h = filtered_history;
            if let Some(last) = h.last() {
                if last.role == ChatRole::User {
                    h.pop();
                }
            }
            h
        };

        let llm_messages = self.build_context(
            &system_prompt,
            &memories,
            &thoughts,
            &history_without_last_user,
            new_content,
            None,
            None,
        );

        // 3. TTS有効判定
        let tts_enabled = self.config_manager.get_config().tts.enabled && tts_config.is_some();

        let session_id_owned = session_id.to_string();

        if tts_enabled {
            // === TTS有効パス ===
            let accumulator = partial_content_accumulator.clone();
            let callback = Box::new(move |chunk: String| {
                if let Some(ref acc) = accumulator {
                    if let Ok(mut content) = acc.lock() {
                        content.push_str(&chunk);
                    }
                }
            });

            let _llm_guard = self.llm_lock.lock().await;
            let response = self
                .llm_client
                .chat_stream(&llm_messages, &self.current_llm_config(), None, callback)
                .await?
                .into_text();
            drop(_llm_guard);

            app_handle
                .emit(
                    "tts:generating",
                    TTSGeneratingEvent {
                        session_id: session_id_owned.clone(),
                    },
                )
                .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

            let char_tts_config = tts_config.as_ref().unwrap();
            let app_tts_config = self.config_manager.get_config().tts.clone();
            let voicepeak_path = app_tts_config.voicepeak_path.as_deref();
            let timeout_seconds = app_tts_config.timeout_seconds;

            if let Some(ref flow_controller) = self.tts_flow_controller {
                match flow_controller
                    .process(&response, char_tts_config, voicepeak_path, timeout_seconds)
                    .await
                {
                    Ok(tts_result) => {
                        let audio_base64 = base64::engine::general_purpose::STANDARD
                            .encode(&tts_result.audio_data);
                        app_handle
                            .emit(
                                "tts:complete",
                                TTSCompleteEvent {
                                    session_id: session_id_owned.clone(),
                                    text: response.clone(),
                                    audio: audio_base64,
                                },
                            )
                            .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;
                    }
                    Err(e) => {
                        app_handle
                            .emit(
                                "tts:error",
                                TTSErrorEvent {
                                    session_id: session_id_owned.clone(),
                                    text: response.clone(),
                                    error: e.to_string(),
                                },
                            )
                            .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;
                    }
                }
            } else {
                app_handle
                    .emit(
                        "tts:error",
                        TTSErrorEvent {
                            session_id: session_id_owned.clone(),
                            text: response.clone(),
                            error: "TTS Flow Controller is not initialized".to_string(),
                        },
                    )
                    .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;
            }

            // アシスタントメッセージ保存（TTS時）
            let assistant_msg_id = uuid::Uuid::new_v4().to_string();
            let assistant_now = chrono::Utc::now().to_rfc3339();

            let assistant_message = ChatMessageRecord {
                id: assistant_msg_id,
                session_id: session_id_owned.clone(),
                role: ChatRole::Assistant,
                content: response.clone(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: assistant_now.clone(),
            };

            {
                let db = self
                    .db
                    .lock()
                    .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
                let conn = db.connection();

                chat_repo::insert_message(conn, &assistant_message)?;

                let preview = truncate_str(&response, 50);
                chat_repo::update_session_metadata(conn, session_id, &assistant_now, &preview)?;
            }

            return Ok(());
        } else {
            // === TTS無効パス: ツール実行ループ付きストリーミングフロー ===
            const MAX_TOOL_ITERATIONS: usize = 10;

            // 有効なツール定義を取得（セッション単位の許可設定でフィルタ）
            let tool_definitions = {
                let global = self
                    .plugin_system
                    .as_ref()
                    .map(|ps| ps.get_enabled_tools())
                    .unwrap_or_default();
                self.filter_tools_by_session_permissions(&session_id_owned, global)
            };
            let tools_for_llm: Option<&[crate::models::ToolDefinition]> =
                if tool_definitions.is_empty() {
                    None
                } else {
                    Some(&tool_definitions)
                };

            // ツール実行ループ用のコンテキスト（可変）
            let mut loop_messages = llm_messages;
            let mut iteration = 0;

            loop {
                iteration += 1;
                if iteration > MAX_TOOL_ITERATIONS {
                    println!(
                        "[ToolLoop] Max iterations ({}) reached, stopping",
                        MAX_TOOL_ITERATIONS
                    );
                    break;
                }

                let app_handle_clone = app_handle.clone();
                let session_id_for_callback = session_id_owned.clone();
                let accumulator = partial_content_accumulator.clone();
                let callback = Box::new(move |chunk: String| {
                    // 部分コンテンツ蓄積
                    if let Some(ref acc) = accumulator {
                        if let Ok(mut content) = acc.lock() {
                            content.push_str(&chunk);
                        }
                    }
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
                let llm_response = self
                    .llm_client
                    .chat_stream(
                        &loop_messages,
                        &self.current_llm_config(),
                        tools_for_llm,
                        callback,
                    )
                    .await?;
                drop(_llm_guard);

                match llm_response {
                    LLMResponse::Text(text) => {
                        // テキスト応答 — ストリーミング完了イベントを送信してループ終了
                        app_handle
                            .emit(
                                "chat:stream",
                                ChatStreamEvent {
                                    session_id: session_id_owned.clone(),
                                    chunk: String::new(),
                                    done: true,
                                },
                            )
                            .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

                        // アシスタントメッセージ保存 & セッションメタデータ更新
                        let assistant_msg_id = uuid::Uuid::new_v4().to_string();
                        let assistant_now = chrono::Utc::now().to_rfc3339();

                        let assistant_message = ChatMessageRecord {
                            id: assistant_msg_id,
                            session_id: session_id_owned.clone(),
                            role: ChatRole::Assistant,
                            content: text.clone(),
                            attachments: None,
                            tool_calls: None,
                            tool_call_id: None,
                            created_at: assistant_now.clone(),
                        };

                        {
                            let db = self.db.lock().map_err(|e| {
                                AppError::Database(format!("Failed to acquire DB lock: {}", e))
                            })?;
                            let conn = db.connection();

                            chat_repo::insert_message(conn, &assistant_message)?;

                            let preview = truncate_str(&text, 50);
                            chat_repo::update_session_metadata(
                                conn,
                                session_id,
                                &assistant_now,
                                &preview,
                            )?;
                        }

                        return Ok(());
                    }
                    LLMResponse::ToolCalls(tool_calls) => {
                        // ツール呼び出し応答 — 実行してループ継続
                        println!(
                            "[ToolLoop] edit_and_resend iteration {}: {} tool call(s)",
                            iteration,
                            tool_calls.len()
                        );

                        // 1. tool:executing イベントをフロントエンドに送信
                        for tc in &tool_calls {
                            app_handle
                                .emit(
                                    "tool:executing",
                                    ToolExecutingEvent {
                                        session_id: session_id_owned.clone(),
                                        tool_name: tc.name.clone(),
                                    },
                                )
                                .map_err(|e| {
                                    AppError::Io(format!("Failed to emit event: {}", e))
                                })?;
                        }

                        // 2. アシスタントメッセージ（tool_calls含む）をDB保存
                        let tc_msg_id = uuid::Uuid::new_v4().to_string();
                        let tc_now = chrono::Utc::now().to_rfc3339();

                        let tc_assistant_message = ChatMessageRecord {
                            id: tc_msg_id,
                            session_id: session_id_owned.clone(),
                            role: ChatRole::Assistant,
                            content: String::new(),
                            attachments: None,
                            tool_calls: Some(tool_calls.clone()),
                            tool_call_id: None,
                            created_at: tc_now.clone(),
                        };

                        {
                            let db = self.db.lock().map_err(|e| {
                                AppError::Database(format!("Failed to acquire DB lock: {}", e))
                            })?;
                            let conn = db.connection();
                            chat_repo::insert_message(conn, &tc_assistant_message)?;
                        }

                        // コンテキストにアシスタントのtool_callメッセージを追加
                        let tool_calls_json =
                            serde_json::to_string(&tool_calls).unwrap_or_default();
                        loop_messages.push(ChatMessage {
                            role: MessageRole::Assistant,
                            content: tool_calls_json,
                            tool_call_id: None,
                            images: None,
                        });

                        // 3. PluginSystem::handle_tool_calls でツール実行
                        let tool_results = if let Some(ref ps) = self.plugin_system {
                            ps.handle_tool_calls(&tool_calls).await?
                        } else {
                            tool_calls
                                .iter()
                                .map(|tc| crate::models::plugin::ToolResult {
                                    tool_call_id: tc.id.clone(),
                                    content: "Plugin system is not available".to_string(),
                                    is_error: true,
                                })
                                .collect()
                        };

                        // 4. ツール実行結果をDB保存 & コンテキストに追加
                        for result in &tool_results {
                            let tool_msg_id = uuid::Uuid::new_v4().to_string();
                            let tool_now = chrono::Utc::now().to_rfc3339();

                            let tool_message = ChatMessageRecord {
                                id: tool_msg_id,
                                session_id: session_id_owned.clone(),
                                role: ChatRole::Tool,
                                content: result.content.clone(),
                                attachments: None,
                                tool_calls: None,
                                tool_call_id: Some(result.tool_call_id.clone()),
                                created_at: tool_now,
                            };

                            {
                                let db = self.db.lock().map_err(|e| {
                                    AppError::Database(format!("Failed to acquire DB lock: {}", e))
                                })?;
                                let conn = db.connection();
                                chat_repo::insert_message(conn, &tool_message)?;
                            }

                            // コンテキストにツール結果を追加
                            loop_messages.push(ChatMessage {
                                role: MessageRole::Tool,
                                content: result.content.clone(),
                                tool_call_id: Some(result.tool_call_id.clone()),
                                images: None,
                            });
                        }

                        // ループ継続 — 再度LLMを呼び出す
                    }
                }
            }

            // MAX_TOOL_ITERATIONS到達時のフォールバック: ストリーミング完了を通知
            app_handle
                .emit(
                    "chat:stream",
                    ChatStreamEvent {
                        session_id: session_id_owned.clone(),
                        chunk: String::new(),
                        done: true,
                    },
                )
                .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

            // 最大反復到達時はエラーメッセージをアシスタントとして保存
            let fallback_content = "[Tool execution limit reached. Please try again.]".to_string();
            let assistant_msg_id = uuid::Uuid::new_v4().to_string();
            let assistant_now = chrono::Utc::now().to_rfc3339();

            let assistant_message = ChatMessageRecord {
                id: assistant_msg_id,
                session_id: session_id_owned.clone(),
                role: ChatRole::Assistant,
                content: fallback_content.clone(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: assistant_now.clone(),
            };

            {
                let db = self
                    .db
                    .lock()
                    .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
                let conn = db.connection();

                chat_repo::insert_message(conn, &assistant_message)?;

                let preview = truncate_str(&fallback_content, 50);
                chat_repo::update_session_metadata(conn, session_id, &assistant_now, &preview)?;
            }
        }

        Ok(())
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
        let attachment_images = attachments
            .as_ref()
            .and_then(|a| Self::extract_attachment_images(a));
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
            let db = self
                .db
                .lock()
                .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
            let conn = db.connection();

            chat_repo::insert_message(conn, &user_message)?;

            let session = chat_repo::get_session(conn, session_id)?
                .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

            let character =
                char_repo::get_character(conn, &session.character_id)?.ok_or_else(|| {
                    AppError::NotFound(format!("Character not found: {}", session.character_id))
                })?;

            let memories = mem_repo::list_memories(conn, &session.character_id)?;

            // 閾値内の最近の思考を取得
            let threshold_minutes = self
                .config_manager
                .get_config()
                .thought
                .auto_delete_threshold_minutes;
            let thoughts = if threshold_minutes > 0 {
                let since =
                    chrono::Utc::now() - chrono::Duration::minutes(threshold_minutes as i64);
                let since_str = since.to_rfc3339();
                thought_repo::get_recent_thoughts(conn, &session.character_id, &since_str)?
            } else {
                thought_repo::get_thoughts(conn, &session.character_id, None)?
            };

            let history = chat_repo::get_messages(conn, session_id)?;

            (character.system_prompt, memories, thoughts, history)
        };

        // 履歴から圧縮済みメッセージを除外
        let filtered_history = Self::filter_compressed_history(&history, &memories, session_id);

        // 履歴末尾のuserメッセージを除外（build_contextが末尾にuser_contentを追加するため）
        let history_without_last_user: Vec<_> = {
            let mut h = filtered_history;
            if let Some(last) = h.last() {
                if last.role == ChatRole::User {
                    h.pop();
                }
            }
            h
        };

        let llm_messages = self.build_context(
            &system_prompt,
            &memories,
            &thoughts,
            &history_without_last_user,
            content,
            attachment_text.as_deref(),
            attachment_images,
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
                app_handle
                    .emit(
                        "chat:stream",
                        ChatStreamEvent {
                            session_id: session_id.to_string(),
                            chunk: text.clone(),
                            done: false,
                        },
                    )
                    .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

                app_handle
                    .emit(
                        "chat:stream",
                        ChatStreamEvent {
                            session_id: session_id.to_string(),
                            chunk: String::new(),
                            done: true,
                        },
                    )
                    .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;

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

                let db = self
                    .db
                    .lock()
                    .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
                let conn = db.connection();

                chat_repo::insert_message(conn, &assistant_message)?;

                let preview = truncate_str(&text, 50);
                chat_repo::update_session_metadata(conn, session_id, &assistant_now, &preview)?;
            }
            LLMResponse::ToolCalls(tool_calls) => {
                // tool_callレスポンス — イベント発火してDB保存
                for tc in &tool_calls {
                    app_handle
                        .emit(
                            "tool:executing",
                            ToolExecutingEvent {
                                session_id: session_id.to_string(),
                                tool_name: tc.name.clone(),
                            },
                        )
                        .map_err(|e| AppError::Io(format!("Failed to emit event: {}", e)))?;
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

                let db = self
                    .db
                    .lock()
                    .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
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

/// テスト専用: AppHandle不要のsend_message（イベント発行をスキップ）
#[cfg(test)]
impl DefaultChatEngine {
    pub async fn send_message_for_test(
        &self,
        session_id: &str,
        content: &str,
        attachments: Option<Vec<Attachment>>,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().to_rfc3339();
        let user_msg_id = uuid::Uuid::new_v4().to_string();

        // 添付ファイル処理
        let attachment_text = attachments
            .as_ref()
            .and_then(|a| Self::extract_attachment_text(a));
        let attachment_images = attachments
            .as_ref()
            .and_then(|a| Self::extract_attachment_images(a));
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

        // DB操作
        let (system_prompt, memories, thoughts, history) = {
            let db = self
                .db
                .lock()
                .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
            let conn = db.connection();

            chat_repo::insert_message(conn, &user_message)?;

            let session = chat_repo::get_session(conn, session_id)?
                .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

            let character =
                char_repo::get_character(conn, &session.character_id)?.ok_or_else(|| {
                    AppError::NotFound(format!("Character not found: {}", session.character_id))
                })?;

            let memories = mem_repo::list_memories(conn, &session.character_id)?;

            let threshold_minutes = self
                .config_manager
                .get_config()
                .thought
                .auto_delete_threshold_minutes;
            let thoughts = if threshold_minutes > 0 {
                let since =
                    chrono::Utc::now() - chrono::Duration::minutes(threshold_minutes as i64);
                let since_str = since.to_rfc3339();
                thought_repo::get_recent_thoughts(conn, &session.character_id, &since_str)?
            } else {
                thought_repo::get_thoughts(conn, &session.character_id, None)?
            };

            let history = chat_repo::get_messages(conn, session_id)?;

            (character.system_prompt, memories, thoughts, history)
        };

        // 2. コンテキスト組み立て
        let filtered_history = Self::filter_compressed_history(&history, &memories, session_id);
        let history_without_last_user: Vec<_> = {
            let mut h = filtered_history;
            if let Some(last) = h.last() {
                if last.role == ChatRole::User {
                    h.pop();
                }
            }
            h
        };

        let llm_messages = self.build_context(
            &system_prompt,
            &memories,
            &thoughts,
            &history_without_last_user,
            content,
            attachment_text.as_deref(),
            attachment_images,
        );

        let session_id_owned = session_id.to_string();

        // === ツール実行ループ（TTS無効パスと同等） ===
        const MAX_TOOL_ITERATIONS: usize = 10;

        let tool_definitions = {
            let global = self
                .plugin_system
                .as_ref()
                .map(|ps| ps.get_enabled_tools())
                .unwrap_or_default();
            self.filter_tools_by_session_permissions(&session_id_owned, global)
        };
        let tools_for_llm: Option<&[crate::models::ToolDefinition]> = if tool_definitions.is_empty()
        {
            None
        } else {
            Some(&tool_definitions)
        };

        let mut loop_messages = llm_messages;
        let mut iteration = 0;

        loop {
            iteration += 1;
            if iteration > MAX_TOOL_ITERATIONS {
                break;
            }

            let callback: Box<dyn Fn(String) + Send> = Box::new(|_chunk: String| {
                // テスト用: イベント発行なし
            });

            let _llm_guard = self.llm_lock.lock().await;
            let llm_response = self
                .llm_client
                .chat_stream(
                    &loop_messages,
                    &self.current_llm_config(),
                    tools_for_llm,
                    callback,
                )
                .await?;
            drop(_llm_guard);

            match llm_response {
                LLMResponse::Text(text) => {
                    // テキスト応答 — ループ終了
                    let assistant_msg_id = uuid::Uuid::new_v4().to_string();
                    let assistant_now = chrono::Utc::now().to_rfc3339();

                    let assistant_message = ChatMessageRecord {
                        id: assistant_msg_id,
                        session_id: session_id_owned.clone(),
                        role: ChatRole::Assistant,
                        content: text.clone(),
                        attachments: None,
                        tool_calls: None,
                        tool_call_id: None,
                        created_at: assistant_now.clone(),
                    };

                    {
                        let db = self.db.lock().map_err(|e| {
                            AppError::Database(format!("Failed to acquire DB lock: {}", e))
                        })?;
                        let conn = db.connection();
                        chat_repo::insert_message(conn, &assistant_message)?;
                        let preview = truncate_str(&text, 50);
                        chat_repo::update_session_metadata(
                            conn,
                            session_id,
                            &assistant_now,
                            &preview,
                        )?;
                    }

                    return Ok(());
                }
                LLMResponse::ToolCalls(tool_calls) => {
                    // ツール呼び出し応答
                    let tc_msg_id = uuid::Uuid::new_v4().to_string();
                    let tc_now = chrono::Utc::now().to_rfc3339();

                    let tc_assistant_message = ChatMessageRecord {
                        id: tc_msg_id,
                        session_id: session_id_owned.clone(),
                        role: ChatRole::Assistant,
                        content: String::new(),
                        attachments: None,
                        tool_calls: Some(tool_calls.clone()),
                        tool_call_id: None,
                        created_at: tc_now.clone(),
                    };

                    {
                        let db = self.db.lock().map_err(|e| {
                            AppError::Database(format!("Failed to acquire DB lock: {}", e))
                        })?;
                        let conn = db.connection();
                        chat_repo::insert_message(conn, &tc_assistant_message)?;
                    }

                    let tool_calls_json = serde_json::to_string(&tool_calls).unwrap_or_default();
                    loop_messages.push(ChatMessage {
                        role: MessageRole::Assistant,
                        content: tool_calls_json,
                        tool_call_id: None,
                        images: None,
                    });

                    // PluginSystem::handle_tool_calls でツール実行
                    let tool_results = if let Some(ref ps) = self.plugin_system {
                        ps.handle_tool_calls(&tool_calls).await?
                    } else {
                        tool_calls
                            .iter()
                            .map(|tc| crate::models::plugin::ToolResult {
                                tool_call_id: tc.id.clone(),
                                content: "Plugin system is not available".to_string(),
                                is_error: true,
                            })
                            .collect()
                    };

                    // ツール実行結果をDB保存 & コンテキストに追加
                    for result in &tool_results {
                        let tool_msg_id = uuid::Uuid::new_v4().to_string();
                        let tool_now = chrono::Utc::now().to_rfc3339();

                        let tool_message = ChatMessageRecord {
                            id: tool_msg_id,
                            session_id: session_id_owned.clone(),
                            role: ChatRole::Tool,
                            content: result.content.clone(),
                            attachments: None,
                            tool_calls: None,
                            tool_call_id: Some(result.tool_call_id.clone()),
                            created_at: tool_now,
                        };

                        {
                            let db = self.db.lock().map_err(|e| {
                                AppError::Database(format!("Failed to acquire DB lock: {}", e))
                            })?;
                            let conn = db.connection();
                            chat_repo::insert_message(conn, &tool_message)?;
                        }

                        loop_messages.push(ChatMessage {
                            role: MessageRole::Tool,
                            content: result.content.clone(),
                            tool_call_id: Some(result.tool_call_id.clone()),
                            images: None,
                        });
                    }
                }
            }
        }

        // MAX_TOOL_ITERATIONS到達時のフォールバック
        let fallback_content = "[Tool execution limit reached. Please try again.]".to_string();
        let assistant_msg_id = uuid::Uuid::new_v4().to_string();
        let assistant_now = chrono::Utc::now().to_rfc3339();

        let assistant_message = ChatMessageRecord {
            id: assistant_msg_id,
            session_id: session_id_owned.clone(),
            role: ChatRole::Assistant,
            content: fallback_content.clone(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: assistant_now.clone(),
        };

        {
            let db = self
                .db
                .lock()
                .map_err(|e| AppError::Database(format!("Failed to acquire DB lock: {}", e)))?;
            let conn = db.connection();
            chat_repo::insert_message(conn, &assistant_message)?;
            let preview = truncate_str(&fallback_content, 50);
            chat_repo::update_session_metadata(conn, session_id, &assistant_now, &preview)?;
        }

        Ok(())
    }
}
