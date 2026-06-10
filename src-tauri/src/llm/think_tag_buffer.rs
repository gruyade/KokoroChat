/// <think>タグ抽出バッファ
/// ストリーミングチャンクが境界をまたいでタグを分割する場合に対応
pub struct ThinkTagBuffer {
    /// 現在<think>タグ内部にいるかどうか
    inside_think: bool,
    /// タグ検出用の未処理バッファ
    pending: String,
}

impl ThinkTagBuffer {
    pub fn new() -> Self {
        Self {
            inside_think: false,
            pending: String::new(),
        }
    }

    /// 現在 think タグ内部にいるかどうかを返す（デバッグ用）
    pub fn is_inside_think(&self) -> bool {
        self.inside_think
    }

    /// チャンクを処理し、(text_parts, thinking_parts) を返す
    /// text_parts: 通常テキストとして出力すべき部分
    /// thinking_parts: thinking contentとして出力すべき部分
    pub fn process_chunk(&mut self, chunk: &str) -> (Vec<String>, Vec<String>) {
        let mut text_parts: Vec<String> = Vec::new();
        let mut thinking_parts: Vec<String> = Vec::new();

        // pending に前回の未確定バッファがあれば先頭に結合
        let input = if self.pending.is_empty() {
            chunk.to_string()
        } else {
            let combined = format!("{}{}", self.pending, chunk);
            self.pending.clear();
            combined
        };
        let mut remaining: &str = &input;

        while !remaining.is_empty() {
            if self.inside_think {
                // </think> の終了を探す
                if let Some(end_pos) = remaining.find("</think>") {
                    let thinking_text = &remaining[..end_pos];
                    if !thinking_text.is_empty() {
                        thinking_parts.push(thinking_text.to_string());
                    }
                    remaining = &remaining[end_pos + "</think>".len()..];
                    self.inside_think = false;
                } else {
                    // 閉じタグの部分一致チェック: 末尾が "</think>" の prefix かもしれない
                    // "<", "</", "</t", "</th", "</thi", "</thin", "</think" のいずれかで終わる場合のみpendingに
                    let close_tag = "</think>";
                    let mut pending_start = remaining.len();

                    // 末尾から '<' を探し、そこから先が close_tag の prefix か確認
                    if let Some(last_lt) = remaining.rfind('<') {
                        let tail = &remaining[last_lt..];
                        if close_tag.starts_with(tail) {
                            pending_start = last_lt;
                        }
                    }

                    // thinking部分を出力
                    let thinking_text = &remaining[..pending_start];
                    if !thinking_text.is_empty() {
                        thinking_parts.push(thinking_text.to_string());
                    }
                    if pending_start < remaining.len() {
                        self.pending.push_str(&remaining[pending_start..]);
                    }
                    break;
                }
            } else {
                // <think> の開始を探す
                if let Some(start_pos) = remaining.find("<think>") {
                    let text = &remaining[..start_pos];
                    if !text.is_empty() {
                        text_parts.push(text.to_string());
                    }
                    remaining = &remaining[start_pos + "<think>".len()..];
                    self.inside_think = true;
                } else {
                    // 開始タグの部分一致チェック: 末尾が "<think>" の prefix かもしれない
                    // "<", "<t", "<th", "<thi", "<thin", "<think" のいずれかで終わる場合のみpendingに
                    let open_tag = "<think>";
                    let mut pending_start = remaining.len();

                    // 末尾から '<' を探し、そこから先が open_tag の prefix か確認
                    if let Some(last_lt) = remaining.rfind('<') {
                        let tail = &remaining[last_lt..];
                        if open_tag.starts_with(tail) {
                            pending_start = last_lt;
                        }
                    }

                    let text = &remaining[..pending_start];
                    if !text.is_empty() {
                        text_parts.push(text.to_string());
                    }
                    if pending_start < remaining.len() {
                        self.pending.push_str(&remaining[pending_start..]);
                    }
                    break;
                }
            }
        }

        (text_parts, thinking_parts)
    }

    /// ストリーム終了時に未処理バッファを確定
    /// pending に残っている内容を最終出力として返す
    pub fn flush(&mut self) -> (Vec<String>, Vec<String>) {
        let mut text_parts: Vec<String> = Vec::new();
        let mut thinking_parts: Vec<String> = Vec::new();

        if !self.pending.is_empty() {
            if self.inside_think {
                // thinking 中に終了 → pending はthinking contentとして確定
                thinking_parts.push(std::mem::take(&mut self.pending));
            } else {
                // thinking 外で部分一致していたが結局タグではなかった → テキストとして出力
                text_parts.push(std::mem::take(&mut self.pending));
            }
        }

        self.inside_think = false;
        (text_parts, thinking_parts)
    }
}

impl Default for ThinkTagBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_think_tags() {
        let mut buf = ThinkTagBuffer::new();
        let (text, thinking) = buf.process_chunk("Hello world");
        assert_eq!(text, vec!["Hello world"]);
        assert!(thinking.is_empty());
    }

    #[test]
    fn test_simple_think_block() {
        let mut buf = ThinkTagBuffer::new();
        let (text, thinking) = buf.process_chunk("before<think>inner</think>after");
        assert_eq!(text, vec!["before", "after"]);
        assert_eq!(thinking, vec!["inner"]);
    }

    #[test]
    fn test_think_tag_split_across_chunks() {
        let mut buf = ThinkTagBuffer::new();

        // "<think>" が2チャンクにまたがるケース
        let (text1, thinking1) = buf.process_chunk("hello<thi");
        assert_eq!(text1, vec!["hello"]);
        assert!(thinking1.is_empty());

        let (text2, thinking2) = buf.process_chunk("nk>inside</think>world");
        assert!(text2 == vec!["world"]);
        assert_eq!(thinking2, vec!["inside"]);
    }

    #[test]
    fn test_close_tag_split_across_chunks() {
        let mut buf = ThinkTagBuffer::new();

        let (text1, thinking1) = buf.process_chunk("<think>thinking content</thi");
        assert!(text1.is_empty());
        assert_eq!(thinking1, vec!["thinking content"]);

        let (text2, thinking2) = buf.process_chunk("nk>after");
        assert_eq!(text2, vec!["after"]);
        assert!(thinking2.is_empty());
    }

    #[test]
    fn test_multiple_think_blocks() {
        let mut buf = ThinkTagBuffer::new();
        let (text, thinking) = buf.process_chunk("a<think>t1</think>b<think>t2</think>c");
        assert_eq!(text, vec!["a", "b", "c"]);
        assert_eq!(thinking, vec!["t1", "t2"]);
    }

    #[test]
    fn test_flush_with_pending_outside_think() {
        let mut buf = ThinkTagBuffer::new();

        // 末尾が開始タグの部分一致で終わる
        let (text, thinking) = buf.process_chunk("hello<th");
        assert_eq!(text, vec!["hello"]);
        assert!(thinking.is_empty());

        // flush: タグではなかったのでテキストとして出力
        let (ftext, fthinking) = buf.flush();
        assert_eq!(ftext, vec!["<th"]);
        assert!(fthinking.is_empty());
    }

    #[test]
    fn test_flush_with_pending_inside_think() {
        let mut buf = ThinkTagBuffer::new();

        let (text, thinking) = buf.process_chunk("<think>partial thinking</thi");
        assert!(text.is_empty());
        assert_eq!(thinking, vec!["partial thinking"]);

        // flush: thinking中なのでthinking contentとして確定
        let (ftext, fthinking) = buf.flush();
        assert!(ftext.is_empty());
        assert_eq!(fthinking, vec!["</thi"]);
    }

    #[test]
    fn test_unclosed_think_tag_flush() {
        let mut buf = ThinkTagBuffer::new();

        let (text, thinking) = buf.process_chunk("<think>unclosed content");
        assert!(text.is_empty());
        assert_eq!(thinking, vec!["unclosed content"]);

        // ストリーム終了: 未閉じの場合、pendingは空だがinside_thinkのままflush
        let (ftext, fthinking) = buf.flush();
        assert!(ftext.is_empty());
        assert!(fthinking.is_empty());
    }

    #[test]
    fn test_empty_chunk() {
        let mut buf = ThinkTagBuffer::new();
        let (text, thinking) = buf.process_chunk("");
        assert!(text.is_empty());
        assert!(thinking.is_empty());
    }

    #[test]
    fn test_only_think_content() {
        let mut buf = ThinkTagBuffer::new();
        let (text, thinking) = buf.process_chunk("<think>all thinking</think>");
        assert!(text.is_empty());
        assert_eq!(thinking, vec!["all thinking"]);
    }

    #[test]
    fn test_sequential_chunks_without_tags() {
        let mut buf = ThinkTagBuffer::new();
        let (t1, th1) = buf.process_chunk("chunk1");
        let (t2, th2) = buf.process_chunk(" chunk2");
        let (t3, th3) = buf.process_chunk(" chunk3");
        assert_eq!(t1, vec!["chunk1"]);
        assert_eq!(t2, vec![" chunk2"]);
        assert_eq!(t3, vec![" chunk3"]);
        assert!(th1.is_empty());
        assert!(th2.is_empty());
        assert!(th3.is_empty());
    }

    #[test]
    fn test_think_content_spanning_multiple_chunks() {
        let mut buf = ThinkTagBuffer::new();

        let (t1, th1) = buf.process_chunk("<think>first ");
        assert!(t1.is_empty());
        assert_eq!(th1, vec!["first "]);

        let (t2, th2) = buf.process_chunk("second ");
        assert!(t2.is_empty());
        assert_eq!(th2, vec!["second "]);

        let (t3, th3) = buf.process_chunk("third</think>done");
        assert_eq!(t3, vec!["done"]);
        assert_eq!(th3, vec!["third"]);
    }

    #[test]
    fn test_multibyte_char_at_tail_outside_think() {
        // 「」はUTF-8で3バイト。末尾がマルチバイト文字でもpanicしないこと
        let mut buf = ThinkTagBuffer::new();
        let (text, thinking) = buf.process_chunk("「");
        assert_eq!(text, vec!["「"]);
        assert!(thinking.is_empty());
    }

    #[test]
    fn test_multibyte_char_at_tail_inside_think() {
        // thinking内でマルチバイト文字が末尾に来てもpanicしないこと
        let mut buf = ThinkTagBuffer::new();
        let (text, thinking) = buf.process_chunk("<think>思考「");
        assert!(text.is_empty());
        assert_eq!(thinking, vec!["思考「"]);
    }

    #[test]
    fn test_multibyte_mixed_with_partial_tag() {
        // マルチバイト文字の後に部分タグが来るケース
        let mut buf = ThinkTagBuffer::new();
        let (text, thinking) = buf.process_chunk("テスト「」<thi");
        assert_eq!(text, vec!["テスト「」"]);
        assert!(thinking.is_empty());

        let (text2, thinking2) = buf.process_chunk("nk>内容</think>終了");
        assert_eq!(text2, vec!["終了"]);
        assert_eq!(thinking2, vec!["内容"]);
    }
}
