// Character Tauri Commands — キャラクターCRUD操作

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::repositories::{
    character as char_repo, chat as chat_repo, memory as memory_repo, thought as thought_repo,
};
use crate::error::AppError;
use crate::models::{Character, CharacterUpdate, ChatMessageRecord, ChatRole, ChatSession, Memory, Thought};
use crate::state::AppState;

// ─── エクスポート用データ型 ───

/// エクスポートオプション
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub include_chats: bool,
    pub include_thoughts: bool,
    pub include_memories: bool,
}

/// インポートオプション
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOptions {
    pub include_chats: bool,
    pub include_thoughts: bool,
    pub include_memories: bool,
}

/// エクスポートデータ全体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterExportData {
    pub version: u32,
    pub exported_at: String,
    pub character: ExportedCharacter,
    pub chat_sessions: Option<Vec<ExportedChatSession>>,
    pub thoughts: Option<Vec<ExportedThought>>,
    pub memories: Option<Vec<ExportedMemory>>,
}

/// エクスポート用キャラクター設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedCharacter {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub tts_config: Option<serde_json::Value>,
}

/// エクスポート用チャットセッション
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedChatSession {
    pub id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub messages: Vec<ExportedMessage>,
}

/// エクスポート用メッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMessage {
    pub role: String,
    pub content: String,
    pub attachments: Option<serde_json::Value>,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_id: Option<String>,
    pub created_at: String,
}

/// エクスポート用思考
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedThought {
    pub content: String,
    pub context: Option<String>,
    pub created_at: String,
}

/// エクスポート用記憶
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMemory {
    pub content: String,
    pub source_session_id: Option<String>,
    pub created_at: String,
}

/// キャラクター新規作成
///
/// system_promptが指定されていればそれを使用、未指定ならLLMで自動生成。
#[tauri::command]
pub async fn create_character(
    name: String,
    description: String,
    system_prompt: Option<String>,
    state: State<'_, AppState>,
) -> Result<Character, AppError> {
    let creator = &state.character_creator;

    // system_promptが空でなければそのまま使用、なければLLMで生成
    let final_prompt = match system_prompt {
        Some(ref p) if !p.trim().is_empty() => p.clone(),
        _ => creator.generate_system_prompt(&name, &description).await?,
    };

    let now = chrono::Utc::now().to_rfc3339();
    let character = Character {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        description,
        system_prompt: final_prompt,
        avatar_path: None,
        tts_config: None,
        created_at: now.clone(),
        updated_at: now,
    };

    creator.save_character(&character).await?;

    Ok(character)
}

/// 全キャラクター一覧取得
#[tauri::command]
pub async fn list_characters(
    state: State<'_, AppState>,
) -> Result<Vec<Character>, AppError> {
    state.character_creator.list_characters().await
}

/// IDでキャラクター取得
#[tauri::command]
pub async fn get_character(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<Character>, AppError> {
    state.character_creator.get_character(&id).await
}

/// キャラクター部分更新
#[tauri::command]
pub async fn update_character(
    id: String,
    updates: CharacterUpdate,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.character_creator.update_character(&id, updates).await
}

/// キャラクター削除（関連データもCASCADE削除）
#[tauri::command]
pub async fn delete_character(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.character_creator.delete_character(&id).await
}

/// System Prompt生成（説明内容から新規作成）
#[tauri::command]
pub async fn generate_system_prompt(
    name: String,
    description: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    state.character_creator.generate_system_prompt(&name, &description).await
}

/// System Prompt改善（既存プロンプト + 説明内容から改良）
#[tauri::command]
pub async fn improve_system_prompt(
    name: String,
    description: String,
    current_prompt: String,
    direction: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    use crate::llm::client::{ChatMessage, LLMClientConfig, LLMResponse, MessageRole};
    use crate::models::config::ModelPurpose;

    // キャラクター生成用のLLM設定を取得
    let llm_config = state.config_manager
        .get_model_settings(&ModelPurpose::CharacterGeneration)
        .map(|s| LLMClientConfig {
            base_url: s.base_url,
            model: s.model,
            api_key: s.api_key,
            temperature: s.temperature,
            provider: s.provider,
        })
        .unwrap_or_else(|| LLMClientConfig {
            base_url: String::new(),
            model: String::new(),
            api_key: None,
            temperature: 0.7,
            provider: None,
        });

    let direction_text = match &direction {
        Some(d) if !d.trim().is_empty() => format!(
            "\n\n【改善の方向性（ユーザー指示）】\n{}\n\n上記の方向性を最優先で反映しつつ、以下の観点でも改善してください：",
            d.trim()
        ),
        _ => "\n\n以下の観点で改善してください：".to_string(),
    };

    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: format!(
            "あなたはAIキャラクター設計の専門家です。\n\
             以下の既存System Promptを改善してください。\n\n\
             【キャラクター名】{}\n\
             【概要説明】{}\n\
             【現在のSystem Prompt】\n{}\
             {}\n\
             1. キャラクターの性格・人格をより具体的に\n\
             2. 話し方・口調のパターンをより明確に\n\
             3. 行動原理・価値観を追加\n\
             4. 会話における振る舞いのガイドラインを充実\n\
             5. 矛盾や曖昧な部分を解消\n\n\
             改善後のSystem Promptのみを出力してください。説明や前置きは不要です。",
            name, description, current_prompt, direction_text
        ),
        tool_call_id: None,
        images: None,
    }];

    let response = state.llm_client.chat(&messages, &llm_config, None).await?;

    match response {
        LLMResponse::Text(text) => Ok(text),
        LLMResponse::ToolCalls(_) => Err(AppError::LlmApi(
            "Unexpected tool_call response during prompt improvement".to_string(),
        )),
    }
}

/// キャラクターデータのエクスポート
///
/// 指定キャラクターの設定と、オプションに応じてチャット履歴・思考・記憶を
/// JSON構造として返却する。
#[tauri::command]
pub async fn export_character(
    character_id: String,
    options: ExportOptions,
    state: State<'_, AppState>,
) -> Result<CharacterExportData, AppError> {
    // DBロック取得
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(format!("DB lock failed: {}", e)))?;
    let conn = db.connection();

    // キャラクター取得
    let character = crate::db::repositories::character::get_character(conn, &character_id)?
        .ok_or_else(|| AppError::NotFound(format!("Character not found: {}", character_id)))?;

    // tts_configをserde_json::Valueに変換
    let tts_config_value = character
        .tts_config
        .as_ref()
        .map(|c| serde_json::to_value(c))
        .transpose()?;

    let exported_character = ExportedCharacter {
        name: character.name,
        description: character.description,
        system_prompt: character.system_prompt,
        tts_config: tts_config_value,
    };

    // チャット履歴（オプション）
    let chat_sessions = if options.include_chats {
        let sessions = chat_repo::list_sessions(conn, &character_id)?;
        let mut exported_sessions = Vec::new();

        for session in sessions {
            let messages = chat_repo::get_messages(conn, &session.id)?;
            let exported_messages: Vec<ExportedMessage> = messages
                .into_iter()
                .map(|msg| {
                    let role_str = match msg.role {
                        crate::models::ChatRole::User => "user",
                        crate::models::ChatRole::Assistant => "assistant",
                        crate::models::ChatRole::Spontaneous => "spontaneous",
                        crate::models::ChatRole::Tool => "tool",
                    };
                    let attachments_value = msg
                        .attachments
                        .map(|a| serde_json::to_value(a))
                        .transpose()?;
                    let tool_calls_value = msg
                        .tool_calls
                        .map(|t| serde_json::to_value(t))
                        .transpose()?;

                    Ok(ExportedMessage {
                        role: role_str.to_string(),
                        content: msg.content,
                        attachments: attachments_value,
                        tool_calls: tool_calls_value,
                        tool_call_id: msg.tool_call_id,
                        created_at: msg.created_at,
                    })
                })
                .collect::<Result<Vec<_>, AppError>>()?;

            exported_sessions.push(ExportedChatSession {
                id: session.id,
                title: session.title,
                created_at: session.created_at,
                messages: exported_messages,
            });
        }

        Some(exported_sessions)
    } else {
        None
    };

    // 思考（オプション）
    let thoughts = if options.include_thoughts {
        let thought_list = thought_repo::get_thoughts(conn, &character_id, None)?;
        let exported: Vec<ExportedThought> = thought_list
            .into_iter()
            .map(|t| ExportedThought {
                content: t.content,
                context: t.context,
                created_at: t.created_at,
            })
            .collect();
        Some(exported)
    } else {
        None
    };

    // 記憶（オプション）
    let memories = if options.include_memories {
        let memory_list = memory_repo::list_memories(conn, &character_id)?;
        let exported: Vec<ExportedMemory> = memory_list
            .into_iter()
            .map(|m| ExportedMemory {
                content: m.content,
                source_session_id: m.source_session_id,
                created_at: m.created_at,
            })
            .collect();
        Some(exported)
    } else {
        None
    };

    let exported_at = chrono::Utc::now().to_rfc3339();

    Ok(CharacterExportData {
        version: 1,
        exported_at,
        character: exported_character,
        chat_sessions,
        thoughts,
        memories,
    })
}

/// アバター画像を保存
/// Base64エンコードされた画像データを受け取り、appDataDir/avatars/ に保存
#[tauri::command]
pub async fn save_avatar(
    base64_data: String,
    app_handle: tauri::AppHandle,
) -> Result<String, AppError> {
    use tauri::Manager;

    let app_data_dir = app_handle.path().app_data_dir()
        .map_err(|e| AppError::Io(format!("Failed to get app data dir: {}", e)))?;
    let avatars_dir = app_data_dir.join("avatars");
    std::fs::create_dir_all(&avatars_dir)
        .map_err(|e| AppError::Io(format!("Failed to create avatars dir: {}", e)))?;

    let file_name = format!("{}.png", uuid::Uuid::new_v4());
    let file_path = avatars_dir.join(&file_name);

    // Base64デコード
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD
        .decode(&base64_data)
        .map_err(|e| AppError::Io(format!("Failed to decode base64: {}", e)))?;

    std::fs::write(&file_path, &data)
        .map_err(|e| AppError::Io(format!("Failed to write avatar file: {}", e)))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// アバター画像をBase64で読み込み
#[tauri::command]
pub async fn read_avatar(
    avatar_path: String,
) -> Result<String, AppError> {
    use base64::Engine;

    let data = std::fs::read(&avatar_path)
        .map_err(|e| AppError::Io(format!("Failed to read avatar file: {}", e)))?;

    Ok(base64::engine::general_purpose::STANDARD.encode(&data))
}

/// キャラクターデータのインポート
///
/// エクスポートされたJSONデータからキャラクターを新規作成する。
/// オプションに応じてチャット履歴・思考・記憶もインポートする。
/// 全操作はトランザクション内で実行し、失敗時はロールバック。
#[tauri::command]
pub async fn import_character(
    data: CharacterExportData,
    options: ImportOptions,
    state: State<'_, AppState>,
) -> Result<Character, AppError> {
    // バリデーション
    if data.version != 1 {
        return Err(AppError::Validation(format!(
            "未対応のエクスポート形式（version: {}）",
            data.version
        )));
    }
    if data.character.name.trim().is_empty() {
        return Err(AppError::Validation(
            "必須データが不足: character.name".to_string(),
        ));
    }
    if data.character.description.trim().is_empty() {
        return Err(AppError::Validation(
            "必須データが不足: character.description".to_string(),
        ));
    }
    if data.character.system_prompt.trim().is_empty() {
        return Err(AppError::Validation(
            "必須データが不足: character.system_prompt".to_string(),
        ));
    }

    // 新規キャラクターID生成
    let new_character_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // tts_configをTTSConfig型に変換
    let tts_config = data
        .character
        .tts_config
        .as_ref()
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()
        .map_err(|e| AppError::Validation(format!("tts_config形式が不正: {}", e)))?;

    let character = Character {
        id: new_character_id.clone(),
        name: data.character.name.clone(),
        description: data.character.description.clone(),
        system_prompt: data.character.system_prompt.clone(),
        avatar_path: None,
        tts_config,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    // DBロック取得・トランザクション内で全操作実行
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(format!("DB lock failed: {}", e)))?;
    let conn = db.connection();

    conn.execute_batch("BEGIN")
        .map_err(|e| AppError::Database(format!("Transaction begin failed: {}", e)))?;

    let result = (|| -> Result<(), AppError> {
        // キャラクター挿入
        char_repo::insert_character(conn, &character)?;

        // チャット履歴インポート
        if options.include_chats {
            if let Some(ref sessions) = data.chat_sessions {
                for session_data in sessions {
                    let new_session_id = uuid::Uuid::new_v4().to_string();

                    let session = ChatSession {
                        id: new_session_id.clone(),
                        character_id: new_character_id.clone(),
                        title: session_data.title.clone(),
                        last_message_at: None,
                        last_message_preview: None,
                        created_at: session_data.created_at.clone(),
                    };
                    chat_repo::insert_session(conn, &session)?;

                    for msg_data in &session_data.messages {
                        let new_msg_id = uuid::Uuid::new_v4().to_string();

                        let role = match msg_data.role.as_str() {
                            "user" => ChatRole::User,
                            "assistant" => ChatRole::Assistant,
                            "spontaneous" => ChatRole::Spontaneous,
                            "tool" => ChatRole::Tool,
                            _ => ChatRole::User,
                        };

                        let attachments = msg_data
                            .attachments
                            .as_ref()
                            .map(|v| serde_json::from_value(v.clone()))
                            .transpose()
                            .map_err(|e| {
                                AppError::Validation(format!("attachments形式が不正: {}", e))
                            })?;

                        let tool_calls = msg_data
                            .tool_calls
                            .as_ref()
                            .map(|v| serde_json::from_value(v.clone()))
                            .transpose()
                            .map_err(|e| {
                                AppError::Validation(format!("tool_calls形式が不正: {}", e))
                            })?;

                        let message = ChatMessageRecord {
                            id: new_msg_id,
                            session_id: new_session_id.clone(),
                            role,
                            content: msg_data.content.clone(),
                            attachments,
                            tool_calls,
                            tool_call_id: msg_data.tool_call_id.clone(),
                            created_at: msg_data.created_at.clone(),
                        };
                        chat_repo::insert_message(conn, &message)?;
                    }
                }
            }
        }

        // 思考インポート
        if options.include_thoughts {
            if let Some(ref thoughts) = data.thoughts {
                for thought_data in thoughts {
                    let new_thought_id = uuid::Uuid::new_v4().to_string();

                    let thought = Thought {
                        id: new_thought_id,
                        character_id: new_character_id.clone(),
                        content: thought_data.content.clone(),
                        context: thought_data.context.clone(),
                        created_at: thought_data.created_at.clone(),
                    };
                    thought_repo::insert_thought(conn, &thought)?;
                }
            }
        }

        // 記憶インポート
        if options.include_memories {
            if let Some(ref memories) = data.memories {
                for memory_data in memories {
                    let new_memory_id = uuid::Uuid::new_v4().to_string();

                    let memory = Memory {
                        id: new_memory_id,
                        character_id: new_character_id.clone(),
                        content: memory_data.content.clone(),
                        source_session_id: None, // インポート時はセッションIDが変わるためnull
                        source_message_from: None,
                        source_message_to: None,
                        created_at: memory_data.created_at.clone(),
                        updated_at: now.clone(),
                    };
                    memory_repo::insert_memory(conn, &memory)?;
                }
            }
        }

        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")
                .map_err(|e| AppError::Database(format!("Transaction commit failed: {}", e)))?;
            Ok(character)
        }
        Err(e) => {
            conn.execute_batch("ROLLBACK").ok(); // ロールバック失敗は無視
            Err(e)
        }
    }
}
