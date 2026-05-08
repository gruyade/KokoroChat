# 実装計画: App Enhancements V3

## 概要

AI Character Chatアプリのv3機能強化。プロバイダー永続化修正（基盤）→ システムメッセージバッジ復元 → TTS無効時UI整理 → 思考・記憶リアルタイム反映 → IrodoriTTSグローバル化 → プロバイダー別API対応の順で実装する。バックエンドはRust、フロントエンドはReact + TypeScript + Zustand。

## タスク

- [ ] 1. プロバイダー設定の永続化修正（Requirement 6）
  - [x] 1.1 Rust側ModelSettingsにproviderフィールドを追加
    - `src-tauri/src/models/config.rs` の `ModelSettings` 構造体に `provider: Option<LLMProvider>` を追加
    - `#[serde(default)]` アトリビュートを付与し後方互換性を維持
    - `LLMProvider` enumを `models/config.rs` に定義（`Openai`, `Anthropic`, `Google`, `OpenaiCompatible`）
    - `#[serde(rename_all = "snake_case")]` で TypeScript側の型と一致させる
    - `src-tauri/src/models/mod.rs` の re-export に `LLMProvider` を追加
    - _Requirements: 6.1, 6.5_

  - [x] 1.2 LLMClientConfigにproviderフィールドを追加
    - `src-tauri/src/llm/client.rs` の `LLMClientConfig` に `provider: Option<LLMProvider>` を追加
    - `#[serde(default)]` を付与
    - `src-tauri/src/commands/config.rs` の `test_llm_connection` で `settings.provider` を `LLMClientConfig` に渡すよう修正
    - 既存テストのLLMClientConfig生成箇所に `provider: None` を追加
    - _Requirements: 5.6, 6.1_

  - [x] 1.3 ModelSettingsラウンドトリップのプロパティテスト
    - **Property 4: ModelSettingsシリアライズ/デシリアライズ ラウンドトリップ**
    - テストファイル: `src-tauri/src/config/property_tests.rs` に追加
    - proptestでランダムなModelSettings値（provider=Some各種/None）を生成 → serialize → deserialize → 等価性確認
    - providerフィールド欠落JSONからのデシリアライズで `provider=None` になることを確認
    - **Validates: Requirements 6.2, 6.3, 6.5, 6.6**

- [x] 2. チェックポイント - プロバイダー永続化修正の確認
  - すべてのテストが通ることを確認し、不明点があればユーザーに質問する。

- [ ] 3. システムメッセージの中央寄せバッジ表示復元（Requirement 1）
  - [x] 3.1 MessageBubbleのシステムメッセージ表示を中央寄せバッジに変更
    - `src/components/chat/MessageBubble.tsx` の `isSystemMessage` 分岐を改修
    - 右寄せバブルスタイルから中央寄せバッジスタイルに変更
    - `justify-center` レイアウト、`rounded-full` バッジ形状、`bg-muted text-muted-foreground text-xs` スタイル
    - Infoアイコン + displayContentをインラインで表示
    - ホバー時にバッジ上部に編集・削除ボタンを表示（`absolute -top-7 left-1/2 -translate-x-1/2`）
    - 編集ボタン押下 → `handleEdit` → `EditableMessage` コンポーネント表示
    - 削除ボタン押下 → `handleDelete` → メッセージ削除
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

- [ ] 4. TTS無効時のボリュームコントロール非表示（Requirement 2）
  - [x] 4.1 ChatHeaderControlsにTTS有効判定を追加
    - `src/components/chat/ChatHeaderControls.tsx` を改修
    - `useConfigStore` から `config?.tts?.enabled` を取得
    - ボリュームコントロール部分（ミュートボタン + スライダー）を `{ttsEnabled && (...)}` で条件付きレンダリング
    - _Requirements: 2.1, 2.2, 2.3_

- [ ] 5. 思考・記憶生成の即時反映（Requirement 3）
  - [x] 5.1 バックエンドでmemory:generatedイベントを発行
    - `src-tauri/src/commands/memory.rs` の `generate_memory_manual` コマンド内で記憶生成後に `app_handle.emit("memory:generated", payload)` を追加
    - `MemoryGeneratedEvent` 構造体（`character_id`, `memory_id`）を定義
    - `src-tauri/src/chat/engine.rs` の自動圧縮処理後にも同様のイベント発行を追加
    - イベント発行失敗時はログ出力のみで処理を継続
    - _Requirements: 3.2_

  - [x] 5.2 ThoughtViewにTauriイベントリスナーを追加
    - `src/components/thought/ThoughtView.tsx` に `listen` をインポート（`@tauri-apps/api/event`）
    - `useEffect` で `thought:generated` イベントをリッスン
    - イベントの `character_id` が現在選択中のキャラクターと一致する場合に `loadThoughts()` を呼び出し
    - アンマウント時に `unlisten` でクリーンアップ
    - _Requirements: 3.1, 3.3, 3.5_

  - [x] 5.3 MemoryViewにTauriイベントリスナーを追加
    - `src/components/memory/MemoryView.tsx` に `listen` をインポート（`@tauri-apps/api/event`）
    - `useEffect` で `memory:generated` イベントをリッスン
    - イベントの `character_id` が現在選択中のキャラクターと一致する場合に `loadMemories()` を呼び出し
    - アンマウント時に `unlisten` でクリーンアップ
    - _Requirements: 3.2, 3.4, 3.5_

- [x] 6. チェックポイント - 即時反映機能の確認
  - すべてのテストが通ることを確認し、不明点があればユーザーに質問する。

- [ ] 7. IrodoriTTSベースURL設定のグローバル化（Requirement 4）
  - [x] 7.1 Rust側TTSGlobalConfigにirodori_base_urlフィールドを追加
    - `src-tauri/src/models/config.rs` の `TTSGlobalConfig` に `irodori_base_url: Option<String>` を追加
    - `#[serde(default)]` を付与
    - _Requirements: 4.1, 4.6_

  - [x] 7.2 Rust側TTSConfigにモード別ベースURLフィールドを追加
    - `src-tauri/src/models/tts.rs` の `TTSConfig` に以下を追加:
      - `base_url: Option<String>` — キャラクター個別の共通ベースURL
      - `caption_base_url: Option<String>` — キャプションモード用
      - `reference_audio_base_url: Option<String>` — 参照音声モード用
    - すべて `#[serde(default)]` を付与
    - _Requirements: 4.5_

  - [x] 7.3 ベースURL解決関数の実装
    - `src-tauri/src/tts/irodori.rs`（または適切な場所）に `resolve_irodori_base_url` 関数を実装
    - 優先順位: モード別URL > キャラクター個別共通URL > グローバル設定URL > None
    - _Requirements: 4.3, 4.4_

  - [x] 7.4 IrodoriTTSベースURL解決のプロパティテスト
    - **Property 1: IrodoriTTSベースURL解決の優先順位**
    - テストファイル: `src-tauri/src/tts/property_tests.rs` に追加
    - proptestでTTSConfig + TTSGlobalConfigのランダムな組み合わせを生成
    - 優先順位ルールが常に守られることを検証
    - **Validates: Requirements 4.3, 4.4**

  - [x] 7.5 TypeScript側TTSGlobalConfigにirodori_base_urlを追加
    - `src/types/config.ts` の `TTSGlobalConfig` に `irodori_base_url?: string` を追加
    - `src/components/settings/SettingsView.tsx` のTTSタブにIrodoriTTSベースURL入力欄を追加
    - _Requirements: 4.1, 4.2_

- [ ] 8. プロバイダー別API仕様対応（Requirement 5）
  - [x] 8.1 API形式判定ロジックの実装
    - `src-tauri/src/llm/client.rs` に `ApiStrategy` enum（`OpenAI`, `Gemini`, `Anthropic`）を追加
    - `resolve_api_strategy(config: &LLMClientConfig) -> ApiStrategy` 関数を実装
    - `is_default_endpoint(base_url: &str, provider: LLMProvider) -> bool` ヘルパーを実装
    - 判定ルール: Google+デフォルト→Gemini、Anthropic+デフォルト→Anthropic、それ以外→OpenAI
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

  - [x] 8.2 API形式決定のプロパティテスト
    - **Property 2: プロバイダー×エンドポイントによるAPI形式決定**
    - テストファイル: `src-tauri/src/llm/property_tests.rs` を新規作成
    - proptestでLLMClientConfig値をランダム生成し、resolve_api_strategyの結果が判定ルールに従うことを検証
    - **Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5**

  - [x] 8.3 Google Gemini APIリクエスト構築の実装
    - `build_gemini_request(messages, config) -> Value` 関数を実装
    - Gemini API形式: `contents[]` にrole/partsマッピング、`systemInstruction` にシステムメッセージ
    - `generationConfig.temperature` を設定
    - Gemini用エンドポイントURL構築: `{base_url}/models/{model}:generateContent`
    - _Requirements: 5.1_

  - [x] 8.4 Anthropic Messages APIリクエスト構築の実装
    - `build_anthropic_request(messages, config) -> Value` 関数を実装
    - Anthropic形式: `system` フィールドにシステムメッセージ、`messages[]` にuser/assistant
    - `model`, `temperature`, `max_tokens` を設定
    - Anthropic用ヘッダー: `x-api-key`, `anthropic-version`
    - _Requirements: 5.2_

  - [x] 8.5 プロバイダー別レスポンスパースの実装
    - `parse_gemini_response(body: &Value) -> Result<LLMResponse, AppError>` を実装
    - `parse_anthropic_response(body: &Value) -> Result<LLMResponse, AppError>` を実装
    - Gemini: `candidates[0].content.parts[0].text` を抽出
    - Anthropic: `content[].text` を結合して抽出
    - _Requirements: 5.7_

  - [x] 8.6 レスポンスパースのプロパティテスト
    - **Property 3: プロバイダー別レスポンスパースの正当性**
    - テストファイル: `src-tauri/src/llm/property_tests.rs` に追加
    - proptestで有効なGemini/Anthropicレスポンス構造をランダム生成し、パース結果が正しいことを検証
    - **Validates: Requirements 5.7**

  - [x] 8.7 LLMClient実装のStrategy Pattern統合
    - `OpenAICompatibleClient` を `MultiProviderClient` にリネームまたは拡張
    - `chat` メソッド内で `resolve_api_strategy` を呼び出し、戦略に応じてリクエスト構築・送信・パースを分岐
    - `chat_stream` メソッドも同様に対応（Gemini/Anthropicのストリーミング形式対応）
    - `test_connection` メソッドもプロバイダー別に対応
    - 既存のOpenAI互換処理はそのまま維持
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7_

- [x] 9. チェックポイント - 全機能の統合確認
  - すべてのテストが通ることを確認し、不明点があればユーザーに質問する。

## 備考

- `*` マーク付きタスクはオプション（スキップ可能）
- 各タスクは特定の要件を参照しトレーサビリティを確保
- チェックポイントで段階的に動作検証
- プロパティテストは設計文書のCorrectness Propertiesに基づく
- Requirement 6（プロバイダー永続化）を最初に実装する理由: Requirement 4, 5がModelSettingsのproviderフィールドに依存するため
- 既存のTypeScript側 `LLMProvider` 型と `ModelSettings.provider` フィールドは既に定義済み（app-enhancements-v2で追加済み）
