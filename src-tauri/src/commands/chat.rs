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
/// attachmentsパラメータはファイルパスのリスト（現時点ではNoneとしてChatEngineに渡す）。
#[tauri::command]
pub async fn send_message(
    session_id: String,
    content: String,
    attachments: Option<Vec<String>>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    // attachmentsは将来Task 13で処理実装予定。現時点ではNoneを渡す。
    let _ = attachments;
    state
        .chat_engine
        .send_message(&session_id, &content, None, &app_handle)
        .await
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
