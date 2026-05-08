//! TTS Connectorのプロパティテスト
//! proptest を使用してTTSリクエストフォーマットの不変条件を検証する。

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use crate::tts::text_splitter::{split_text, SplitConfig};

    use std::path::Path;

    use crate::models::tts::{EmotionParams, TTSConfig, TTSProvider};
    use crate::tts::irodori::IrodoriTTSHandler;
    use crate::tts::voicepeak::VoicePeakHandler;

    // ========================================
    // Arbitrary Strategies
    // ========================================

    /// EmotionParams のストラテジー
    fn arb_emotion_params() -> impl Strategy<Value = EmotionParams> {
        proptest::collection::hash_map(
            prop::sample::select(vec![
                "happy".to_string(),
                "fun".to_string(),
                "angry".to_string(),
                "sad".to_string(),
            ]),
            0i32..100,
            0..=4,
        )
    }

    /// Irodori-TTS用 TTSConfig のストラテジー
    fn arb_irodori_config() -> impl Strategy<Value = TTSConfig> {
        (
            "http://[a-z]{3,10}:[0-9]{4}",
            proptest::option::of("/path/to/[a-z]{3,10}\\.wav"),
            proptest::option::of("[a-zA-Z ]{1,30}"),
        )
            .prop_map(|(base_url, reference_audio_path, caption)| TTSConfig {
                provider: TTSProvider::IrodoriTts,
                base_url: Some(base_url),
                caption_base_url: None,
                reference_audio_base_url: None,
                reference_audio_path,
                caption,
                narrator: None,
                emotion: None,
                speed: None,
                pitch: None,
                irodori_mode: None,
            })
    }

    /// VoicePeak用 TTSConfig のストラテジー
    fn arb_voicepeak_config() -> impl Strategy<Value = TTSConfig> {
        (
            proptest::option::of("[A-Za-z ]{3,20}"),
            proptest::option::of(arb_emotion_params()),
            proptest::option::of(50.0f32..200.0),
            proptest::option::of(-100.0f32..100.0),
        )
            .prop_map(
                |(narrator, emotion, speed, pitch)| TTSConfig {
                    provider: TTSProvider::Voicepeak,
                    base_url: None,
                    caption_base_url: None,
                    reference_audio_base_url: None,
                    reference_audio_path: None,
                    caption: None,
                    narrator,
                    emotion,
                    speed,
                    pitch,
                    irodori_mode: None,
                },
            )
    }

    /// テキスト入力のストラテジー
    fn arb_text() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ]{1,100}"
    }

    // ========================================
    // Property 14: TTS request format correctness per provider
    // ========================================
    // **Validates: Requirements 6.2, 6.4**
    //
    // For any valid TTSConfig, the request body built by the handler SHALL:
    // - For IrodoriTts: always contain "text" field, include "reference_audio_path"
    //   and "caption" when present in config
    // - None fields SHALL NOT appear in the serialized JSON

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        // --- Irodori-TTS ---

        /// Irodori: リクエストボディは常に "text" フィールドを含む
        #[test]
        fn prop_irodori_always_contains_text(
            text in arb_text(),
            config in arb_irodori_config(),
        ) {
            let body = IrodoriTTSHandler::build_request_body(&text, &config);
            let json = serde_json::to_value(&body).unwrap();

            prop_assert_eq!(json["text"].as_str().unwrap(), text.as_str());
        }

        /// Irodori: reference_audio_path が Some の場合、JSONに含まれる
        #[test]
        fn prop_irodori_includes_reference_audio_when_present(
            text in arb_text(),
            base_url in "http://[a-z]{3,10}:[0-9]{4}",
            ref_path in "/path/to/[a-z]{3,10}\\.wav",
            caption in proptest::option::of("[a-zA-Z]{1,20}"),
        ) {
            let config = TTSConfig {
                provider: TTSProvider::IrodoriTts,
                base_url: Some(base_url),
                caption_base_url: None,
                reference_audio_base_url: None,
                reference_audio_path: Some(ref_path.clone()),
                caption,
                narrator: None,
                emotion: None,
                speed: None,
                pitch: None,
                irodori_mode: None,
            };

            let body = IrodoriTTSHandler::build_request_body(&text, &config);
            let json = serde_json::to_value(&body).unwrap();

            prop_assert_eq!(
                json["reference_audio_path"].as_str().unwrap(),
                ref_path.as_str(),
                "reference_audio_path must be present when config has it"
            );
        }

        /// Irodori: caption が Some の場合、JSONに含まれる
        #[test]
        fn prop_irodori_includes_caption_when_present(
            text in arb_text(),
            base_url in "http://[a-z]{3,10}:[0-9]{4}",
            ref_path in proptest::option::of("/path/to/[a-z]{3,10}\\.wav"),
            caption in "[a-zA-Z]{1,20}",
        ) {
            let config = TTSConfig {
                provider: TTSProvider::IrodoriTts,
                base_url: Some(base_url),
                caption_base_url: None,
                reference_audio_base_url: None,
                reference_audio_path: ref_path,
                caption: Some(caption.clone()),
                narrator: None,
                emotion: None,
                speed: None,
                pitch: None,
                irodori_mode: None,
            };

            let body = IrodoriTTSHandler::build_request_body(&text, &config);
            let json = serde_json::to_value(&body).unwrap();

            prop_assert_eq!(
                json["caption"].as_str().unwrap(),
                caption.as_str(),
                "caption must be present when config has it"
            );
        }

        /// Irodori: None フィールドはシリアライズされたJSONに含まれない
        #[test]
        fn prop_irodori_none_fields_absent_in_json(
            text in arb_text(),
            config in arb_irodori_config(),
        ) {
            let body = IrodoriTTSHandler::build_request_body(&text, &config);
            let json_str = serde_json::to_string(&body).unwrap();

            if config.reference_audio_path.is_none() {
                prop_assert!(
                    !json_str.contains("reference_audio_path"),
                    "None reference_audio_path must not appear in JSON"
                );
            }
            if config.caption.is_none() {
                prop_assert!(
                    !json_str.contains("caption"),
                    "None caption must not appear in JSON"
                );
            }
        }
    }

    // ========================================
    // Helper: CLI引数からTTSConfig相当の値をパースする（テスト用）
    // ========================================

    /// CLI引数列から感情文字列をパースしてEmotionParamsに変換
    fn parse_emotion_str(s: &str) -> EmotionParams {
        let mut emotion = EmotionParams::new();

        for pair in s.split(',') {
            let parts: Vec<&str> = pair.splitn(2, '=').collect();
            if parts.len() == 2 {
                let value: i32 = parts[1].parse().unwrap();
                emotion.insert(parts[0].to_string(), value);
            }
        }

        emotion
    }

    /// CLI引数からTTSConfig相当の値をパースする（テスト用）
    fn parse_cli_args(
        args: &[String],
    ) -> (
        Option<String>,
        Option<EmotionParams>,
        Option<i32>,
        Option<i32>,
    ) {
        let mut narrator = None;
        let mut emotion = None;
        let mut speed = None;
        let mut pitch = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--narrator" => {
                    narrator = Some(args[i + 1].clone());
                    i += 2;
                }
                "--emotion" => {
                    emotion = Some(parse_emotion_str(&args[i + 1]));
                    i += 2;
                }
                "--speed" => {
                    speed = Some(args[i + 1].parse().unwrap());
                    i += 2;
                }
                "--pitch" => {
                    pitch = Some(args[i + 1].parse().unwrap());
                    i += 2;
                }
                _ => {
                    i += 1;
                }
            }
        }
        (narrator, emotion, speed, pitch)
    }

    // ========================================
    // Feature: voicepeak-cli-integration
    // Property 1: CLI引数構築のラウンドトリップ
    // ========================================
    // **Validates: Requirements 7.1, 2.3, 2.5, 2.6, 2.7**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// CLI引数構築のラウンドトリップ: build_cli_argsで構築した引数をパースし直すと元の設定値と等価
        #[test]
        fn prop_voicepeak_cli_args_roundtrip(
            config in arb_voicepeak_config(),
        ) {
            let output_path = Path::new("/tmp/test_output.wav");
            let args = VoicePeakHandler::build_cli_args("test text", output_path, &config);

            let (parsed_narrator, parsed_emotion, parsed_speed, parsed_pitch) =
                parse_cli_args(&args);

            // narrator の検証
            prop_assert_eq!(&parsed_narrator, &config.narrator);

            // emotion の検証
            match (&config.emotion, &parsed_emotion) {
                (Some(orig), Some(parsed)) => {
                    // format_emotionが空HashMapでNoneを返す場合は--emotionが省略される
                    // ここに来るのはformat_emotionがSomeを返した場合のみ
                    prop_assert_eq!(parsed, orig);
                }
                (Some(orig), None) => {
                    // 空のHashMapの場合、format_emotionがNoneを返し--emotionが省略される
                    prop_assert!(
                        orig.is_empty(),
                        "emotion was Some but HashMap was empty, so --emotion was omitted"
                    );
                }
                (None, None) => {
                    // 両方None: OK
                }
                (None, Some(_)) => {
                    prop_assert!(false, "parsed emotion should be None when config emotion is None");
                }
            }

            // speed の検証（f32 → i32 変換）
            match config.speed {
                Some(s) => prop_assert_eq!(parsed_speed, Some(s as i32)),
                None => prop_assert_eq!(parsed_speed, None),
            }

            // pitch の検証（f32 → i32 変換）
            match config.pitch {
                Some(p) => prop_assert_eq!(parsed_pitch, Some(p as i32)),
                None => prop_assert_eq!(parsed_pitch, None),
            }
        }
    }

    // ========================================
    // Feature: voicepeak-cli-integration
    // Property 2: 入力テキストの保全
    // ========================================
    // **Validates: Requirements 7.2, 2.1**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// 入力テキストの保全: build_cli_argsの--sayフラグ値が入力テキストと完全一致
        #[test]
        fn prop_voicepeak_text_preservation(
            text in arb_text(),
        ) {
            let config = TTSConfig {
                provider: TTSProvider::Voicepeak,
                base_url: None,
                caption_base_url: None,
                reference_audio_base_url: None,
                reference_audio_path: None,
                caption: None,
                narrator: None,
                emotion: None,
                speed: None,
                pitch: None,
                irodori_mode: None,
            };
            let output_path = Path::new("/tmp/test_output.wav");
            let args = VoicePeakHandler::build_cli_args(&text, output_path, &config);

            // --say の次の引数が入力テキストと完全一致することを検証
            let say_index = args.iter().position(|a| a == "--say")
                .expect("--say flag must be present in args");
            prop_assert_eq!(&args[say_index + 1], &text);
        }
    }

    // ========================================
    // Feature: voicepeak-cli-integration
    // Property 3: 感情パラメータのフォーマット正確性
    // ========================================
    // **Validates: Requirements 7.3, 2.4**

    /// 少なくとも1つのエントリを持つEmotionParamsのストラテジー
    fn arb_emotion_params_with_at_least_one() -> impl Strategy<Value = EmotionParams> {
        proptest::collection::hash_map(
            prop::sample::select(vec![
                "happy".to_string(),
                "fun".to_string(),
                "angry".to_string(),
                "sad".to_string(),
            ]),
            0i32..100,
            1..=4,
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// 感情パラメータのフォーマット正確性: format_emotionが全エントリを含む
        #[test]
        fn prop_voicepeak_emotion_format_correctness(
            emotion in arb_emotion_params_with_at_least_one(),
        ) {
            let result = VoicePeakHandler::format_emotion(&emotion);

            // 少なくとも1つのエントリがあるので、結果はSome
            prop_assert!(result.is_some(), "format_emotion must return Some when HashMap is non-empty");
            let result_str = result.unwrap();

            // パースして検証
            let parsed = parse_emotion_str(&result_str);

            // 全エントリが出力に含まれる
            for (key, value) in &emotion {
                prop_assert_eq!(
                    parsed.get(key),
                    Some(value),
                    "key '{}' mismatch", key
                );
            }

            // 出力のペア数がHashMapのエントリ数と一致
            let pairs: Vec<&str> = result_str.split(',').collect();
            prop_assert_eq!(pairs.len(), emotion.len(),
                "number of pairs must equal number of entries in HashMap");
        }
    }

    // ========================================
    // Feature: tts-playback-flow, Property 4: Emotion parameter validation bounds
    // ========================================
    // **Validates: Requirements 4.3**

    use crate::tts::emotion_generator::EmotionGenerator;

    /// 任意のJSON文字列（ランダムな数値を含む）を生成するストラテジー
    fn arb_emotion_json() -> impl Strategy<Value = String> {
        (
            proptest::option::of(-500i64..999),
            proptest::option::of(-500i64..999),
            proptest::option::of(-500i64..999),
            proptest::option::of(-500i64..999),
            proptest::option::of(-500.0f64..999.0),
            proptest::option::of(-500.0f64..999.0),
        )
            .prop_map(|(happy, fun, angry, sad, speed, pitch)| {
                let mut fields = Vec::new();
                if let Some(v) = happy {
                    fields.push(format!("\"happy\": {}", v));
                }
                if let Some(v) = fun {
                    fields.push(format!("\"fun\": {}", v));
                }
                if let Some(v) = angry {
                    fields.push(format!("\"angry\": {}", v));
                }
                if let Some(v) = sad {
                    fields.push(format!("\"sad\": {}", v));
                }
                if let Some(v) = speed {
                    fields.push(format!("\"speed\": {:.1}", v));
                }
                if let Some(v) = pitch {
                    fields.push(format!("\"pitch\": {:.1}", v));
                }
                format!("{{{}}}", fields.join(", "))
            })
    }

    /// ベースTTSConfig（最小限のデフォルト）を生成するストラテジー
    fn arb_base_tts_config() -> impl Strategy<Value = TTSConfig> {
        Just(TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: None,
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: None,
            caption: None,
            narrator: None,
            emotion: None,
            speed: Some(100.0),
            pitch: Some(0.0),
            irodori_mode: None,
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// 感情パラメータバリデーション: parse_and_validateはエラーを返すか、全値が有効範囲内
        #[test]
        fn prop_emotion_parameter_validation_bounds(
            json_str in arb_emotion_json(),
            base_config in arb_base_tts_config(),
        ) {
            let result = EmotionGenerator::parse_and_validate(&json_str, &base_config);

            match result {
                Err(_) => {
                    // エラーの場合はOK（無効なJSONなど）
                }
                Ok(params) => {
                    // 全感情パラメータ: 0–100
                    for (key, value) in &params.emotion {
                        prop_assert!(
                            *value >= 0 && *value <= 100,
                            "{} {} out of range [0, 100]", key, value
                        );
                    }

                    // speed: 50.0–200.0
                    if let Some(speed) = params.speed {
                        prop_assert!(
                            speed >= 50.0 && speed <= 200.0,
                            "speed {} out of range [50.0, 200.0]", speed
                        );
                    }

                    // pitch: -300.0–300.0
                    if let Some(pitch) = params.pitch {
                        prop_assert!(
                            pitch >= -300.0 && pitch <= 300.0,
                            "pitch {} out of range [-300.0, 300.0]", pitch
                        );
                    }
                }
            }
        }
    }

    // ========================================
    // Feature: tts-playback-flow, Property 5: WAV concatenation correctness
    // ========================================
    // **Validates: Requirements 6.1, 6.2**

    use crate::tts::wav_concat::{concatenate_wav, parse_wav_header};

    /// テスト用の有効なWAVデータを生成するヘルパー
    fn make_wav(channels: u16, sample_rate: u32, bits_per_sample: u16, pcm_data: &[u8]) -> Vec<u8> {
        let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
        let block_align = channels * bits_per_sample / 8;
        let data_size = pcm_data.len() as u32;
        let file_size = 36 + data_size;

        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
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

    /// WAVチャンクリスト（同一フォーマット、1〜5チャンク、各1〜500バイトPCM）のストラテジー
    fn arb_wav_chunks() -> impl Strategy<Value = Vec<Vec<u8>>> {
        // 固定フォーマットパラメータを選択し、1〜5チャンクを生成
        (
            prop::sample::select(vec![1u16, 2]),           // channels
            prop::sample::select(vec![22050u32, 44100, 48000]), // sample_rate
            prop::sample::select(vec![8u16, 16]),          // bits_per_sample
            prop::collection::vec(
                prop::collection::vec(any::<u8>(), 1..=500), // PCM data per chunk
                1..=5,                                       // number of chunks
            ),
        )
            .prop_map(|(channels, sample_rate, bits_per_sample, pcm_datas)| {
                pcm_datas
                    .into_iter()
                    .map(|pcm| make_wav(channels, sample_rate, bits_per_sample, &pcm))
                    .collect()
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// WAV結合正確性: 出力PCMデータが入力PCMデータの順序付き連結と一致
        #[test]
        fn prop_wav_concatenation_correctness(
            chunks in arb_wav_chunks(),
        ) {
            // 各入力チャンクからPCMデータを抽出して期待値を構築
            let mut expected_pcm: Vec<u8> = Vec::new();
            for chunk in &chunks {
                let header = parse_wav_header(chunk).unwrap();
                let pcm_end = header.data_offset + header.data_size;
                expected_pcm.extend_from_slice(&chunk[header.data_offset..pcm_end]);
            }

            // concatenate_wav を呼び出し
            let result = concatenate_wav(&chunks).unwrap();

            // 出力WAVヘッダーをパース
            let output_header = parse_wav_header(&result).unwrap();

            // 出力PCMデータを抽出
            let output_pcm = &result[output_header.data_offset..output_header.data_offset + output_header.data_size];

            // 出力PCMデータが入力PCMデータの順序付き連結と一致することを検証
            prop_assert_eq!(
                output_pcm,
                expected_pcm.as_slice(),
                "Output PCM data must equal ordered concatenation of input PCM data sections"
            );

            // ヘッダーのdata_sizeフィールドが実際のPCMデータ長と一致
            prop_assert_eq!(
                output_header.data_size,
                expected_pcm.len(),
                "Output WAV data_size must equal total PCM data length"
            );

            // 出力フォーマットが入力と一致（最初のチャンクの情報を基準）
            let first_header = parse_wav_header(&chunks[0]).unwrap();
            prop_assert_eq!(output_header.channels, first_header.channels);
            prop_assert_eq!(output_header.sample_rate, first_header.sample_rate);
            prop_assert_eq!(output_header.bits_per_sample, first_header.bits_per_sample);
        }
    }

    // ========================================
    // Feature: tts-playback-flow, Property 1: Text splitter round-trip preservation
    // ========================================
    // **Validates: Requirements 3.4**

    /// 日本語テキスト（文境界・節境界を含む）を生成するカスタムストラテジー
    fn arb_japanese_text_with_boundaries() -> impl Strategy<Value = String> {
        // セグメント: 日本語文字列 + オプションの境界文字
        let segment = (
            prop::collection::vec(
                prop::sample::select(vec![
                    'あ', 'い', 'う', 'え', 'お',
                    'か', 'き', 'く', 'け', 'こ',
                    'さ', 'し', 'す', 'せ', 'そ',
                    'た', 'ち', 'つ', 'て', 'と',
                    'な', 'に', 'ぬ', 'ね', 'の',
                    'は', 'ひ', 'ふ', 'へ', 'ほ',
                    '私', '今', '日', '天', '気',
                ]),
                1..=20,
            ),
            proptest::option::of(prop::sample::select(vec!['。', '！', '？', '、'])),
        )
            .prop_map(|(chars, boundary)| {
                let mut s: String = chars.into_iter().collect();
                if let Some(b) = boundary {
                    s.push(b);
                }
                s
            });

        // 1〜10個のセグメントを結合
        prop::collection::vec(segment, 1..=10)
            .prop_map(|segments| segments.into_iter().collect::<String>())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// テキスト分割のラウンドトリップ保全: 全チャンクを順序通り結合すると元テキストと一致
        #[test]
        fn prop_text_splitter_round_trip_preservation(
            text in arb_japanese_text_with_boundaries(),
            max_chunk_size in 1usize..200,
        ) {
            let config = SplitConfig { max_chunk_size };
            let chunks = split_text(&text, &config);
            let concatenated: String = chunks.into_iter().collect();
            prop_assert_eq!(
                &concatenated, &text,
                "Concatenating all chunks must produce the original text"
            );
        }
    }

    // ========================================
    // Feature: tts-playback-flow, Property 2: Chunk size invariant
    // ========================================
    // **Validates: Requirements 3.5**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// チャンクサイズ不変条件: 全チャンクの文字数が max_chunk_size 以下
        #[test]
        fn prop_text_splitter_chunk_size_invariant(
            text in arb_japanese_text_with_boundaries(),
            max_chunk_size in 1usize..200,
        ) {
            let config = SplitConfig { max_chunk_size };
            let chunks = split_text(&text, &config);
            for chunk in &chunks {
                let len = chunk.chars().count();
                prop_assert!(
                    len <= max_chunk_size,
                    "Chunk '{}' has {} chars, exceeds max_chunk_size {}",
                    chunk, len, max_chunk_size
                );
            }
        }
    }

    // ========================================
    // Feature: tts-playback-flow, Property 3: Sentence boundary preference
    // ========================================
    // **Validates: Requirements 3.2**

    /// 短い文（各文が max_chunk_size 未満）を生成するストラテジー
    /// フォールバック分割が発生しない条件を保証する
    fn arb_short_sentences() -> impl Strategy<Value = (String, usize)> {
        // 1〜8個の短い文を生成（各文は1〜15文字 + 境界文字）
        let sentence = (
            prop::collection::vec(
                prop::sample::select(vec![
                    'あ', 'い', 'う', 'え', 'お',
                    'か', 'き', 'く', 'け', 'こ',
                    'さ', 'し', 'す', 'せ', 'そ',
                    'た', 'ち', 'つ', 'て', 'と',
                    '私', '今', '日', '天', '気',
                ]),
                1..=15,
            ),
            prop::sample::select(vec!['。', '！', '？']),
        )
            .prop_map(|(chars, boundary)| {
                let mut s: String = chars.into_iter().collect();
                s.push(boundary);
                s
            });

        // 2〜8個の文を結合（複数チャンクが生成されるよう最低2文）
        prop::collection::vec(sentence, 2..=8).prop_map(|sentences| {
            // 最長の文の文字数を取得し、max_chunk_size をそれ以上に設定
            let max_sentence_len = sentences.iter().map(|s| s.chars().count()).max().unwrap_or(1);
            // max_chunk_size は最長文以上だが全体テキスト未満にする
            // （複数チャンクに分割されることを保証）
            let total_len: usize = sentences.iter().map(|s| s.chars().count()).sum();
            let text: String = sentences.into_iter().collect();
            // max_chunk_size: 最長文以上、全体テキスト未満
            let max_chunk_size = if total_len > max_sentence_len {
                // 全体より小さく、最長文以上の値を使う
                // 最長文 ≤ max_chunk_size < total_len を保証
                max_sentence_len.max(1)
            } else {
                max_sentence_len
            };
            (text, max_chunk_size)
        })
        // 複数チャンクに分割されることをフィルタで保証
        .prop_filter("must produce multiple chunks", |(text, max_chunk_size)| {
            let config = SplitConfig { max_chunk_size: *max_chunk_size };
            split_text(text, &config).len() > 1
        })
    }

    /// 文境界文字かどうかを判定
    fn ends_with_sentence_boundary(s: &str) -> bool {
        const SENTENCE_BOUNDARIES: &[char] = &['。', '！', '？'];
        s.chars().last().map_or(false, |c| SENTENCE_BOUNDARIES.contains(&c))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// 文境界優先: フォールバック不要時、非最終チャンクは文境界文字で終わる
        #[test]
        fn prop_text_splitter_sentence_boundary_preference(
            (text, max_chunk_size) in arb_short_sentences(),
        ) {
            let config = SplitConfig { max_chunk_size };
            let chunks = split_text(&text, &config);

            // 非最終チャンクが全て文境界文字で終わることを検証
            for (i, chunk) in chunks.iter().enumerate() {
                if i < chunks.len() - 1 {
                    prop_assert!(
                        ends_with_sentence_boundary(chunk),
                        "Non-final chunk [{}] '{}' does not end with sentence boundary (。！？). \
                         max_chunk_size={}, text='{}'",
                        i, chunk, max_chunk_size, text
                    );
                }
            }
        }
    }

    // ========================================
    // Feature: tts-playback-flow, Property 6: Caption combination completeness
    // ========================================
    // **Validates: Requirements 7.2**

    use crate::tts::caption_generator::CaptionGenerator;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// キャプション結合完全性: combine_captionsの結果がベースキャプションと動的キャプションの両方を部分文字列として含む
        #[test]
        fn prop_caption_combination_completeness(
            base_caption in "[^\x00]{1,50}".prop_filter("non-empty", |s| !s.is_empty()),
            dynamic_caption in "[^\x00]{1,50}".prop_filter("non-empty", |s| !s.is_empty()),
        ) {
            let result = CaptionGenerator::combine_captions(&base_caption, &dynamic_caption);

            prop_assert!(
                result.contains(&base_caption),
                "Result '{}' must contain base caption '{}' as substring",
                result, base_caption
            );
            prop_assert!(
                result.contains(&dynamic_caption),
                "Result '{}' must contain dynamic caption '{}' as substring",
                result, dynamic_caption
            );
        }
    }

    // ========================================
    // Feature: app-enhancements-v3, Property 1: IrodoriTTSベースURL解決の優先順位
    // ========================================
    // **Validates: Requirements 4.3, 4.4**

    use crate::models::config::TTSGlobalConfig;
    use crate::models::tts::IrodoriMode;
    use crate::tts::irodori::resolve_irodori_base_url;

    /// IrodoriMode のストラテジー
    fn arb_irodori_mode() -> impl Strategy<Value = Option<IrodoriMode>> {
        prop::sample::select(vec![
            None,
            Some(IrodoriMode::Caption),
            Some(IrodoriMode::ReferenceAudio),
        ])
    }

    /// URL解決テスト用 TTSConfig のストラテジー
    fn arb_tts_config_for_url_resolution() -> impl Strategy<Value = TTSConfig> {
        (
            arb_irodori_mode(),
            proptest::option::of("http://[a-z]{3,8}:[0-9]{4}/base"),
            proptest::option::of("http://[a-z]{3,8}:[0-9]{4}/caption"),
            proptest::option::of("http://[a-z]{3,8}:[0-9]{4}/refaudio"),
        )
            .prop_map(|(irodori_mode, base_url, caption_base_url, reference_audio_base_url)| {
                TTSConfig {
                    provider: TTSProvider::IrodoriTts,
                    base_url,
                    caption_base_url,
                    reference_audio_base_url,
                    reference_audio_path: None,
                    caption: None,
                    narrator: None,
                    emotion: None,
                    speed: None,
                    pitch: None,
                    irodori_mode,
                }
            })
    }

    /// URL解決テスト用 TTSGlobalConfig のストラテジー
    fn arb_tts_global_config_for_url_resolution() -> impl Strategy<Value = TTSGlobalConfig> {
        proptest::option::of("http://[a-z]{3,8}:[0-9]{4}/global")
            .prop_map(|irodori_base_url| TTSGlobalConfig {
                enabled: true,
                voicepeak_path: None,
                timeout_seconds: 60,
                max_chunk_size: 140,
                irodori_base_url,
                irodori_caption_base_url: None,
                irodori_reference_audio_base_url: None,
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        /// IrodoriTTSベースURL解決の優先順位が常に守られることを検証
        /// 優先順位: モード別URL > キャラクター個別共通URL > グローバル設定URL > None
        #[test]
        fn prop_irodori_base_url_resolution_priority(
            char_config in arb_tts_config_for_url_resolution(),
            global_config in arb_tts_global_config_for_url_resolution(),
        ) {
            let result = resolve_irodori_base_url(&char_config, &global_config);

            // 1. モード別URLが存在する場合はそれを返す
            let mode_url = match char_config.irodori_mode {
                Some(IrodoriMode::Caption) => char_config.caption_base_url.clone(),
                Some(IrodoriMode::ReferenceAudio) => char_config.reference_audio_base_url.clone(),
                None => None,
            };

            if mode_url.is_some() {
                prop_assert_eq!(
                    result, mode_url,
                    "When mode-specific URL is set, it must be returned. \
                     mode={:?}, caption_base_url={:?}, reference_audio_base_url={:?}",
                    char_config.irodori_mode, char_config.caption_base_url, char_config.reference_audio_base_url
                );
            }
            // 2. モード別URLがNoneで、base_urlが存在する場合はそれを返す
            else if char_config.base_url.is_some() {
                prop_assert_eq!(
                    result, char_config.base_url,
                    "When mode URL is None and base_url is set, base_url must be returned"
                );
            }
            // 3. base_urlもNoneで、グローバル設定が存在する場合はそれを返す
            else if global_config.irodori_base_url.is_some() {
                prop_assert_eq!(
                    result, global_config.irodori_base_url,
                    "When char URLs are None and global irodori_base_url is set, global must be returned"
                );
            }
            // 4. すべてNoneの場合はNoneを返す
            else {
                prop_assert_eq!(
                    result, None,
                    "When all URLs are None, result must be None"
                );
            }
        }
    }
}
