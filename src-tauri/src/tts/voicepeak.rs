// VoicePeak CLI実装

use std::path::Path;

use crate::error::AppError;
use crate::models::tts::{EmotionParams, TTSConfig};

/// VoicePeak CLIハンドラ
pub struct VoicePeakHandler;

impl VoicePeakHandler {
    pub fn new() -> Self {
        Self
    }

    /// TTSConfigからCLI引数を構築（純粋関数）
    pub fn build_cli_args(text: &str, output_path: &Path, config: &TTSConfig) -> Vec<String> {
        let mut args = Vec::new();

        // --say は常に含まれる（必須）
        args.push("--say".to_string());
        args.push(text.to_string());

        // --out は常に含まれる（必須）
        args.push("--out".to_string());
        args.push(output_path.to_string_lossy().to_string());

        // --narrator は config.narrator が Some の場合のみ
        if let Some(ref narrator) = config.narrator {
            args.push("--narrator".to_string());
            args.push(narrator.clone());
        }

        // --emotion は config.emotion が Some かつ少なくとも1つのフィールドが Some の場合のみ
        if let Some(ref emotion) = config.emotion {
            if let Some(emotion_str) = Self::format_emotion(emotion) {
                args.push("--emotion".to_string());
                args.push(emotion_str);
            }
        }

        // --speed は config.speed が Some の場合、整数に変換
        if let Some(speed) = config.speed {
            args.push("--speed".to_string());
            args.push((speed as i32).to_string());
        }

        // --pitch は config.pitch が Some の場合、整数に変換
        if let Some(pitch) = config.pitch {
            args.push("--pitch".to_string());
            args.push((pitch as i32).to_string());
        }

        args
    }

    /// 感情パラメータを "--emotion" フォーマット文字列に変換
    /// 空のHashMapの場合 None を返す
    pub fn format_emotion(emotion: &EmotionParams) -> Option<String> {
        if emotion.is_empty() {
            return None;
        }
        let parts: Vec<String> = emotion.iter()
            .map(|(key, value)| format!("{}={}", key, value))
            .collect();
        Some(parts.join(","))
    }

    /// 音声合成: CLIプロセス実行 → WAVファイル読み取り
    /// `executable_path`: 全体設定から渡されるVoicePeak CLIパス（None時は"voicepeak"）
    pub async fn synthesize(&self, text: &str, config: &TTSConfig, executable_path: Option<&str>) -> Result<Vec<u8>, AppError> {
        // VoicePeakはtempディレクトリへの出力でエラーになる場合があるため、
        // 実行ファイルと同じディレクトリに出力する
        let executable = executable_path.unwrap_or("voicepeak");
        let output_dir = std::path::Path::new(executable)
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let output_filename = format!("tts_output_{}.wav", uuid::Uuid::new_v4().simple());
        let tmp_path = output_dir.join(&output_filename);

        let args = Self::build_cli_args(text, &tmp_path, config);

        println!("[VoicePeak] Executing: {} {}", executable, args.iter().map(|a| format!("\"{}\"", a)).collect::<Vec<_>>().join(" "));
        println!("[VoicePeak] Output path: {}", tmp_path.display());

        let output = tokio::process::Command::new(executable)
            .args(&args)
            .output()
            .await
            .map_err(|e| AppError::Tts(format!("VoicePeak executable not found: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("[VoicePeak] CLI failed: {}", stderr);
            // 失敗時もファイルがあれば削除
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(AppError::Tts(format!(
                "VoicePeak CLI failed (exit code {}): {}",
                output.status.code().unwrap_or(-1),
                stderr
            )));
        }

        println!("[VoicePeak] CLI succeeded, reading output file...");
        let audio_data = tokio::fs::read(&tmp_path)
            .await
            .map_err(|e| AppError::Tts(format!("Failed to read VoicePeak output: {}", e)))?;

        println!("[VoicePeak] Read {} bytes from output file", audio_data.len());

        // 読み取り後にファイル削除
        let _ = tokio::fs::remove_file(&tmp_path).await;

        Ok(audio_data)
    }

    /// 接続テスト: 短いテキストでCLI実行確認
    /// `executable_path`: 全体設定から渡されるVoicePeak CLIパス（None時は"voicepeak"）
    pub async fn test_connection(&self, config: &TTSConfig, executable_path: Option<&str>) -> Result<(), AppError> {
        let executable = executable_path.unwrap_or("voicepeak");
        let output_dir = std::path::Path::new(executable)
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let output_filename = format!("tts_test_{}.wav", uuid::Uuid::new_v4().simple());
        let tmp_path = output_dir.join(&output_filename);

        let args = Self::build_cli_args("テスト", &tmp_path, config);

        let output = tokio::process::Command::new(executable)
            .args(&args)
            .output()
            .await
            .map_err(|e| AppError::Tts(format!("VoicePeak executable not found: {}", e)))?;

        // テスト後にファイル削除
        let _ = tokio::fs::remove_file(&tmp_path).await;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Tts(format!(
                "VoicePeak CLI failed (exit code {}): {}",
                output.status.code().unwrap_or(-1),
                stderr
            )));
        }

        Ok(())
    }

    /// ナレーターの利用可能な感情リストを取得
    /// コマンド: voicepeak --list-emotion "ナレーター名"
    pub async fn list_emotions(executable_path: Option<&str>, narrator: &str) -> Result<Vec<String>, AppError> {
        let executable = executable_path.unwrap_or("voicepeak");
        let output = tokio::process::Command::new(executable)
            .args(&["--list-emotion", narrator])
            .output()
            .await
            .map_err(|e| AppError::Tts(format!("VoicePeak executable not found: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Tts(format!("VoicePeak --list-emotion failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let emotions: Vec<String> = stdout.lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        Ok(emotions)
    }
}
