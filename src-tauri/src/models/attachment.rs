use serde::{Deserialize, Serialize};

/// ファイル添付の種別
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    Text,
    Pdf,
    Image,
}

/// 添付ファイル情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub file_name: String,
    pub file_path: String,
    pub attachment_type: AttachmentType,
    pub size_bytes: u64,
    /// テキスト/PDF抽出結果
    pub extracted_text: Option<String>,
    /// 画像のBase64エンコード
    pub base64_data: Option<String>,
}

/// 最大ファイルサイズ: 10MB
pub const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
