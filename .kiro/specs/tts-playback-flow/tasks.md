# Implementation Plan: TTS Playback Flow

## Overview

TTS有効時のチャットフローを再設計し、LLM応答完了後にTTS音声生成→テキスト+音声同時配信を実現する。実装はデータモデル拡張→純粋関数モジュール→LLM依存モジュール→オーケストレーター→Chat Engine統合→フロントエンドの順で進め、各ステップでコンパイル確認を行う。

## Tasks

- [x] 1. Data model extensions (IrodoriMode, TTSGlobalConfig, TTSConfig, event payloads)
  - [x] 1.1 Add `IrodoriMode` enum and extend `TTSConfig` in `src-tauri/src/models/tts.rs`
    - Add `IrodoriMode` enum with `Caption` and `ReferenceAudio` variants
    - Add `irodori_mode: Option<IrodoriMode>` field to `TTSConfig`
    - _Requirements: 7.4_
  - [x] 1.2 Extend `TTSGlobalConfig` in `src-tauri/src/models/config.rs`
    - Add `timeout_seconds: u64` field with `#[serde(default = "default_tts_timeout")]` (default: 60)
    - Add `max_chunk_size: usize` field with `#[serde(default = "default_max_chunk_size")]` (default: 140)
    - Add the default value functions
    - _Requirements: 3.5, 5.3_
  - [x] 1.3 Add TTS event payload structs in `src-tauri/src/models/tts.rs`
    - Add `TTSGeneratingEvent { session_id: String }`
    - Add `TTSCompleteEvent { session_id: String, text: String, audio: String }`
    - Add `TTSErrorEvent { session_id: String, text: String, error: String }`
    - All derive `Clone, Serialize`
    - _Requirements: 1.3, 2.1, 5.1_
  - [x] 1.4 Extend frontend TypeScript types
    - Add `IrodoriMode` type to `src/types/tts.ts`
    - Add `irodori_mode?: IrodoriMode` to `TTSConfig` interface
    - Add `TTSCompleteEvent`, `TTSGeneratingEvent`, `TTSErrorEvent` interfaces
    - Extend `TTSGlobalConfig` in `src/types/config.ts` with `timeout_seconds` and `max_chunk_size`
    - _Requirements: 7.4, 2.1_

- [x] 2. Text Splitter module (pure function, independently testable)
  - [x] 2.1 Create `src-tauri/src/tts/text_splitter.rs`
    - Implement `SplitConfig` struct with `max_chunk_size: usize`
    - Implement `split_text(text: &str, config: &SplitConfig) -> Vec<String>`
    - Split logic: sentence boundaries (。！？) → clause boundaries (、) fallback → forced split
    - Filter out empty chunks
    - Register module in `src-tauri/src/tts/mod.rs`
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_
  - [x] 2.2 Write property test: text splitter round-trip preservation
    - **Property 1: Round trip consistency**
    - Concatenating all chunks in order produces the original text
    - Add to `src-tauri/src/tts/property_tests.rs`
    - **Validates: Requirements 3.4**
  - [x] 2.3 Write property test: text splitter chunk size invariant
    - **Property 2: Chunk size invariant**
    - Every chunk length ≤ max_chunk_size
    - **Validates: Requirements 3.5**
  - [x] 2.4 Write property test: text splitter sentence boundary preference
    - **Property 3: Sentence boundary preference**
    - Non-final chunks end with sentence boundary unless fallback was triggered
    - **Validates: Requirements 3.2**

- [x] 3. WAV Concatenator module (pure function, independently testable)
  - [x] 3.1 Create `src-tauri/src/tts/wav_concat.rs`
    - Implement `WavHeader` struct (channels, sample_rate, bits_per_sample, data_offset, data_size)
    - Implement `parse_wav_header(data: &[u8]) -> Result<WavHeader, AppError>`
    - Implement `concatenate_wav(chunks: &[Vec<u8>]) -> Result<Vec<u8>, AppError>`
    - Use first chunk's header as reference, concatenate PCM data sections
    - Register module in `src-tauri/src/tts/mod.rs`
    - _Requirements: 6.1, 6.2_
  - [x] 3.2 Write property test: WAV concatenation correctness
    - **Property 5: WAV concatenation correctness**
    - Output PCM data equals ordered concatenation of input PCM data sections
    - Generate valid WAV headers + random PCM data for testing
    - **Validates: Requirements 6.1, 6.2**

- [x] 4. Checkpoint - Ensure pure function modules compile and tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Emotion Generator module (LLM-dependent)
  - [x] 5.1 Create `src-tauri/src/tts/emotion_generator.rs`
    - Implement `EmotionGenerator` struct
    - Implement `GeneratedEmotionParams` struct (emotion, speed, pitch)
    - Implement `generate()` async method: build prompt → call LLM → parse JSON response
    - Implement `parse_and_validate(json_str, base_config)` for JSON parsing with bounds validation
    - Emotion values: 0–100, speed: 50–200, pitch: -300–300
    - Register module in `src-tauri/src/tts/mod.rs`
    - _Requirements: 4.1, 4.2, 4.3, 4.5_
  - [x] 5.2 Write property test: emotion parameter validation bounds
    - **Property 4: Emotion parameter validation bounds**
    - `parse_and_validate` either returns error OR produces values within valid ranges
    - Generate arbitrary JSON with random numeric values
    - **Validates: Requirements 4.3**
  - [x] 5.3 Write unit tests for Emotion Generator
    - Test LLM failure fallback returns base config defaults
    - Test valid JSON parsing produces correct params
    - Test out-of-range values are clamped or rejected
    - _Requirements: 4.3, 4.4_

- [x] 6. Caption Generator module (LLM-dependent)
  - [x] 6.1 Create `src-tauri/src/tts/caption_generator.rs`
    - Implement `CaptionGenerator` struct
    - Implement `generate()` async method: build prompt → call LLM → return caption string
    - Implement `combine_captions(base_caption, dynamic_caption) -> String`
    - Register module in `src-tauri/src/tts/mod.rs`
    - _Requirements: 7.1, 7.2, 7.7_
  - [x] 6.2 Write property test: caption combination completeness
    - **Property 6: Caption combination completeness**
    - `combine_captions` result contains both base and dynamic captions as substrings
    - **Validates: Requirements 7.2**
  - [x] 6.3 Write unit tests for Caption Generator
    - Test LLM failure fallback returns base caption only
    - Test combine_captions with various input combinations
    - _Requirements: 7.2, 7.7_

- [x] 7. TTS Flow Controller (orchestrator)
  - [x] 7.1 Create `src-tauri/src/tts/flow_controller.rs`
    - Implement `TTSFlowController` struct with `tts_connector`, `llm_client`, `config_manager` fields
    - Implement `TTSResult` struct (audio_data, text)
    - Implement `process()` async method orchestrating:
      1. Determine provider → call EmotionGenerator (VoicePeak) or CaptionGenerator (Irodori caption mode)
      2. Split text via `text_splitter::split_text`
      3. Synthesize each chunk via `TTSConnector`
      4. Concatenate WAV via `wav_concat::concatenate_wav`
    - Implement timeout wrapper using `tokio::time::timeout`
    - Handle fallback: LLM generation failure → use default config
    - Register module in `src-tauri/src/tts/mod.rs`
    - _Requirements: 1.2, 3.6, 4.4, 5.3, 7.3_
  - [x] 7.2 Write unit tests for TTS Flow Controller
    - Test full flow with mocked TTSConnector and LLMClient
    - Test timeout triggers error
    - Test LLM failure fallback uses default config
    - Test Irodori reference_audio mode skips caption generation
    - _Requirements: 5.3, 4.4, 7.3_

- [x] 8. Checkpoint - Ensure all backend modules compile and tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 9. Chat Engine TTS branch integration
  - [x] 9.1 Extend `DefaultChatEngine` to hold TTS dependencies
    - Add `tts_connector: Arc<dyn TTSConnector>` field
    - Add `tts_flow_controller: Option<Arc<TTSFlowController>>` field
    - Update constructor and `AppState` wiring in `src-tauri/src/state.rs` and `src-tauri/src/lib.rs`
    - _Requirements: 1.1, 1.2_
  - [x] 9.2 Implement TTS branch in `send_message`
    - Check `tts_enabled && character.tts_config.is_some()`
    - If TTS enabled: accumulate full LLM response (no streaming chunks emitted)
    - Emit `tts:generating` event after LLM response complete
    - Call `TTSFlowController::process()`
    - On success: emit `tts:complete` event with text + base64 audio
    - On failure: emit `tts:error` event with text + error message
    - If TTS disabled: existing streaming flow unchanged
    - _Requirements: 1.1, 1.2, 1.3, 1.5, 5.1, 5.2_
  - [x] 9.3 Write unit tests for Chat Engine TTS branch
    - Test TTS enabled path emits `tts:generating` then `tts:complete`
    - Test TTS disabled path emits `chat:stream` chunks as before
    - Test TTS failure emits `tts:error` with full text
    - _Requirements: 1.1, 1.5, 5.1_

- [x] 10. Frontend: Chat store and hook extensions
  - [x] 10.1 Extend `ChatState` in `src/stores/chat.store.ts`
    - Add `isTTSGenerating: boolean` state field (default: false)
    - Add `setTTSGenerating(value: boolean)` action
    - Add `finishWithAudio(text: string, audio: string)` action
    - _Requirements: 2.1, 2.4_
  - [x] 10.2 Extend `useChat` hook with TTS event listeners
    - Add `tts:generating` listener → set `isTTSGenerating = true`
    - Add `tts:complete` listener → call `finishWithAudio`, set `isTTSGenerating = false`, trigger audio playback
    - Add `tts:error` listener → display text immediately, show toast notification, set `isTTSGenerating = false`
    - _Requirements: 1.4, 2.1, 2.4, 5.2_
  - [x] 10.3 Extend `useAudio` hook for `tts:complete` audio playback
    - Replace single `tts:audio` listener with `tts:complete` event handling
    - Play audio from `TTSCompleteEvent.audio` field (base64 WAV)
    - _Requirements: 1.4, 6.3_

- [x] 11. Frontend: TTS loading indicator UI
  - [x] 11.1 Add TTS loading indicator to `ChatView.tsx`
    - Show distinct loading indicator when `isTTSGenerating` is true
    - Ensure it is visually different from the streaming indicator
    - Do NOT display partial text during TTS generation
    - _Requirements: 2.2, 2.3_

- [x] 12. Frontend: Character form Irodori mode selection
  - [x] 12.1 Add Irodori mode toggle to `CharacterForm.tsx`
    - Add radio/select for `irodori_mode`: "caption" vs "reference_audio"
    - When caption mode: show base voice caption field (required)
    - When reference_audio mode: show reference audio path field, hide caption fields
    - _Requirements: 7.4, 7.5, 7.6_

- [x] 13. Final checkpoint - Full compilation and test verification
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental compilation verification
- Property tests validate universal correctness properties from the design document
- Unit tests validate specific examples, edge cases, and fallback behavior
- Pure function modules (text_splitter, wav_concat) are implemented first for early testability
- Chat Engine integration is deferred until all TTS modules are independently verified
