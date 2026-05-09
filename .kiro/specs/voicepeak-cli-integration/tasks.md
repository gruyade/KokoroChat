# Implementation Plan: VoicePeak CLI Integration

## 概要

VoicePeakとの連携方式をHTTP APIブリッジサーバー経由からCLI直接呼び出しに変更する。データモデル変更→バックエンドロジック→テスト→フロントエンドUIの順で実装する。

## Tasks

- [x] 1. データモデル更新（Rust + TypeScript）
  - [x] 1.1 `src-tauri/src/models/tts.rs` の `TTSConfig` 構造体を更新
    - `base_url` を `String` → `Option<String>` に変更
    - `executable_path: Option<String>` フィールドを追加
    - `#[serde(default)]` アトリビュートを適切に付与
    - _Requirements: 1.1, 1.3, 5.2_

  - [x] 1.2 `src/types/tts.ts` の `TTSConfig` 型を更新
    - `base_url` を必須 → optional (`base_url?: string`) に変更
    - `executable_path?: string` フィールドを追加
    - _Requirements: 5.1, 5.2, 5.3_

- [x] 2. VoicePeakHandler CLI方式への書き換え
  - [x] 2.1 `src-tauri/src/tts/voicepeak.rs` をCLI方式に全面書き換え
    - HTTP関連の構造体・インポートを全て除去
    - `VoicePeakHandler` を引数なしの `new()` に変更（HTTPクライアント不要）
    - `build_cli_args(text, output_path, config)` 純粋関数を実装
    - `format_emotion(emotion)` ヘルパー関数を実装
    - `synthesize` メソッドを `tokio::process::Command` + 一時ファイル方式に書き換え
    - `test_connection` メソッドを短いテストテキストでのCLI実行に書き換え
    - `tempfile` クレートで一時WAVファイル管理（`TempPath`による自動クリーンアップ）
    - _Requirements: 1.2, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 4.1, 4.2, 4.3, 4.4, 6.1, 6.2_

  - [x] 2.2 `src-tauri/src/tts/connector.rs` の `DefaultTTSConnector` を更新
    - `synthesize_voicepeak` から HTTPクライアント渡しを除去
    - `test_voicepeak` から HTTPクライアント渡しを除去
    - `VoicePeakHandler::new()` を引数なしで呼び出すように変更
    - _Requirements: 6.3_

  - [x] 2.3 `src-tauri/Cargo.toml` に `tempfile` クレートを依存追加
    - _Requirements: 3.1_

- [x] 3. Checkpoint - バックエンドコンパイル確認
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. プロパティベーステスト
  - [x] 4.1 `src-tauri/src/tts/property_tests.rs` — Property 1: CLI引数構築のラウンドトリップ
    - **Property 1: CLI引数構築のラウンドトリップ**
    - 任意の有効なTTSConfig値に対して `build_cli_args` で構築した引数列をパースし直すと元の設定値と等価
    - `proptest` クレートで `arb_tts_config_voicepeak` ジェネレータを実装
    - **Validates: Requirements 7.1, 2.3, 2.5, 2.6, 2.7**

  - [x] 4.2 `src-tauri/src/tts/property_tests.rs` — Property 2: 入力テキストの保全
    - **Property 2: 入力テキストの保全**
    - 任意の入力テキストに対して `build_cli_args` の `--say` フラグ値が入力と完全一致
    - **Validates: Requirements 7.2, 2.1**

  - [x] 4.3 `src-tauri/src/tts/property_tests.rs` — Property 3: 感情パラメータのフォーマット正確性
    - **Property 3: 感情パラメータのフォーマット正確性**
    - 少なくとも1つの非Noneフィールドを持つEmotionParamsに対して `format_emotion` が非Noneフィールドのみを含む
    - **Validates: Requirements 7.3, 2.4**

- [x] 5. ユニットテスト
  - [x] 5.1 `src-tauri/src/tts/tests.rs` のユニットテストを更新
    - デフォルト値テスト: `executable_path` 未指定時に `"voicepeak"` が使用されること
    - 空EmotionParamsテスト: 全フィールドNone時に `--emotion` フラグが省略されること
    - オプショナルパラメータ省略テスト: 未指定パラメータのフラグが含まれないこと
    - TTSConfigのシリアライズ/デシリアライズ互換性テスト
    - _Requirements: 1.2, 2.7, 7.1_

- [x] 6. Checkpoint - 全テスト通過確認
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. フロントエンドTTS設定UI実装
  - [x] 7.1 `src/components/character/CharacterForm.tsx` にTTS設定セクションを追加
    - TTS有効/無効トグル（チェックボックス）を追加
    - プロバイダー選択ラジオボタン（「TTSサーバー（Irodori-TTS）」「VoicePeak（CLI方式）」）を追加
    - TTS無効時はラジオボタンを `disabled` にし、設定フィールドを非表示
    - `TTSFormState` ローカルステートを導入（両プロバイダーの値を独立保持）
    - _Requirements: 8.1, 8.2, 8.3, 8.8_

  - [x] 7.2 VoicePeak設定フィールドの実装
    - `executable_path` テキスト入力（プレースホルダー: "voicepeak"）
    - `narrator` テキスト入力
    - `speed` 数値入力（整数パーセント）
    - `pitch` 数値入力（整数オフセット）
    - 感情パラメータスライダー（happy, fun, angry, sad: 0〜100）
    - VoicePeakプロバイダー選択時のみ表示
    - _Requirements: 8.5_

  - [x] 7.3 Irodori-TTS設定フィールドの実装
    - `base_url` テキスト入力
    - `reference_audio_path` テキスト入力
    - `caption` テキスト入力
    - Irodori-TTSプロバイダー選択時のみ表示
    - _Requirements: 8.4_

  - [x] 7.4 フォーム送信時のデータ変換ロジック実装
    - `onSave` シグネチャに `tts_config?: TTSConfig` を追加
    - TTS無効時: `tts_config` を含めない
    - TTS有効時: 選択中プロバイダーの値のみを `TTSConfig` に変換して含める
    - 既存キャラクター編集時: `tts_config` から初期値をロード
    - _Requirements: 8.6, 8.7, 8.9_

  - [x] 7.5 アクセシビリティ対応
    - TTS設定セクションを `<fieldset>` + `<legend>` でグループ化
    - トグルに `role="switch"` + `aria-checked` を付与
    - ラジオボタンに `role="radiogroup"` を付与
    - 非活性フィールドに `aria-disabled` + `disabled` 属性を付与
    - _Requirements: 8.3_

- [x] 8. Final checkpoint - 全体動作確認
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- `*` 付きタスクはオプション（スキップ可能）
- 各タスクは対応する要件番号を明記し、トレーサビリティを確保
- プロパティテストは設計ドキュメントの正当性プロパティに対応
- チェックポイントでインクリメンタルな検証を実施
- VoicePeak CLIが利用可能な環境でのみ統合テストを実行（CI環境では省略）
