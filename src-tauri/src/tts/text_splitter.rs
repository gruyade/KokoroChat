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

/// テキストを音声合成用チャンクに分割（純粋関数）
///
/// 分割ロジック:
/// 1. 句点（。）、感嘆符（！）、疑問符（？）で分割
/// 2. 単一文がmax_chunk_sizeを超える場合、読点（、）でフォールバック分割
/// 3. それでも超える場合はmax_chunk_size位置で強制分割
pub fn split_text(text: &str, config: &SplitConfig) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    // Step 1: 文境界で分割
    let sentences = split_at_boundaries(text, SENTENCE_BOUNDARIES);

    let mut chunks: Vec<String> = Vec::new();

    for sentence in sentences {
        if sentence.is_empty() {
            continue;
        }

        if sentence.chars().count() <= config.max_chunk_size {
            chunks.push(sentence);
        } else {
            // Step 2: 節境界でフォールバック分割
            let clauses = split_at_boundaries(&sentence, CLAUSE_BOUNDARIES);
            for clause in clauses {
                if clause.is_empty() {
                    continue;
                }
                if clause.chars().count() <= config.max_chunk_size {
                    chunks.push(clause);
                } else {
                    // Step 3: 強制分割
                    let forced = force_split(&clause, config.max_chunk_size);
                    chunks.extend(forced);
                }
            }
        }
    }

    // 空チャンクをフィルタ
    chunks.into_iter().filter(|c| !c.is_empty()).collect()
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
        let config = SplitConfig { max_chunk_size: 140 };
        let result = split_text("", &config);
        assert!(result.is_empty());
    }

    #[test]
    fn test_short_text_no_split() {
        let config = SplitConfig { max_chunk_size: 140 };
        let text = "こんにちは。";
        let result = split_text(text, &config);
        assert_eq!(result, vec!["こんにちは。"]);
    }

    #[test]
    fn test_sentence_boundary_split() {
        let config = SplitConfig { max_chunk_size: 140 };
        let text = "最初の文。次の文。最後の文";
        let result = split_text(text, &config);
        assert_eq!(result, vec!["最初の文。", "次の文。", "最後の文"]);
    }

    #[test]
    fn test_exclamation_and_question_boundaries() {
        let config = SplitConfig { max_chunk_size: 140 };
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
        let concatenated: String = result.into_iter().collect();
        assert_eq!(concatenated, text);
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
