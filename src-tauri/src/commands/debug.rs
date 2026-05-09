// Debug Tauri Commands — デバッグモード用の手動トリガー

use tauri::State;

use crate::error::AppError;
use crate::models::Thought;
use crate::state::AppState;

/// 記憶圧縮を手動実行（デバッグ用: 閾値を無視して強制実行）
#[tauri::command]
pub async fn debug_compress_memory(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    use crate::db::repositories::chat as chat_repo;
    use crate::llm::client::{ChatMessage, LLMResponse, MessageRole};

    // セッションのメッセージを取得
    let (messages_text, character_id, first_id, last_id) = {
        let db = state.chat_engine_db().map_err(|e| AppError::Database(e))?;
        let conn = db.connection();

        let session = chat_repo::get_session(conn, &session_id)?
            .ok_or_else(|| AppError::NotFound(format!("Session: {}", session_id)))?;

        let messages = chat_repo::get_messages(conn, &session_id)?;
        if messages.is_empty() {
            return Err(AppError::Validation("メッセージがない".to_string()));
        }

        let text = messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    crate::models::ChatRole::User => "ユーザー",
                    crate::models::ChatRole::Assistant => "アシスタント",
                    crate::models::ChatRole::Spontaneous => "アシスタント（自発）",
                    crate::models::ChatRole::Tool => "ツール",
                };
                format!("{}: {}", role, m.content)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let first = messages.first().map(|m| m.id.clone());
        let last = messages.last().map(|m| m.id.clone());

        (text, session.character_id, first, last)
    };

    // LLMで要約
    let config = state
        .config_manager
        .get_model_settings(&crate::models::config::ModelPurpose::Memory)
        .map(|s| crate::llm::client::LLMClientConfig {
            base_url: s.base_url,
            model: s.model,
            api_key: s.api_key,
            temperature: s.temperature,
            provider: s.provider,
        })
        .unwrap_or_else(|| crate::llm::client::LLMClientConfig {
            base_url: String::new(),
            model: String::new(),
            api_key: None,
            temperature: 0.3,
            provider: None,
        });

    let llm_messages = vec![
        ChatMessage {
            role: MessageRole::System,
            content: "あなたは会話要約アシスタントです。以下の会話を分析し、重要な情報を簡潔に要約してください。\n\n以下の観点で要約してください：\n- ユーザーに関する重要な事実\n- 議論された主要なトピック\n- 表明された好みや意見\n- 行われた約束やコミットメント\n\n箇条書きで簡潔にまとめてください。".to_string(),
            tool_call_id: None,
            images: None,
        },
        ChatMessage {
            role: MessageRole::User,
            content: format!("以下の会話を要約してください：\n\n{}", messages_text),
            tool_call_id: None,
            images: None,
        },
    ];

    let response = state.llm_client.chat(&llm_messages, &config, None).await?;
    let summary = match response {
        LLMResponse::Text(text) => text,
        _ => return Err(AppError::LlmApi("Unexpected response".to_string())),
    };

    // Memory保存
    let now = chrono::Utc::now().to_rfc3339();
    let memory = crate::models::Memory {
        id: uuid::Uuid::new_v4().to_string(),
        character_id,
        content: summary.clone(),
        source_session_id: Some(session_id),
        source_message_from: first_id,
        source_message_to: last_id,
        created_at: now.clone(),
        updated_at: now,
    };

    {
        let db = state.chat_engine_db().map_err(|e| AppError::Database(e))?;
        let conn = db.connection();
        crate::db::repositories::memory::insert_memory(conn, &memory)?;
    }

    Ok(summary)
}

/// 思考生成を手動実行
#[tauri::command]
pub async fn debug_generate_thought(
    character_id: String,
    state: State<'_, AppState>,
) -> Result<Thought, AppError> {
    state.thought_engine.generate_thought(&character_id).await
}

/// 自発的発話を手動実行（LLMに1回だけ自発的発話を生成させる）
#[tauri::command]
pub async fn debug_trigger_spontaneous(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    use crate::db::repositories::{character as char_repo, chat as chat_repo};
    use crate::llm::client::{ChatMessage, LLMResponse, MessageRole};
    use crate::models::ChatRole;

    // セッション情報取得
    let (system_prompt, recent_messages) = {
        let db = state.chat_engine_db().map_err(|e| AppError::Database(e))?;
        let conn = db.connection();

        let session = chat_repo::get_session(conn, &session_id)?
            .ok_or_else(|| AppError::NotFound(format!("Session: {}", session_id)))?;

        let character = char_repo::get_character(conn, &session.character_id)?
            .ok_or_else(|| AppError::NotFound(format!("Character: {}", session.character_id)))?;

        let msgs = chat_repo::get_messages(conn, &session_id)?;
        let recent = if msgs.len() > 20 {
            msgs[msgs.len() - 20..].to_vec()
        } else {
            msgs
        };

        (character.system_prompt, recent)
    };

    // プロンプト構築
    let mut messages = vec![ChatMessage {
        role: MessageRole::System,
        content: system_prompt,
        tool_call_id: None,
        images: None,
    }];

    for msg in &recent_messages {
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

    messages.push(ChatMessage {
        role: MessageRole::User,
        content: "あなたはキャラクターとして、直前の会話の流れや状況を踏まえて自然に一言話しかけてください。短く、キャラクターらしい口調で。必ず何か話してください。".to_string(),
        tool_call_id: None,
        images: None,
    });

    // LLM呼び出し
    let config = state
        .config_manager
        .get_model_settings(&crate::models::config::ModelPurpose::Chat)
        .map(|s| crate::llm::client::LLMClientConfig {
            base_url: s.base_url,
            model: s.model,
            api_key: s.api_key,
            temperature: s.temperature,
            provider: s.provider,
        })
        .unwrap_or_else(|| crate::llm::client::LLMClientConfig {
            base_url: String::new(),
            model: String::new(),
            api_key: None,
            temperature: 0.7,
            provider: None,
        });

    let response = state.llm_client.chat(&messages, &config, None).await?;

    let content = match response {
        LLMResponse::Text(text) => text.trim().to_string(),
        _ => return Err(AppError::LlmApi("Unexpected response".to_string())),
    };

    if content.is_empty() {
        return Err(AppError::LlmApi("Empty spontaneous message".to_string()));
    }

    // DBに保存
    let now = chrono::Utc::now().to_rfc3339();
    let msg = crate::models::ChatMessageRecord {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.clone(),
        role: ChatRole::Spontaneous,
        content: content.clone(),
        attachments: None,
        tool_calls: None,
        tool_call_id: None,
        created_at: now,
    };

    {
        let db = state.chat_engine_db().map_err(|e| AppError::Database(e))?;
        let conn = db.connection();
        chat_repo::insert_message(conn, &msg)?;
    }

    Ok(content)
}
