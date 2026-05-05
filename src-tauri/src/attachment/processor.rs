// Attachment Processor - ファイル添付処理

use async_trait::async_trait;
use base64::Engine as _;
use std::path::Path;

use crate::error::AppError;
use crate::models::attachment::{Attachment, AttachmentType, MAX_FILE_SIZE};

/// サポートする拡張子一覧
const SUPPORTED_EXTENSIONS: &[&str] = &["txt", "md", "csv", "pdf", "png", "jpg", "webp"];

/// テキスト系拡張子
const TEXT_EXTENSIONS: &[&str] = &["txt", "md", "csv"];

/// 画像系拡張子
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "webp"];

/// 添付ファイル処理トレイト
#[async_trait]
pub trait AttachmentProcessor: Send + Sync {
    /// ファイルを処理してAttachmentを生成
    async fn process_file(&self, file_path: &str) -> Result<Attachment, AppError>;
    /// サポートする拡張子一覧を返す
    fn supported_extensions(&self) -> Vec<&'static str>;
    /// 指定パスの拡張子がサポート対象か判定
    fn is_supported(&self, file_path: &str) -> bool;
    /// Attachmentからテキストを抽出
    async fn extract_text(&self, attachment: &Attachment) -> Result<String, AppError>;
    /// Attachmentの画像データをBase64エンコード
    async fn encode_image(&self, attachment: &Attachment) -> Result<String, AppError>;
}

/// デフォルト実装
pub struct DefaultAttachmentProcessor;

impl DefaultAttachmentProcessor {
    pub fn new() -> Self {
        Self
    }

    /// 拡張子からAttachmentTypeを判定
    fn determine_type(extension: &str) -> Option<AttachmentType> {
        let ext = extension.to_lowercase();
        if TEXT_EXTENSIONS.contains(&ext.as_str()) {
            Some(AttachmentType::Text)
        } else if ext == "pdf" {
            Some(AttachmentType::Pdf)
        } else if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            Some(AttachmentType::Image)
        } else {
            None
        }
    }

    /// ファイルパスから拡張子を取得
    fn get_extension(file_path: &str) -> Option<String> {
        Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
    }
}

impl Default for DefaultAttachmentProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttachmentProcessor for DefaultAttachmentProcessor {
    async fn process_file(&self, file_path: &str) -> Result<Attachment, AppError> {
        let path = Path::new(file_path);

        // 1. ファイル存在チェック
        if !path.exists() {
            return Err(AppError::Attachment(format!(
                "ファイルが見つかりません: {}",
                file_path
            )));
        }

        // 2. ファイルサイズチェック
        let metadata = std::fs::metadata(path)?;
        let size_bytes = metadata.len();
        if size_bytes > MAX_FILE_SIZE {
            return Err(AppError::Attachment(format!(
                "ファイルサイズが上限(10MB)を超えています: {}バイト",
                size_bytes
            )));
        }

        // 3. 拡張子からAttachmentType判定
        let extension = Self::get_extension(file_path).ok_or_else(|| {
            AppError::Attachment(format!(
                "ファイルの拡張子を判定できません。対応形式: {}",
                SUPPORTED_EXTENSIONS.join(", ")
            ))
        })?;

        let attachment_type = Self::determine_type(&extension).ok_or_else(|| {
            AppError::Attachment(format!(
                "非対応のファイル形式です: .{}。対応形式: {}",
                extension,
                SUPPORTED_EXTENSIONS.join(", ")
            ))
        })?;

        // 4. ファイル名取得
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // 5. ファイル内容読み込み
        let (extracted_text, base64_data) = match &attachment_type {
            AttachmentType::Text => {
                let text = std::fs::read_to_string(path).map_err(|e| {
                    AppError::Attachment(format!("テキストファイルの読み込みに失敗: {}", e))
                })?;
                (Some(text), None)
            }
            AttachmentType::Pdf => {
                let text = extract_pdf_text(path)?;
                (Some(text), None)
            }
            AttachmentType::Image => {
                let bytes = std::fs::read(path).map_err(|e| {
                    AppError::Attachment(format!("画像ファイルの読み込みに失敗: {}", e))
                })?;
                let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                (None, Some(encoded))
            }
        };

        // 6. Attachment構造体を返す
        let id = uuid::Uuid::new_v4().to_string();
        Ok(Attachment {
            id,
            file_name,
            file_path: file_path.to_string(),
            attachment_type,
            size_bytes,
            extracted_text,
            base64_data,
        })
    }

    fn supported_extensions(&self) -> Vec<&'static str> {
        SUPPORTED_EXTENSIONS.to_vec()
    }

    fn is_supported(&self, file_path: &str) -> bool {
        Self::get_extension(file_path)
            .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.as_str()))
            .unwrap_or(false)
    }

    async fn extract_text(&self, attachment: &Attachment) -> Result<String, AppError> {
        match &attachment.attachment_type {
            AttachmentType::Text | AttachmentType::Pdf => {
                attachment.extracted_text.clone().ok_or_else(|| {
                    AppError::Attachment("テキストが抽出されていません".to_string())
                })
            }
            AttachmentType::Image => Err(AppError::Attachment(
                "画像ファイルからテキスト抽出はできない".to_string(),
            )),
        }
    }

    async fn encode_image(&self, attachment: &Attachment) -> Result<String, AppError> {
        match &attachment.attachment_type {
            AttachmentType::Image => attachment.base64_data.clone().ok_or_else(|| {
                AppError::Attachment("Base64データが存在しません".to_string())
            }),
            _ => Err(AppError::Attachment(
                "画像以外のファイルはBase64エンコードできない".to_string(),
            )),
        }
    }
}

/// PDFファイルからテキストを抽出
fn extract_pdf_text(path: &Path) -> Result<String, AppError> {
    let bytes = std::fs::read(path)
        .map_err(|e| AppError::Attachment(format!("PDFファイルの読み込みに失敗: {}", e)))?;

    pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| AppError::Attachment(format!("PDFテキスト抽出に失敗: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_determine_type_text() {
        assert_eq!(
            DefaultAttachmentProcessor::determine_type("txt"),
            Some(AttachmentType::Text)
        );
        assert_eq!(
            DefaultAttachmentProcessor::determine_type("md"),
            Some(AttachmentType::Text)
        );
        assert_eq!(
            DefaultAttachmentProcessor::determine_type("csv"),
            Some(AttachmentType::Text)
        );
    }

    #[test]
    fn test_determine_type_pdf() {
        assert_eq!(
            DefaultAttachmentProcessor::determine_type("pdf"),
            Some(AttachmentType::Pdf)
        );
    }

    #[test]
    fn test_determine_type_image() {
        assert_eq!(
            DefaultAttachmentProcessor::determine_type("png"),
            Some(AttachmentType::Image)
        );
        assert_eq!(
            DefaultAttachmentProcessor::determine_type("jpg"),
            Some(AttachmentType::Image)
        );
        assert_eq!(
            DefaultAttachmentProcessor::determine_type("webp"),
            Some(AttachmentType::Image)
        );
    }

    #[test]
    fn test_determine_type_unsupported() {
        assert_eq!(DefaultAttachmentProcessor::determine_type("exe"), None);
        assert_eq!(DefaultAttachmentProcessor::determine_type("zip"), None);
        assert_eq!(DefaultAttachmentProcessor::determine_type("docx"), None);
    }

    #[test]
    fn test_determine_type_case_insensitive() {
        assert_eq!(
            DefaultAttachmentProcessor::determine_type("TXT"),
            Some(AttachmentType::Text)
        );
        assert_eq!(
            DefaultAttachmentProcessor::determine_type("PNG"),
            Some(AttachmentType::Image)
        );
        assert_eq!(
            DefaultAttachmentProcessor::determine_type("Pdf"),
            Some(AttachmentType::Pdf)
        );
    }

    #[test]
    fn test_get_extension() {
        assert_eq!(
            DefaultAttachmentProcessor::get_extension("test.txt"),
            Some("txt".to_string())
        );
        assert_eq!(
            DefaultAttachmentProcessor::get_extension("/path/to/file.PDF"),
            Some("pdf".to_string())
        );
        assert_eq!(
            DefaultAttachmentProcessor::get_extension("no_extension"),
            None
        );
    }

    #[test]
    fn test_is_supported() {
        let processor = DefaultAttachmentProcessor::new();
        assert!(processor.is_supported("file.txt"));
        assert!(processor.is_supported("file.md"));
        assert!(processor.is_supported("file.csv"));
        assert!(processor.is_supported("file.pdf"));
        assert!(processor.is_supported("file.png"));
        assert!(processor.is_supported("file.jpg"));
        assert!(processor.is_supported("file.webp"));
        assert!(!processor.is_supported("file.exe"));
        assert!(!processor.is_supported("file.zip"));
        assert!(!processor.is_supported("no_extension"));
    }

    #[test]
    fn test_is_supported_case_insensitive() {
        let processor = DefaultAttachmentProcessor::new();
        assert!(processor.is_supported("file.TXT"));
        assert!(processor.is_supported("file.Png"));
        assert!(processor.is_supported("file.PDF"));
    }

    #[test]
    fn test_supported_extensions() {
        let processor = DefaultAttachmentProcessor::new();
        let exts = processor.supported_extensions();
        assert_eq!(exts.len(), 7);
        assert!(exts.contains(&"txt"));
        assert!(exts.contains(&"md"));
        assert!(exts.contains(&"csv"));
        assert!(exts.contains(&"pdf"));
        assert!(exts.contains(&"png"));
        assert!(exts.contains(&"jpg"));
        assert!(exts.contains(&"webp"));
    }

    #[tokio::test]
    async fn test_process_file_text() {
        let mut tmp = NamedTempFile::with_suffix(".txt").unwrap();
        write!(tmp, "Hello, world!").unwrap();

        let processor = DefaultAttachmentProcessor::new();
        let result = processor
            .process_file(tmp.path().to_str().unwrap())
            .await
            .unwrap();

        assert_eq!(result.attachment_type, AttachmentType::Text);
        assert_eq!(result.extracted_text, Some("Hello, world!".to_string()));
        assert!(result.base64_data.is_none());
        assert_eq!(result.size_bytes, 13);
    }

    #[tokio::test]
    async fn test_process_file_image() {
        let mut tmp = NamedTempFile::with_suffix(".png").unwrap();
        let fake_image_data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic bytes
        tmp.write_all(&fake_image_data).unwrap();

        let processor = DefaultAttachmentProcessor::new();
        let result = processor
            .process_file(tmp.path().to_str().unwrap())
            .await
            .unwrap();

        assert_eq!(result.attachment_type, AttachmentType::Image);
        assert!(result.extracted_text.is_none());
        assert!(result.base64_data.is_some());
        // Base64デコードして元データと一致するか確認
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(result.base64_data.unwrap())
            .unwrap();
        assert_eq!(decoded, fake_image_data);
    }

    #[tokio::test]
    async fn test_process_file_not_found() {
        let processor = DefaultAttachmentProcessor::new();
        let result = processor.process_file("/nonexistent/file.txt").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Attachment(msg) => assert!(msg.contains("見つかりません")),
            _ => panic!("Expected AppError::Attachment"),
        }
    }

    #[tokio::test]
    async fn test_process_file_unsupported_extension() {
        let tmp = NamedTempFile::with_suffix(".exe").unwrap();

        let processor = DefaultAttachmentProcessor::new();
        let result = processor
            .process_file(tmp.path().to_str().unwrap())
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Attachment(msg) => {
                assert!(msg.contains("非対応"));
                assert!(msg.contains("txt"));
            }
            _ => panic!("Expected AppError::Attachment"),
        }
    }

    #[tokio::test]
    async fn test_process_file_size_limit() {
        let mut tmp = NamedTempFile::with_suffix(".txt").unwrap();
        // MAX_FILE_SIZE + 1 バイトのデータを書き込み
        let data = vec![b'a'; (MAX_FILE_SIZE + 1) as usize];
        tmp.write_all(&data).unwrap();

        let processor = DefaultAttachmentProcessor::new();
        let result = processor
            .process_file(tmp.path().to_str().unwrap())
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Attachment(msg) => assert!(msg.contains("上限")),
            _ => panic!("Expected AppError::Attachment"),
        }
    }

    #[tokio::test]
    async fn test_extract_text_from_text_attachment() {
        let processor = DefaultAttachmentProcessor::new();
        let attachment = Attachment {
            id: "test-id".to_string(),
            file_name: "test.txt".to_string(),
            file_path: "/tmp/test.txt".to_string(),
            attachment_type: AttachmentType::Text,
            size_bytes: 5,
            extracted_text: Some("hello".to_string()),
            base64_data: None,
        };

        let text = processor.extract_text(&attachment).await.unwrap();
        assert_eq!(text, "hello");
    }

    #[tokio::test]
    async fn test_extract_text_from_image_fails() {
        let processor = DefaultAttachmentProcessor::new();
        let attachment = Attachment {
            id: "test-id".to_string(),
            file_name: "test.png".to_string(),
            file_path: "/tmp/test.png".to_string(),
            attachment_type: AttachmentType::Image,
            size_bytes: 100,
            extracted_text: None,
            base64_data: Some("base64data".to_string()),
        };

        let result = processor.extract_text(&attachment).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_encode_image_success() {
        let processor = DefaultAttachmentProcessor::new();
        let attachment = Attachment {
            id: "test-id".to_string(),
            file_name: "test.png".to_string(),
            file_path: "/tmp/test.png".to_string(),
            attachment_type: AttachmentType::Image,
            size_bytes: 100,
            extracted_text: None,
            base64_data: Some("aW1hZ2VkYXRh".to_string()),
        };

        let encoded = processor.encode_image(&attachment).await.unwrap();
        assert_eq!(encoded, "aW1hZ2VkYXRh");
    }

    #[tokio::test]
    async fn test_encode_image_non_image_fails() {
        let processor = DefaultAttachmentProcessor::new();
        let attachment = Attachment {
            id: "test-id".to_string(),
            file_name: "test.txt".to_string(),
            file_path: "/tmp/test.txt".to_string(),
            attachment_type: AttachmentType::Text,
            size_bytes: 5,
            extracted_text: Some("hello".to_string()),
            base64_data: None,
        };

        let result = processor.encode_image(&attachment).await;
        assert!(result.is_err());
    }
}
