use crate::error::AppError;

/// WAVヘッダー情報
pub struct WavHeader {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub data_offset: usize,
    pub data_size: usize,
}

/// リトルエンディアンでu16を読み取る
fn read_u16_le(data: &[u8], offset: usize) -> Result<u16, AppError> {
    if offset + 2 > data.len() {
        return Err(AppError::Tts("WAV data too short for u16 read".to_string()));
    }
    Ok(u16::from_le_bytes([data[offset], data[offset + 1]]))
}

/// リトルエンディアンでu32を読み取る
fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, AppError> {
    if offset + 4 > data.len() {
        return Err(AppError::Tts("WAV data too short for u32 read".to_string()));
    }
    Ok(u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

/// WAVデータからヘッダー情報をパース
///
/// RIFFヘッダー、fmtチャンク、dataチャンクを探索し、
/// オーディオフォーマット情報とPCMデータの位置を返す。
pub fn parse_wav_header(data: &[u8]) -> Result<WavHeader, AppError> {
    // 最小サイズチェック: RIFF(4) + size(4) + WAVE(4) + fmt (4) + size(4) + format(16) + data(4) + size(4) = 44
    if data.len() < 44 {
        return Err(AppError::Tts("WAV data too short".to_string()));
    }

    // RIFFヘッダー検証
    if &data[0..4] != b"RIFF" {
        return Err(AppError::Tts(
            "Invalid WAV: missing RIFF header".to_string(),
        ));
    }
    if &data[8..12] != b"WAVE" {
        return Err(AppError::Tts(
            "Invalid WAV: missing WAVE format".to_string(),
        ));
    }

    // fmtチャンクを探索
    let mut offset = 12;
    let mut channels: Option<u16> = None;
    let mut sample_rate: Option<u32> = None;
    let mut bits_per_sample: Option<u16> = None;

    while offset + 8 <= data.len() {
        let chunk_id = &data[offset..offset + 4];
        let chunk_size = read_u32_le(data, offset + 4)? as usize;

        if chunk_id == b"fmt " {
            if offset + 8 + 16 > data.len() {
                return Err(AppError::Tts("WAV fmt chunk too short".to_string()));
            }
            let fmt_offset = offset + 8;
            channels = Some(read_u16_le(data, fmt_offset + 2)?);
            sample_rate = Some(read_u32_le(data, fmt_offset + 4)?);
            bits_per_sample = Some(read_u16_le(data, fmt_offset + 14)?);
        }

        if chunk_id == b"data" {
            let data_offset = offset + 8;
            let data_size = chunk_size;

            let channels = channels.ok_or_else(|| {
                AppError::Tts("WAV: data chunk found before fmt chunk".to_string())
            })?;
            let sample_rate = sample_rate.ok_or_else(|| {
                AppError::Tts("WAV: data chunk found before fmt chunk".to_string())
            })?;
            let bits_per_sample = bits_per_sample.ok_or_else(|| {
                AppError::Tts("WAV: data chunk found before fmt chunk".to_string())
            })?;

            return Ok(WavHeader {
                channels,
                sample_rate,
                bits_per_sample,
                data_offset,
                data_size,
            });
        }

        offset += 8 + chunk_size;
        // チャンクサイズが奇数の場合、パディングバイトをスキップ
        if chunk_size % 2 != 0 {
            offset += 1;
        }
    }

    Err(AppError::Tts("WAV: data chunk not found".to_string()))
}

/// 複数WAVデータを結合（同一フォーマット前提）
///
/// 最初のチャンクのヘッダーを基準に、全チャンクのPCMデータを連結して
/// 新しいWAVファイルを生成する。
pub fn concatenate_wav(chunks: &[Vec<u8>]) -> Result<Vec<u8>, AppError> {
    if chunks.is_empty() {
        return Err(AppError::Tts("No WAV chunks to concatenate".to_string()));
    }

    if chunks.len() == 1 {
        return Ok(chunks[0].clone());
    }

    // 最初のチャンクのヘッダーを基準として使用
    let reference_header = parse_wav_header(&chunks[0])?;

    // 全チャンクのPCMデータを収集
    let mut total_pcm_data: Vec<u8> = Vec::new();

    for chunk in chunks {
        let header = parse_wav_header(chunk)?;

        // PCMデータ部分を抽出して追加
        let pcm_end = header.data_offset + header.data_size;
        let actual_end = pcm_end.min(chunk.len());
        if header.data_offset <= chunk.len() {
            total_pcm_data.extend_from_slice(&chunk[header.data_offset..actual_end]);
        }
    }

    let total_pcm_size = total_pcm_data.len() as u32;

    // 新しいWAVファイルを構築
    // 標準WAVヘッダー: RIFF(4) + size(4) + WAVE(4) + fmt (4) + size(4) + fmt_data(16) + data(4) + size(4) = 44 bytes
    let header_size: u32 = 44;
    let file_size = header_size - 8 + total_pcm_size; // RIFF size = file_size - 8

    let byte_rate = reference_header.sample_rate
        * reference_header.channels as u32
        * reference_header.bits_per_sample as u32
        / 8;
    let block_align = reference_header.channels * reference_header.bits_per_sample / 8;

    let mut output = Vec::with_capacity((header_size + total_pcm_size) as usize);

    // RIFF header
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&file_size.to_le_bytes());
    output.extend_from_slice(b"WAVE");

    // fmt chunk
    output.extend_from_slice(b"fmt ");
    output.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size (PCM = 16)
    output.extend_from_slice(&1u16.to_le_bytes()); // audio format (PCM = 1)
    output.extend_from_slice(&reference_header.channels.to_le_bytes());
    output.extend_from_slice(&reference_header.sample_rate.to_le_bytes());
    output.extend_from_slice(&byte_rate.to_le_bytes());
    output.extend_from_slice(&block_align.to_le_bytes());
    output.extend_from_slice(&reference_header.bits_per_sample.to_le_bytes());

    // data chunk
    output.extend_from_slice(b"data");
    output.extend_from_slice(&total_pcm_size.to_le_bytes());
    output.extend_from_slice(&total_pcm_data);

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// テスト用の有効なWAVデータを生成
    fn make_wav(channels: u16, sample_rate: u32, bits_per_sample: u16, pcm_data: &[u8]) -> Vec<u8> {
        let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
        let block_align = channels * bits_per_sample / 8;
        let data_size = pcm_data.len() as u32;
        let file_size = 36 + data_size; // 44 - 8 + data_size

        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        wav.extend_from_slice(pcm_data);
        wav
    }

    #[test]
    fn test_parse_wav_header_valid() {
        let pcm = vec![0u8; 100];
        let wav = make_wav(1, 44100, 16, &pcm);
        let header = parse_wav_header(&wav).unwrap();

        assert_eq!(header.channels, 1);
        assert_eq!(header.sample_rate, 44100);
        assert_eq!(header.bits_per_sample, 16);
        assert_eq!(header.data_offset, 44);
        assert_eq!(header.data_size, 100);
    }

    #[test]
    fn test_parse_wav_header_stereo() {
        let pcm = vec![0u8; 200];
        let wav = make_wav(2, 48000, 16, &pcm);
        let header = parse_wav_header(&wav).unwrap();

        assert_eq!(header.channels, 2);
        assert_eq!(header.sample_rate, 48000);
        assert_eq!(header.bits_per_sample, 16);
        assert_eq!(header.data_size, 200);
    }

    #[test]
    fn test_parse_wav_header_too_short() {
        let data = vec![0u8; 10];
        let result = parse_wav_header(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_wav_header_invalid_riff() {
        let mut wav = make_wav(1, 44100, 16, &[0u8; 10]);
        wav[0..4].copy_from_slice(b"XXXX");
        let result = parse_wav_header(&wav);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_wav_header_invalid_wave() {
        let mut wav = make_wav(1, 44100, 16, &[0u8; 10]);
        wav[8..12].copy_from_slice(b"XXXX");
        let result = parse_wav_header(&wav);
        assert!(result.is_err());
    }

    #[test]
    fn test_concatenate_wav_single_chunk() {
        let pcm = vec![1u8, 2, 3, 4];
        let wav = make_wav(1, 44100, 16, &pcm);
        let result = concatenate_wav(&[wav.clone()]).unwrap();
        assert_eq!(result, wav);
    }

    #[test]
    fn test_concatenate_wav_multiple_chunks() {
        let pcm1 = vec![1u8, 2, 3, 4];
        let pcm2 = vec![5u8, 6, 7, 8];
        let pcm3 = vec![9u8, 10, 11, 12];

        let wav1 = make_wav(1, 44100, 16, &pcm1);
        let wav2 = make_wav(1, 44100, 16, &pcm2);
        let wav3 = make_wav(1, 44100, 16, &pcm3);

        let result = concatenate_wav(&[wav1, wav2, wav3]).unwrap();

        // 結果をパースして検証
        let header = parse_wav_header(&result).unwrap();
        assert_eq!(header.channels, 1);
        assert_eq!(header.sample_rate, 44100);
        assert_eq!(header.bits_per_sample, 16);
        assert_eq!(header.data_size, 12); // 4 + 4 + 4

        // PCMデータが正しく連結されていることを確認
        let pcm_data = &result[header.data_offset..header.data_offset + header.data_size];
        assert_eq!(pcm_data, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    }

    #[test]
    fn test_concatenate_wav_empty_chunks() {
        let result = concatenate_wav(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_concatenate_wav_preserves_format() {
        let pcm1 = vec![0u8; 50];
        let pcm2 = vec![0u8; 30];

        let wav1 = make_wav(2, 48000, 16, &pcm1);
        let wav2 = make_wav(2, 48000, 16, &pcm2);

        let result = concatenate_wav(&[wav1, wav2]).unwrap();
        let header = parse_wav_header(&result).unwrap();

        assert_eq!(header.channels, 2);
        assert_eq!(header.sample_rate, 48000);
        assert_eq!(header.bits_per_sample, 16);
        assert_eq!(header.data_size, 80); // 50 + 30
    }
}
