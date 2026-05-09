/// テキスト分割設定
pub struct SplitConfig {
    /// 最大チャンクサイズ（文字数）。デフォルト: 140
    pub max_chunk_size: usize,
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self {
            max_chunk_size: 140,
        }
    }
}

/// 文境界文字
const SENTENCE_BOUNDARIES: &[char] = &['。', '！', '？'];

/// 節境界文字
const CLAUSE_BOUNDARIES: &[char] = &['、'];

/// TTS用テキストサニタイズ
///
/// LLMの応答から地の文（《》内）・絵文字・改行を除外する。
/// 《》で囲まれた部分を除去し、残りを音声合成に渡す。
pub fn sanitize_for_tts(text: &str) -> String {
    // 《》内の地の文を除去
    let without_narration = remove_narration_markers(text);
    // 絵文字・改行除去
    remove_emoji_and_newlines(&without_narration)
}

/// 《》で囲まれた地の文を除去
fn remove_narration_markers(text: &str) -> String {
    let mut result = String::new();
    let mut in_narration = false;

    for ch in text.chars() {
        match ch {
            '《' => in_narration = true,
            '》' => in_narration = false,
            _ => {
                if !in_narration {
                    result.push(ch);
                }
            }
        }
    }
    result
}

/// 絵文字とUnicode絵文字を除去し、改行をスペースに置換
fn remove_emoji_and_newlines(text: &str) -> String {
    text.chars()
        .filter_map(|ch| {
            if ch == '\n' || ch == '\r' {
                Some(' ')
            } else if is_emoji(ch) {
                None
            } else {
                Some(ch)
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Unicode絵文字判定（主要な絵文字範囲）
fn is_emoji(ch: char) -> bool {
    let cp = ch as u32;
    // Emoticons, Dingbats, Symbols, Transport, Misc Symbols, Supplemental Symbols
    (0x1F600..=0x1F64F).contains(&cp) ||  // Emoticons
    (0x1F300..=0x1F5FF).contains(&cp) ||  // Misc Symbols and Pictographs
    (0x1F680..=0x1F6FF).contains(&cp) ||  // Transport and Map
    (0x1F1E0..=0x1F1FF).contains(&cp) ||  // Flags
    (0x2600..=0x26FF).contains(&cp) ||    // Misc Symbols
    (0x2700..=0x27BF).contains(&cp) ||    // Dingbats
    (0xFE00..=0xFE0F).contains(&cp) ||    // Variation Selectors
    (0x200D..=0x200D).contains(&cp) ||    // Zero Width Joiner
    (0x1F900..=0x1F9FF).contains(&cp) ||  // Supplemental Symbols
    (0x1FA00..=0x1FA6F).contains(&cp) ||  // Chess Symbols
    (0x1FA70..=0x1FAFF).contains(&cp) // Symbols Extended-A
}

/// テキストを音声合成用チャンクに分割（純粋関数）
///
/// 分割ロジック:
/// - テキストから絵文字を除去
/// - 文境界（。！？）で分割
/// - 隣接する文をmax_chunk_size以内でスペース区切りで結合
/// - 単一文がmax_chunk_sizeを超える場合、読点（、）でフォールバック分割
/// - それでも超える場合はmax_chunk_size位置で強制分割
pub fn split_text(text: &str, config: &SplitConfig) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    // 絵文字除去（文末記号・改行は保持）
    let cleaned: String = text.chars().filter(|ch| !is_emoji(*ch)).collect();

    // 改行をスペースに置換
    let cleaned = cleaned.replace('\n', " ").replace('\r', "");

    // テキスト全体がmax_chunk_size以下なら分割不要
    if cleaned.chars().count() <= config.max_chunk_size {
        let trimmed = cleaned.trim().to_string();
        if trimmed.is_empty() {
            return Vec::new();
        }
        return vec![trimmed];
    }

    // Step 1: 文境界で分割
    let sentences = split_at_boundaries(&cleaned, SENTENCE_BOUNDARIES);

    // Step 2: 各文を処理（長い文は節境界/強制分割）
    let mut segments: Vec<String> = Vec::new();

    for sentence in sentences {
        let trimmed = sentence.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.chars().count() <= config.max_chunk_size {
            segments.push(trimmed);
        } else {
            // 節境界でフォールバック分割
            let clauses = split_at_boundaries(&trimmed, CLAUSE_BOUNDARIES);
            for clause in clauses {
                let ct = clause.trim().to_string();
                if ct.is_empty() {
                    continue;
                }
                if ct.chars().count() <= config.max_chunk_size {
                    segments.push(ct);
                } else {
                    // 強制分割
                    let forced = force_split(&ct, config.max_chunk_size);
                    segments.extend(forced);
                }
            }
        }
    }

    // Step 3: 隣接セグメントをmax_chunk_size以内でスペース区切りで結合
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();

    for segment in segments {
        if current.is_empty() {
            current = segment;
        } else {
            // スペース1文字分を加味して結合可能か判定
            let combined_len = current.chars().count() + 1 + segment.chars().count();
            if combined_len <= config.max_chunk_size {
                current.push(' ');
                current.push_str(&segment);
            } else {
                chunks.push(current);
                current = segment;
            }
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

/// 指定された境界文字で分割（境界文字は前のセグメントの末尾に含める）
fn split_at_boundaries(text: &str, boundaries: &[char]) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if boundaries.contains(&ch) {
            segments.push(current);
            current = String::new();
        }
    }

    // 残りがあれば追加
    if !current.is_empty() {
        segments.push(current);
    }

    segments
}

/// max_chunk_size文字位置で強制分割
fn force_split(text: &str, max_chunk_size: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut chunks: Vec<String> = Vec::new();
    let mut start = 0;

    while start < chars.len() {
        let end = (start + max_chunk_size).min(chars.len());
        let chunk: String = chars[start..end].iter().collect();
        if !chunk.is_empty() {
            chunks.push(chunk);
        }
        start = end;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_text() {
        let config = SplitConfig {
            max_chunk_size: 140,
        };
        let result = split_text("", &config);
        assert!(result.is_empty());
    }

    #[test]
    fn test_short_text_no_split() {
        let config = SplitConfig {
            max_chunk_size: 140,
        };
        let text = "こんにちは。";
        let result = split_text(text, &config);
        assert_eq!(result, vec!["こんにちは。"]);
    }

    #[test]
    fn test_sentence_boundary_split() {
        let config = SplitConfig {
            max_chunk_size: 5,
        };
        let text = "最初の文。次の文。最後の文";
        let result = split_text(text, &config);
        assert_eq!(result, vec!["最初の文。", "次の文。", "最後の文"]);
    }

    #[test]
    fn test_exclamation_and_question_boundaries() {
        let config = SplitConfig {
            max_chunk_size: 5,
        };
        let text = "すごい！本当？そうだよ。";
        let result = split_text(text, &config);
        assert_eq!(result, vec!["すごい！", "本当？", "そうだよ。"]);
    }

    #[test]
    fn test_clause_boundary_fallback() {
        let config = SplitConfig { max_chunk_size: 10 };
        // "あいうえお、かきくけこ" = 11文字 > 10、読点で分割
        let text = "あいうえお、かきくけこ";
        let result = split_text(text, &config);
        assert_eq!(result, vec!["あいうえお、", "かきくけこ"]);
    }

    #[test]
    fn test_forced_split() {
        let config = SplitConfig { max_chunk_size: 5 };
        // 10文字、境界なし → 5文字ずつ強制分割
        let text = "あいうえおかきくけこ";
        let result = split_text(text, &config);
        assert_eq!(result, vec!["あいうえお", "かきくけこ"]);
    }

    #[test]
    fn test_round_trip_preservation() {
        let config = SplitConfig { max_chunk_size: 10 };
        let text = "最初の文。次の文、長い節がある。最後";
        let result = split_text(text, &config);
        // 分割後の各チャンクを結合すると元テキストの全文字が保持される
        // （Step 3の結合でスペースが挿入される場合があるため、スペースを除去して比較）
        let concatenated: String = result.into_iter().collect::<String>().replace(' ', "");
        assert_eq!(concatenated, text.replace(' ', ""));
    }

    #[test]
    fn test_chunk_size_invariant() {
        let config = SplitConfig { max_chunk_size: 5 };
        let text = "あいうえおかきくけこさしすせそ。たちつてと";
        let result = split_text(text, &config);
        for chunk in &result {
            assert!(
                chunk.chars().count() <= config.max_chunk_size,
                "Chunk '{}' exceeds max_chunk_size {}",
                chunk,
                config.max_chunk_size
            );
        }
    }
}
