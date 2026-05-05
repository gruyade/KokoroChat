// Chat Tauri Commands — チャットセッション・メッセージ操作

use tauri::{AppHandle, State};

use crate::error::AppError;
use crate::models::{ChatMessageRecord, ChatSession};
use crate::state::AppState;

/// 新規チャットセッション作成
///
/// 指定キャラクターに紐づくセッションを作成し、session_idを返す。
#[tauri::command]
pub async fn create_session(
    character_id: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    state.chat_engine.create_session(&character_id).await
}

/// メッセージ送信
///
/// ユーザーメッセージをChatEngineに送信し、LLMレスポンスをストリーミングイベントで返す。
/// 送信完了後、設定に応じて記憶圧縮と自発的発話をバックグラウンドでトリガー。
#[tauri::command]
pub async fn send_message(
    session_id: String,
    content: String,
    attachments: Option<Vec<String>>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let _ = attachments;
    state
        .chat_engine
        .send_message(&session_id, &content, None, &app_handle)
        .await?;

    // バックグラウンドで記憶圧縮チェック
    // 直列化保証: chat_engine.send_message()内でllm_lockを取得→ストリーミング完了→解放済み。
    // memory_manager.check_and_compress()内部でllm_lockを再取得するため、
    // 他のLLMリクエストと競合しない（task 3.3, 3.4で導入済み）。
    let memory_manager = state.memory_manager.clone();
    let session_id_for_memory = session_id.clone();
    tokio::spawn(async move {
        let _ = memory_manager.check_and_compress(&session_id_for_memory).await;
    });

    // 自発的発話はフロントエンドのタイマーから trigger_spontaneous_check コマンドで呼ばれる

    Ok(())
}

/// 自発的発話チェック（フロントエンドのタイマーから呼ばれる）
/// 確率判定を行い、発話する場合はDBに保存してイベント通知。発話しない場合はOk(false)を返す。
#[tauri::command]
pub async fn trigger_spontaneous_check(
    session_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, AppError> {
    use crate::db::repositories::{character as char_repo, chat as chat_repo};
    use crate::llm::client::{ChatMessage, LLMResponse, MessageRole, LLMClientConfig};
    use crate::models::{ChatRole, ChatMessageRecord};
    use tauri::Emitter;

    let config = state.config_manager.get_config();
    if !config.spontaneous.enabled {
        println!("[spontaneous] disabled in config, skipping");
        return Ok(false);
    }

    // 確率判定
    let probability = config.spontaneous.probability;
    let roll = rand::random::<f32>();
    println!("[spontaneous] probability={}, roll={}", probability, roll);
    if roll > probability {
        println!("[spontaneous] probability check failed, skipping");
        return Ok(false);
    }

    // セッション情報取得
    let (system_prompt, recent_messages) = {
        let db_guard = state.db.lock().map_err(|e| AppError::Database(format!("{}", e)))?;
        let conn = db_guard.connection();

        let session = chat_repo::get_session(conn, &session_id)?
            .ok_or_else(|| AppError::NotFound(format!("Session: {}", session_id)))?;
        let character = char_repo::get_character(conn, &session.character_id)?
            .ok_or_else(|| AppError::NotFound(format!("Character: {}", session.character_id)))?;
        let msgs = chat_repo::get_messages(conn, &session_id)?;
        let recent = if msgs.len() > 15 { msgs[msgs.len()-15..].to_vec() } else { msgs };
        (character.system_prompt, recent)
    };

    // プロンプト構築
    let mut messages = vec![ChatMessage {
        role: MessageRole::System,
        content: system_prompt,
        tool_call_id: None,
    }];
    for msg in &recent_messages {
        let role = match msg.role {
            ChatRole::User => MessageRole::User,
            ChatRole::Assistant | ChatRole::Spontaneous => MessageRole::Assistant,
            ChatRole::Tool => MessageRole::Tool,
        };
        messages.push(ChatMessage { role, content: msg.content.clone(), tool_call_id: None });
    }

    // 確率1.0の場合は必ず発話させる（デバッグ用途を想定）
    let spontaneous_prompt = if probability >= 1.0 {
        "あなたはキャラクターとして、直前の会話の流れや状況を踏まえて自然に一言話しかけてください。短く、キャラクターらしい口調で。必ず何か話してください。".to_string()
    } else {
        "あなたはキャラクターとして、直前の会話の流れや状況を踏まえて自然に一言話しかけてください。短く、キャラクターらしい口調で。会話の流れ的にどうしても不自然な場合のみ「[SKIP]」とだけ返してください。".to_string()
    };

    messages.push(ChatMessage {
        role: MessageRole::User,
        content: spontaneous_prompt,
        tool_call_id: None,
    });

    // LLM呼び出し
    let llm_config = state.config_manager
        .get_model_settings(&crate::models::config::ModelPurpose::Chat)
        .map(|s| LLMClientConfig { base_url: s.base_url, model: s.model, api_key: s.api_key, temperature: s.temperature })
        .unwrap_or(LLMClientConfig { base_url: String::new(), model: String::new(), api_key: None, temperature: 0.9 });

    if llm_config.base_url.is_empty() {
        println!("[spontaneous] LLM base_url is empty, skipping");
        return Ok(false);
    }
    println!("[spontaneous] calling LLM (model={})", llm_config.model);

    let response = state.llm_client.chat(&messages, &llm_config, None).await?;
    let content = match response {
        LLMResponse::Text(t) => t.trim().to_string(),
        _ => {
            println!("[spontaneous] LLM returned non-text response, skipping");
            return Ok(false);
        }
    };
    if content.is_empty() || content.contains("[SKIP]") {
        println!("[spontaneous] LLM returned SKIP or empty (content={:?}), skipping", content);
        return Ok(false);
    }
    let preview: String = content.chars().take(50).collect();
    println!("[spontaneous] LLM generated: {}", preview);

    // DBに保存
    let now = chrono::Utc::now().to_rfc3339();
    let msg = ChatMessageRecord {
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
        let db_guard = state.db.lock().map_err(|e| AppError::Database(format!("{}", e)))?;
        chat_repo::insert_message(db_guard.connection(), &msg)?;
    }

    // フロントエンドにイベント通知
    let _ = app_handle.emit("spontaneous:message", serde_json::json!({
        "session_id": session_id,
        "message": content,
    }));

    Ok(true)
}

/// チャット履歴取得
///
/// 指定セッションの全メッセージを時系列順で返す。
#[tauri::command]
pub async fn get_history(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessageRecord>, AppError> {
    state.chat_engine.get_history(&session_id).await
}

/// セッション一覧取得
///
/// 指定キャラクターに紐づく全セッションを返す。
#[tauri::command]
pub async fn list_sessions(
    character_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatSession>, AppError> {
    state.chat_engine.list_sessions(&character_id).await
}

/// セッション削除
///
/// 指定セッションとその全メッセージを削除する。
#[tauri::command]
pub async fn delete_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.chat_engine.delete_session(&session_id).await
}

/// メッセージ再生成
///
/// 対象のassistantメッセージを削除し、直前のuserメッセージ内容で再送信する。
#[tauri::command]
pub async fn regenerate_message(
    session_id: String,
    message_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state
        .chat_engine
        .regenerate(&session_id, &message_id, &app_handle)
        .await
}

/// メッセージ削除
#[tauri::command]
pub async fn delete_message(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let db = state.db.lock().map_err(|e| AppError::Database(format!("{}", e)))?;
    db.connection().execute(
        "DELETE FROM chat_messages WHERE id = ?1",
        rusqlite::params![id],
    )?;
    Ok(())
}
