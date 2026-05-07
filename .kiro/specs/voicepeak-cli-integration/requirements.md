# 要件ドキュメント

## 概要

VoicePeakとの連携方式を、HTTP APIブリッジサーバー経由からVoicePeak公式CLIを直接呼び出す方式に変更する。現在の実装はブリッジサーバーの仕様が未確定で動作しないため、Tauriバックエンドから`voicepeak`コマンドを直接実行し、生成されたWAVファイルを読み取る方式に置き換える。

## 用語集

- **VoicePeak_CLI**: VoicePeakが公式に提供するコマンドラインインターフェース。テキストから音声WAVファイルを生成する
- **TTS_Connector**: TTSプロバイダーへのディスパッチを担うトレイト。`synthesize`と`test_connection`メソッドを持つ
- **VoicePeak_Handler**: VoicePeak固有の音声合成ロジックを実装するモジュール
- **TTSConfig**: キャラクターごとのTTS設定を保持する構造体
- **Emotion_Params**: VoicePeakの感情パラメータ（happy, fun, angry, sad）を保持する構造体
- **CLI_Executor**: VoicePeak CLIプロセスを起動し、引数を組み立て、実行結果を取得するコンポーネント
- **CharacterForm**: キャラクターの作成・編集を行うReactコンポーネント。名前・概要・System Prompt・TTS設定を入力する

## 要件

### 要件1: VoicePeak CLIパス設定

**ユーザーストーリー:** ユーザーとして、VoicePeak CLIの実行ファイルパスを設定したい。これにより、VoicePeakがデフォルト以外の場所にインストールされていても利用できる。

#### 受け入れ基準

1. THE TTSConfig SHALL include an `executable_path` field to specify the VoicePeak CLI executable location
2. WHEN `executable_path` is not specified, THE VoicePeak_Handler SHALL use the default value `"voicepeak"` (relying on system PATH resolution)
3. THE TTSConfig SHALL no longer require the `base_url` field for the VoicePeak provider

### 要件2: CLIコマンド引数構築

**ユーザーストーリー:** 開発者として、TTSConfigの設定値からVoicePeak CLIの引数を正しく構築したい。これにより、ナレーター・感情・速度・ピッチの各パラメータが正確にCLIに渡される。

#### 受け入れ基準

1. THE VoicePeak_Handler SHALL construct CLI arguments with `--say` flag followed by the input text
2. THE VoicePeak_Handler SHALL construct CLI arguments with `--out` flag followed by a temporary output file path
3. WHEN `narrator` is specified in TTSConfig, THE VoicePeak_Handler SHALL include `--narrator` flag with the narrator name
4. WHEN `emotion` is specified in TTSConfig, THE VoicePeak_Handler SHALL include `--emotion` flag with comma-separated key=value pairs (e.g., `happy=50,sad=20`)
5. WHEN `speed` is specified in TTSConfig, THE VoicePeak_Handler SHALL include `--speed` flag with the speed value as an integer percentage (e.g., 120 for 1.2x)
6. WHEN `pitch` is specified in TTSConfig, THE VoicePeak_Handler SHALL include `--pitch` flag with the pitch value as an integer offset (e.g., -50 for -0.5)
7. WHEN optional parameters are not specified in TTSConfig, THE VoicePeak_Handler SHALL omit the corresponding CLI flags

### 要件3: CLI実行と音声データ取得

**ユーザーストーリー:** ユーザーとして、テキストを入力したらVoicePeak CLIが実行されて音声データが返されてほしい。これにより、ブリッジサーバーなしで音声合成が動作する。

#### 受け入れ基準

1. WHEN `synthesize` is called, THE VoicePeak_Handler SHALL create a temporary WAV file path for CLI output
2. WHEN `synthesize` is called, THE VoicePeak_Handler SHALL execute the VoicePeak CLI process with the constructed arguments
3. WHEN the CLI process exits with status code 0, THE VoicePeak_Handler SHALL read the generated WAV file and return its contents as `Vec<u8>`
4. WHEN the CLI process exits with a non-zero status code, THE VoicePeak_Handler SHALL return an `AppError::Tts` error containing the stderr output
5. WHEN the CLI process fails to start (executable not found), THE VoicePeak_Handler SHALL return an `AppError::Tts` error indicating the executable was not found
6. THE VoicePeak_Handler SHALL delete the temporary WAV file after reading its contents, regardless of success or failure

### 要件4: 接続テスト

**ユーザーストーリー:** ユーザーとして、VoicePeak CLIが正しく動作するか事前にテストしたい。これにより、設定ミスを早期に発見できる。

#### 受け入れ基準

1. WHEN `test_connection` is called, THE VoicePeak_Handler SHALL execute the VoicePeak CLI with a short test text
2. WHEN the test CLI process exits with status code 0, THE VoicePeak_Handler SHALL return `Ok(())`
3. WHEN the test CLI process exits with a non-zero status code or fails to start, THE VoicePeak_Handler SHALL return an `AppError::Tts` error with a descriptive message
4. THE VoicePeak_Handler SHALL delete any temporary files created during the connection test

### 要件5: フロントエンド型定義の更新

**ユーザーストーリー:** 開発者として、フロントエンドの型定義がCLI方式に対応していてほしい。これにより、設定UIが正しいフィールドを表示できる。

#### 受け入れ基準

1. THE TTSConfig type in TypeScript SHALL include an optional `executable_path` field of type `string`
2. THE TTSConfig type in TypeScript SHALL make the `base_url` field optional (required only for irodori-tts provider)
3. THE TTSConfig type in TypeScript SHALL retain `narrator`, `emotion`, `speed`, and `pitch` fields for VoicePeak configuration

### 要件6: HTTP依存の除去

**ユーザーストーリー:** 開発者として、VoicePeakハンドラーからHTTPクライアント依存を除去したい。これにより、不要なネットワーク依存がなくなりコードが簡潔になる。

#### 受け入れ基準

1. THE VoicePeak_Handler SHALL NOT use `reqwest::Client` or any HTTP client for VoicePeak operations
2. THE VoicePeak_Handler SHALL use `tokio::process::Command` (or equivalent async process execution) to invoke the CLI
3. THE DefaultTTSConnector SHALL NOT pass an HTTP client reference to VoicePeak_Handler

### 要件7: CLIコマンド引数のフォーマット正確性

**ユーザーストーリー:** 開発者として、CLIコマンド引数のフォーマットが仕様通りであることを保証したい。これにより、任意の入力に対して正しいコマンドが生成される。

#### 受け入れ基準

1. FOR ALL valid TTSConfig values, THE VoicePeak_Handler SHALL produce CLI arguments that can be parsed back to equivalent configuration values (round-trip property for argument construction)
2. FOR ALL input texts, THE VoicePeak_Handler SHALL include the text exactly as provided in the `--say` argument without modification
3. FOR ALL EmotionParams with at least one non-None field, THE VoicePeak_Handler SHALL format the `--emotion` argument as a comma-separated list of `key=value` pairs containing only the non-None fields

### 要件8: キャラクターTTS設定UI

**ユーザーストーリー:** ユーザーとして、キャラクター編集画面でTTSの有効/無効やプロバイダー別の設定を行いたい。これにより、キャラクターごとに音声合成の挙動をGUIから制御できる。

#### 受け入れ基準

1. THE CharacterForm SHALL display a toggle or checkbox to enable or disable TTS for the character
2. WHEN TTS is enabled, THE CharacterForm SHALL display radio buttons allowing the user to select a TTS provider from "TTSサーバー（Irodori-TTS）" and "VoicePeak（CLI方式）"
3. WHILE TTS is disabled, THE CharacterForm SHALL render the provider selection radio buttons in a disabled state
4. WHEN the user selects "TTSサーバー" as the provider, THE CharacterForm SHALL display only the following configuration fields: base_url, reference_audio_path, caption
5. WHEN the user selects "VoicePeak" as the provider, THE CharacterForm SHALL display only the following configuration fields: executable_path, narrator, emotion, speed, pitch
6. WHEN TTS is disabled, THE CharacterForm SHALL not include `tts_config` in the saved character data
7. WHEN TTS is enabled, THE CharacterForm SHALL include `tts_config` with the selected provider and corresponding field values in the saved character data
8. WHEN switching between providers, THE CharacterForm SHALL preserve previously entered field values for each provider until the form is submitted or cancelled
9. IF the character being edited already has a `tts_config`, THEN THE CharacterForm SHALL populate the TTS toggle, provider selection, and configuration fields with the existing values
