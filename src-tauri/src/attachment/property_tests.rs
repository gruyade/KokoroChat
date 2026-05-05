//! ファイル添付のプロパティテスト
//! proptest を使用して AttachmentProcessor の不変条件を検証する。
//!
//! **Validates: Requirements 10.2, 10.5, 10.6, 10.8**

#[cfg(test)]
mod tests {
    use std::io::Write;

    use base64::Engine as _;
    use proptest::prelude::*;
    use tempfile::NamedTempFile;

    use crate::attachment::processor::DefaultAttachmentProcessor;
    use crate::attachment::AttachmentProcessor;
    use crate::models::attachment::{AttachmentType, MAX_FILE_SIZE};

    // ========================================
    // ストラテジー
    // ========================================

    /// サポート対象のテキスト拡張子
    fn text_extension() -> impl Strategy<Value = &'static str> {
        prop_oneof![Just("txt"), Just("md"), Just("csv"),]
    }

    /// サポート対象の画像拡張子
    fn image_extension() -> impl Strategy<Value = &'static str> {
        prop_oneof![Just("png"), Just("jpg"), Just("webp"),]
    }

    /// サポート対象の全拡張子
    fn supported_extension() -> impl Strategy<Value = &'static str> {
        prop_oneof![
            Just("txt"),
            Just("md"),
            Just("csv"),
            Just("pdf"),
            Just("png"),
            Just("jpg"),
            Just("webp"),
        ]
    }

    /// 非サポート拡張子
    fn unsupported_extension() -> impl Strategy<Value = &'static str> {
        prop_oneof![
            Just("exe"),
            Just("zip"),
            Just("docx"),
            Just("mp3"),
            Just("avi"),
            Just("bin"),
            Just("dll"),
            Just("iso"),
        ]
    }

    /// MAX_FILE_SIZEを超えるファイルサイズ（10MB + 1 〜 10MB + 1KB）
    fn oversized_file_size() -> impl Strategy<Value = usize> {
        (MAX_FILE_SIZE as usize + 1)..=(MAX_FILE_SIZE as usize + 1024)
    }

    /// MAX_FILE_SIZE以下のファイルサイズ（1バイト〜1KB）
    /// テスト実行速度のため小さめに制限
    fn valid_file_size() -> impl Strategy<Value = usize> {
        1usize..=1024
    }

    /// テキストファイル用の非空コンテンツ
    fn text_content() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ,.!?\n]{1,200}"
    }

    /// 画像ファイル用のバイナリコンテンツ（最低1バイト）
    fn image_content() -> impl Strategy<Value = Vec<u8>> {
        proptest::collection::vec(any::<u8>(), 1..=256)
    }

    // ========================================
    // Property 18: Attachment file size validation
    // ========================================
    //
    // **Validates: Requirements 10.5**
    //
    // For any file with size > MAX_FILE_SIZE (10MB), process_file SHALL return
    // an error. For any file with size <= MAX_FILE_SIZE, process_file SHALL NOT
    // return a size error.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]

        #[test]
        fn prop_oversized_file_returns_error(
            size in oversized_file_size(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // サイズ超過のテキストファイルを作成
                let mut tmp = NamedTempFile::with_suffix(".txt").unwrap();
                let data = vec![b'a'; size];
                tmp.write_all(&data).unwrap();
                tmp.flush().unwrap();

                let processor = DefaultAttachmentProcessor::new();
                let result = processor
                    .process_file(tmp.path().to_str().unwrap())
                    .await;

                // エラーが返ること
                prop_assert!(
                    result.is_err(),
                    "File with size {} (> MAX_FILE_SIZE={}) should return error",
                    size,
                    MAX_FILE_SIZE
                );

                // エラーメッセージにサイズ上限に関する内容が含まれる
                let err_msg = format!("{}", result.unwrap_err());
                prop_assert!(
                    err_msg.contains("上限") || err_msg.contains("10MB"),
                    "Error message should mention size limit, got: {}",
                    err_msg
                );

                Ok(())
            })?;
        }

        #[test]
        fn prop_valid_size_text_file_succeeds(
            size in valid_file_size(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // サイズ以内のテキストファイルを作成
                let mut tmp = NamedTempFile::with_suffix(".txt").unwrap();
                let data = vec![b'a'; size];
                tmp.write_all(&data).unwrap();
                tmp.flush().unwrap();

                let processor = DefaultAttachmentProcessor::new();
                let result = processor
                    .process_file(tmp.path().to_str().unwrap())
                    .await;

                // 成功すること
                prop_assert!(
                    result.is_ok(),
                    "File with size {} (<= MAX_FILE_SIZE={}) should succeed, got error: {:?}",
                    size,
                    MAX_FILE_SIZE,
                    result.err()
                );

                // サイズが正しく記録されている
                let attachment = result.unwrap();
                prop_assert_eq!(
                    attachment.size_bytes,
                    size as u64,
                    "Recorded size should match actual file size"
                );

                Ok(())
            })?;
        }

        #[test]
        fn prop_valid_size_image_file_succeeds(
            content in image_content(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut tmp = NamedTempFile::with_suffix(".png").unwrap();
                tmp.write_all(&content).unwrap();
                tmp.flush().unwrap();

                let processor = DefaultAttachmentProcessor::new();
                let result = processor
                    .process_file(tmp.path().to_str().unwrap())
                    .await;

                prop_assert!(
                    result.is_ok(),
                    "Image file with size {} should succeed, got error: {:?}",
                    content.len(),
                    result.err()
                );

                let attachment = result.unwrap();
                prop_assert_eq!(
                    attachment.size_bytes,
                    content.len() as u64,
                    "Recorded size should match actual file size"
                );

                Ok(())
            })?;
        }
    }

    // ========================================
    // Property 19: Attachment type detection correctness
    // ========================================
    //
    // **Validates: Requirements 10.2, 10.6**
    //
    // For any file path with a supported extension, the detected AttachmentType
    // SHALL match the expected type for that extension. For unsupported extensions,
    // the processor SHALL return an error.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_text_extension_detected_as_text(
            ext in text_extension(),
            content in text_content(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let suffix = format!(".{}", ext);
                let mut tmp = NamedTempFile::with_suffix(&suffix).unwrap();
                tmp.write_all(content.as_bytes()).unwrap();
                tmp.flush().unwrap();

                let processor = DefaultAttachmentProcessor::new();
                let result = processor
                    .process_file(tmp.path().to_str().unwrap())
                    .await;

                prop_assert!(result.is_ok(), "Text file .{} should process successfully", ext);

                let attachment = result.unwrap();
                prop_assert!(
                    attachment.attachment_type == AttachmentType::Text,
                    "Extension .{} should be detected as Text, got {:?}",
                    ext,
                    attachment.attachment_type
                );

                Ok(())
            })?;
        }

        #[test]
        fn prop_image_extension_detected_as_image(
            ext in image_extension(),
            content in image_content(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let suffix = format!(".{}", ext);
                let mut tmp = NamedTempFile::with_suffix(&suffix).unwrap();
                tmp.write_all(&content).unwrap();
                tmp.flush().unwrap();

                let processor = DefaultAttachmentProcessor::new();
                let result = processor
                    .process_file(tmp.path().to_str().unwrap())
                    .await;

                prop_assert!(result.is_ok(), "Image file .{} should process successfully", ext);

                let attachment = result.unwrap();
                prop_assert!(
                    attachment.attachment_type == AttachmentType::Image,
                    "Extension .{} should be detected as Image, got {:?}",
                    ext,
                    attachment.attachment_type
                );

                Ok(())
            })?;
        }

        #[test]
        fn prop_unsupported_extension_returns_error(
            ext in unsupported_extension(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let suffix = format!(".{}", ext);
                let tmp = NamedTempFile::with_suffix(&suffix).unwrap();

                let processor = DefaultAttachmentProcessor::new();
                let result = processor
                    .process_file(tmp.path().to_str().unwrap())
                    .await;

                prop_assert!(
                    result.is_err(),
                    "Unsupported extension .{} should return error",
                    ext
                );

                // エラーメッセージに対応形式の情報が含まれる
                let err_msg = format!("{}", result.unwrap_err());
                prop_assert!(
                    err_msg.contains("非対応") || err_msg.contains("txt"),
                    "Error should mention unsupported format or list supported ones, got: {}",
                    err_msg
                );

                Ok(())
            })?;
        }

        #[test]
        fn prop_is_supported_matches_process_result(
            ext in supported_extension(),
        ) {
            let processor = DefaultAttachmentProcessor::new();
            let file_path = format!("/tmp/test_file.{}", ext);

            // is_supported がtrueを返す拡張子は全てサポート対象
            prop_assert!(
                processor.is_supported(&file_path),
                "Extension .{} should be reported as supported",
                ext
            );
        }

        #[test]
        fn prop_is_supported_false_for_unsupported(
            ext in unsupported_extension(),
        ) {
            let processor = DefaultAttachmentProcessor::new();
            let file_path = format!("/tmp/test_file.{}", ext);

            prop_assert!(
                !processor.is_supported(&file_path),
                "Extension .{} should NOT be reported as supported",
                ext
            );
        }
    }

    // ========================================
    // Property 20: Attachment round-trip persistence
    // ========================================
    //
    // **Validates: Requirements 10.8**
    //
    // For any successfully processed Attachment, the extracted_text (for text/pdf)
    // or base64_data (for images) SHALL be non-None and non-empty.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_text_attachment_has_extracted_text(
            ext in text_extension(),
            content in text_content(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let suffix = format!(".{}", ext);
                let mut tmp = NamedTempFile::with_suffix(&suffix).unwrap();
                tmp.write_all(content.as_bytes()).unwrap();
                tmp.flush().unwrap();

                let processor = DefaultAttachmentProcessor::new();
                let result = processor
                    .process_file(tmp.path().to_str().unwrap())
                    .await;

                prop_assert!(result.is_ok(), "Text file should process successfully");

                let attachment = result.unwrap();

                // extracted_textがSome かつ非空
                prop_assert!(
                    attachment.extracted_text.is_some(),
                    "Text attachment should have extracted_text"
                );
                prop_assert!(
                    !attachment.extracted_text.as_ref().unwrap().is_empty(),
                    "extracted_text should not be empty"
                );

                // base64_dataはNone
                prop_assert!(
                    attachment.base64_data.is_none(),
                    "Text attachment should NOT have base64_data"
                );

                // extracted_textの内容が元のコンテンツと一致
                prop_assert_eq!(
                    attachment.extracted_text.as_ref().unwrap(),
                    &content,
                    "extracted_text should match original content"
                );

                Ok(())
            })?;
        }

        #[test]
        fn prop_image_attachment_has_base64_data(
            ext in image_extension(),
            content in image_content(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let suffix = format!(".{}", ext);
                let mut tmp = NamedTempFile::with_suffix(&suffix).unwrap();
                tmp.write_all(&content).unwrap();
                tmp.flush().unwrap();

                let processor = DefaultAttachmentProcessor::new();
                let result = processor
                    .process_file(tmp.path().to_str().unwrap())
                    .await;

                prop_assert!(result.is_ok(), "Image file should process successfully");

                let attachment = result.unwrap();

                // base64_dataがSome かつ非空
                prop_assert!(
                    attachment.base64_data.is_some(),
                    "Image attachment should have base64_data"
                );
                prop_assert!(
                    !attachment.base64_data.as_ref().unwrap().is_empty(),
                    "base64_data should not be empty"
                );

                // extracted_textはNone
                prop_assert!(
                    attachment.extracted_text.is_none(),
                    "Image attachment should NOT have extracted_text"
                );

                // Base64デコードして元データと一致するか確認
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(attachment.base64_data.as_ref().unwrap())
                    .unwrap();
                prop_assert_eq!(
                    decoded,
                    content,
                    "Decoded base64 should match original image bytes"
                );

                Ok(())
            })?;
        }

        #[test]
        fn prop_attachment_metadata_correctness(
            ext in text_extension(),
            content in text_content(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let suffix = format!(".{}", ext);
                let mut tmp = NamedTempFile::with_suffix(&suffix).unwrap();
                tmp.write_all(content.as_bytes()).unwrap();
                tmp.flush().unwrap();

                let processor = DefaultAttachmentProcessor::new();
                let result = processor
                    .process_file(tmp.path().to_str().unwrap())
                    .await;

                prop_assert!(result.is_ok());
                let attachment = result.unwrap();

                // IDが非空
                prop_assert!(
                    !attachment.id.is_empty(),
                    "Attachment ID should not be empty"
                );

                // file_nameが非空
                prop_assert!(
                    !attachment.file_name.is_empty(),
                    "file_name should not be empty"
                );

                // file_pathが元のパスと一致
                prop_assert_eq!(
                    &attachment.file_path,
                    tmp.path().to_str().unwrap(),
                    "file_path should match input path"
                );

                // size_bytesが正しい
                prop_assert_eq!(
                    attachment.size_bytes,
                    content.len() as u64,
                    "size_bytes should match content length"
                );

                Ok(())
            })?;
        }
    }
}
