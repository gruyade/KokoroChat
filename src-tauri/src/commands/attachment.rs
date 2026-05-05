// Attachment Tauri Commands — ファイル添付処理

use tauri::State;

use crate::error::AppError;
use crate::models::Attachment;
use crate::state::AppState;

/// ファイルを処理してAttachment情報を返す
///
/// 指定パスのファイルを読み込み、種別判定・テキスト抽出・Base64エンコードを行う。
#[tauri::command]
pub async fn process_attachment(
    file_path: String,
    state: State<'_, AppState>,
) -> Result<Attachment, AppError> {
    state.attachment_processor.process_file(&file_path).await
}

/// サポートするファイル拡張子一覧を返す
#[tauri::command]
pub async fn get_supported_extensions(
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let extensions = state
        .attachment_processor
        .supported_extensions()
        .into_iter()
        .map(|ext| ext.to_string())
        .collect();
    Ok(extensions)
}
