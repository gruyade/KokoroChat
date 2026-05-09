# Requirements Document

## Introduction

TTS（Text-to-Speech）有効時のチャットフローを統合する機能。現在のストリーミング表示フローを変更し、LLM応答完了後にTTS音声生成を行い、音声再生とテキスト表示を同時に開始する。長文テキストの分割処理、およびLLMによる感情パラメータ自動生成を含む。

## Glossary

- **Chat_Engine**: バックエンド（Rust/Tauri）のチャット処理エンジン。LLM呼び出し、DB保存、イベント発行を担当
- **TTS_Connector**: TTS音声合成コネクタ。テキストを受け取り音声バイトデータを返すインターフェース
- **Text_Splitter**: 長文テキストを音声合成に適したチャンクに分割するモジュール
- **Emotion_Generator**: LLMを使用してテキストの感情パラメータを自動生成するモジュール（VoicePeak用）
- **Caption_Generator**: LLMを使用してテキストの喋り方キャプションを自動生成するモジュール（Irodori-TTS用）
- **Frontend_Chat_Store**: フロントエンドのチャット状態管理ストア（Zustand）
- **Audio_Player**: Web Audio APIによる音声再生モジュール
- **VoicePeak_CLI**: VoicePeak音声合成CLIツール。テキストからWAVファイルを生成
- **TTS_Flow_Controller**: TTS有効時のチャットフロー全体を制御するオーケストレーター

## Requirements

### Requirement 1: TTS有効時のチャットフロー変更

**User Story:** As a ユーザー, I want TTS有効時にLLM応答完了後に音声生成してからテキストと音声を同時に再生開始したい, so that テキスト表示と音声が同期した自然な体験を得られる。

#### Acceptance Criteria

1. WHILE TTS is enabled for the current character, THE Chat_Engine SHALL accumulate the full LLM response without emitting streaming chunks to the frontend
2. WHILE TTS is enabled for the current character, THE Chat_Engine SHALL initiate TTS audio generation after the full LLM response is received
3. WHEN TTS audio generation completes successfully, THE Chat_Engine SHALL emit a single event containing both the full text content and the audio data
4. WHEN the frontend receives the combined text-and-audio event, THE Frontend_Chat_Store SHALL display the full message text and start audio playback simultaneously
5. WHILE TTS is disabled for the current character, THE Chat_Engine SHALL maintain the existing streaming behavior unchanged

### Requirement 2: TTS音声生成中のローディング表示

**User Story:** As a ユーザー, I want TTS音声生成中にローディング表示を見たい, so that 処理中であることを認識でき、アプリがフリーズしていないと分かる。

#### Acceptance Criteria

1. WHEN the LLM response is complete and TTS generation begins, THE Frontend_Chat_Store SHALL transition to a TTS-loading state
2. WHILE the TTS-loading state is active, THE Frontend_Chat_Store SHALL display a loading indicator distinct from the streaming indicator
3. WHILE the TTS-loading state is active, THE Frontend_Chat_Store SHALL NOT display any partial text content
4. WHEN TTS generation completes or fails, THE Frontend_Chat_Store SHALL exit the TTS-loading state

### Requirement 3: テキスト分割

**User Story:** As a 開発者, I want 長文テキストをVoicePeak CLIに渡す前に適切なサイズに分割したい, so that VoicePeakの文字数制限を超えず、生成時間を適切に管理できる。

#### Acceptance Criteria

1. WHEN the LLM response text exceeds the configured maximum chunk size, THE Text_Splitter SHALL split the text into multiple chunks
2. THE Text_Splitter SHALL split text at sentence boundaries (句点「。」、感嘆符「！」、疑問符「？」) to preserve natural speech flow
3. IF a single sentence exceeds the maximum chunk size, THEN THE Text_Splitter SHALL split at clause boundaries (読点「、」) as a fallback
4. THE Text_Splitter SHALL preserve the original text order across all chunks without omission or duplication
5. THE Text_Splitter SHALL produce chunks where each chunk length is less than or equal to the configured maximum chunk size
6. WHEN text is split into multiple chunks, THE TTS_Flow_Controller SHALL synthesize each chunk sequentially and concatenate the resulting audio data

### Requirement 4: VoicePeak感情パラメータ自動生成

**User Story:** As a ユーザー, I want チャット応答の内容に応じてVoicePeakの感情パラメータが自動調整されてほしい, so that キャラクターの音声が応答内容に合った感情表現を持つ。

#### Acceptance Criteria

1. WHILE TTS is enabled and the provider is VoicePeak, THE Emotion_Generator SHALL invoke the LLM to analyze the response text and generate emotion parameters (happy, fun, angry, sad), speed, and pitch values
2. THE Emotion_Generator SHALL use the character's default TTS config values as the baseline for parameter generation
3. WHEN the Emotion_Generator receives LLM-generated parameters, THE Emotion_Generator SHALL validate that emotion values are within 0-100, speed is within 50-200, and pitch is within -300 to 300
4. IF the Emotion_Generator LLM call fails, THEN THE TTS_Flow_Controller SHALL fall back to the character's default TTS config parameters
5. THE Emotion_Generator SHALL complete parameter generation within a single LLM call per message to minimize latency

### Requirement 5: TTS失敗時のフォールバック

**User Story:** As a ユーザー, I want TTS音声生成が失敗してもチャットメッセージを見たい, so that 音声合成の問題でチャット体験が完全に止まることがない。

#### Acceptance Criteria

1. IF TTS audio generation fails for any chunk, THEN THE TTS_Flow_Controller SHALL emit the full text content to the frontend without audio data
2. IF TTS audio generation fails, THEN THE Frontend_Chat_Store SHALL display the message text immediately and show a non-blocking error notification
3. IF TTS audio generation times out (exceeds configured timeout), THEN THE TTS_Flow_Controller SHALL cancel the TTS process and fall back to text-only display

### Requirement 6: 複数チャンクの音声結合

**User Story:** As a 開発者, I want 分割テキストから生成された複数のWAVデータを1つの連続音声として再生したい, so that ユーザーが途切れのない自然な音声を聞ける。

#### Acceptance Criteria

1. WHEN multiple audio chunks are generated from split text, THE TTS_Flow_Controller SHALL concatenate the WAV audio data into a single continuous audio stream
2. THE TTS_Flow_Controller SHALL preserve the original chunk order during concatenation
3. THE Audio_Player SHALL play the concatenated audio as a single continuous playback without gaps between chunks

### Requirement 7: Irodori-TTSキャプション動的生成

**User Story:** As a ユーザー, I want Irodori-TTS使用時にチャット内容に応じた喋り方のキャプションが自動生成されてほしい, so that キャラクターの声が応答内容に合った表現で再生される。

#### Acceptance Criteria

1. WHEN the TTS provider is Irodori-TTS and the mode is caption-based, THE Caption_Generator SHALL invoke the LLM to generate a speaking-style caption based on the response text content
2. THE Caption_Generator SHALL combine the character's base voice caption (configured in character settings, describing the voice characteristics) with the dynamically generated speaking-style caption into a single caption string for the TTS request
3. WHEN the TTS provider is Irodori-TTS and the mode is reference-audio-based, THE TTS_Flow_Controller SHALL use only the configured reference audio path without any caption
4. THE character settings SHALL allow the user to choose between caption mode and reference audio mode (mutually exclusive)
5. WHEN caption mode is selected in character settings, THE CharacterForm SHALL require a base voice caption field describing the voice characteristics (e.g., "落ち着いた女性の声")
6. WHEN reference audio mode is selected in character settings, THE CharacterForm SHALL require a reference audio file path and SHALL NOT display caption fields
7. IF the Caption_Generator LLM call fails, THEN THE TTS_Flow_Controller SHALL use only the character's base voice caption without the dynamic speaking-style portion
