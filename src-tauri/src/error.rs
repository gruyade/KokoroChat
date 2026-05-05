use serde::Serialize;
use thiserror::Error;

/// アプリケーション全体で使用するエラー型。
/// Tauri v2ではSerializeを実装することでコマンドのエラー型として使用可能。
#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    /// データベース操作エラー（rusqlite由来）
    #[error("Database error: {0}")]
    Database(String),

    /// HTTP/ネットワークエラー（reqwest由来）
    #[error("Network error: {0}")]
    Network(String),

    /// シリアライズ/デシリアライズエラー（serde_json由来）
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// IOエラー（std::io由来）
    #[error("IO error: {0}")]
    Io(String),

    /// LLM APIエラー（接続失敗、レスポンス不正等）
    #[error("LLM API error: {0}")]
    LlmApi(String),

    /// TTS関連エラー（接続失敗、音声合成失敗等）
    #[error("TTS error: {0}")]
    Tts(String),

    /// バリデーションエラー（入力値不正等）
    #[error("Validation error: {0}")]
    Validation(String),

    /// プラグインエラー（実行失敗、登録エラー等）
    #[error("Plugin error: {0}")]
    Plugin(String),

    /// 添付ファイルエラー（サイズ超過、非対応形式等）
    #[error("Attachment error: {0}")]
    Attachment(String),

    /// リソース未検出エラー
    #[error("Not found: {0}")]
    NotFound(String),
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Database(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Network(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Serialization(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_display() {
        let err = AppError::Database("connection failed".to_string());
        assert_eq!(err.to_string(), "Database error: connection failed");

        let err = AppError::NotFound("character xyz".to_string());
        assert_eq!(err.to_string(), "Not found: character xyz");
    }

    #[test]
    fn test_app_error_serialize() {
        let err = AppError::Validation("name is empty".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("Validation"));
        assert!(json.contains("name is empty"));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let app_err: AppError = io_err.into();
        match app_err {
            AppError::Io(msg) => assert!(msg.contains("file not found")),
            _ => panic!("Expected AppError::Io"),
        }
    }

    #[test]
    fn test_from_serde_json_error() {
        let result: Result<serde_json::Value, _> = serde_json::from_str("invalid json");
        let serde_err = result.unwrap_err();
        let app_err: AppError = serde_err.into();
        match app_err {
            AppError::Serialization(msg) => assert!(!msg.is_empty()),
            _ => panic!("Expected AppError::Serialization"),
        }
    }

    #[test]
    fn test_all_variants_serialize() {
        let variants: Vec<AppError> = vec![
            AppError::Database("db err".into()),
            AppError::Network("net err".into()),
            AppError::Serialization("ser err".into()),
            AppError::Io("io err".into()),
            AppError::LlmApi("llm err".into()),
            AppError::Tts("tts err".into()),
            AppError::Validation("val err".into()),
            AppError::Plugin("plugin err".into()),
            AppError::Attachment("attach err".into()),
            AppError::NotFound("not found err".into()),
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            assert!(!json.is_empty());
        }
    }
}
