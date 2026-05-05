# Implementation Plan: AI Character Chat

## Overview

Tauri v2 + Rust バックエンド + React フロントエンドによるAIキャラクターチャットデスクトップアプリケーションの実装計画。プロジェクト基盤構築 → データ層 → コア機能 → UI → 統合の順で段階的に実装。

## Tasks

- [x] 1. プロジェクト基盤構築
  - [x] 1.1 Tauri v2プロジェクト初期化とディレクトリ構成作成
    - `cargo create-tauri-app` でプロジェクト生成
    - `src-tauri/src/` 配下にモジュールディレクトリ作成（llm/, tts/, character/, chat/, spontaneous/, thought/, memory/, attachment/, plugin/, config/, db/, models/, commands/）
    - `src/` 配下にフロントエンドディレクトリ作成（components/, stores/, hooks/, types/, styles/）
    - `tests/` 配下にテストディレクトリ作成（unit/, property/, component/, e2e/）
    - _Requirements: 8.1, 9.1_

  - [x] 1.2 Rust依存関係とCargo.toml設定
    - tokio, reqwest, rusqlite, serde, serde_json, uuid, chrono, thiserror, async-trait, tauri, tauri-plugin-fs, tauri-plugin-dialog, pdf-extract 追加
    - proptest（dev-dependencies）追加
    - _Requirements: 7.3, 8.6_

  - [x] 1.3 フロントエンド依存関係とpackage.json設定
    - React 19, TypeScript, Vite, Zustand, Tailwind CSS, shadcn/ui, @tauri-apps/api 追加
    - Vitest, @testing-library/react, fast-check（devDependencies）追加
    - ESLint, Prettier設定
    - _Requirements: 9.1, 9.2_

  - [x] 1.4 Tauri設定ファイル（tauri.conf.json）作成
    - ウィンドウ設定（minWidth: 800, minHeight: 600）
    - CSPセキュリティ設定
    - プラグイン許可設定（fs, dialog）
    - _Requirements: 9.1, 8.6_

  - [x] 1.5 セキュリティ・GitHub公開対応ファイル作成
    - `.gitignore`（.env, *.sqlite, target/, node_modules/）
    - `.env.example`（AI_CHAT_LLM_BASE_URL等のテンプレート）
    - `.pre-commit-config.yaml`（detect-secrets設定）
    - _Requirements: 8.1, 8.2, 8.5, 8.7_

  - [x] 1.6 AppError型とエラーハンドリング基盤実装
    - `src-tauri/src/error.rs` にAppError enum定義
    - thiserror, Serialize derive実装
    - Tauri InvokeError変換実装
    - _Requirements: 2.6, 6.6, 10.5, 10.6, 11.7_

- [x] 2. データ層実装
  - [x] 2.1 SQLiteデータベース初期化とマイグレーション
    - `src-tauri/src/db/database.rs` にDB接続・初期化ロジック
    - `src-tauri/src/db/migrations.rs` にスキーマ作成SQL（characters, chat_sessions, chat_messages, memories, thoughts, plugins, attachments テーブル）
    - インデックス作成（idx_chat_sessions_character, idx_chat_messages_session, idx_memories_character, idx_thoughts_character, idx_attachments_message）
    - _Requirements: 1.2, 2.3, 5.2_

  - [x] 2.2 データモデル定義（Rust structs）
    - `src-tauri/src/models/` 配下に Character, ChatSession, ChatMessageRecord, Memory, Thought, AppConfig 等の構造体定義
    - Serialize/Deserialize derive実装
    - _Requirements: 1.2, 2.2, 4.2, 5.1_

  - [x] 2.3 リポジトリ層実装（CRUD操作）
    - `src-tauri/src/db/repositories/character.rs` — Character CRUD
    - `src-tauri/src/db/repositories/chat.rs` — ChatSession, ChatMessage CRUD
    - `src-tauri/src/db/repositories/memory.rs` — Memory CRUD
    - `src-tauri/src/db/repositories/thought.rs` — Thought CRUD
    - CASCADE DELETE対応（Character削除時に関連データ全削除）
    - _Requirements: 1.6, 2.3, 2.4, 5.2, 5.4_

  - [x] 2.4 データモデルのプロパティテスト
    - **Property 1: Character serialization round-trip**
    - **Property 3: Cascade delete removes all related data**
    - **Property 5: Session listing completeness**
    - **Property 6: Session metadata invariant**
    - **Validates: Requirements 1.2, 1.4, 1.6, 2.3, 2.4**

- [x] 3. LLMクライアント実装
  - [x] 3.1 LLM Client trait定義と実装
    - `src-tauri/src/llm/client.rs` にLLMClient trait実装
    - OpenAI互換APIフォーマットでのリクエスト構築（model, messages, temperature, tools）
    - reqwest + tokioによる非同期HTTP通信
    - ストリーミングレスポンス（SSE）パース処理
    - tool_callレスポンスのパース（LLMResponse::ToolCalls）
    - 接続テスト機能
    - _Requirements: 7.3, 2.2, 2.5, 11.3_

  - [x] 3.2 LLMクライアントのプロパティテスト
    - **Property 17: LLM request OpenAI format compliance**
    - **Validates: Requirements 7.3**

- [x] 4. Checkpoint - 基盤確認
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. キャラクター機能実装
  - [x] 5.1 Character Creator実装
    - `src-tauri/src/character/creator.rs` にCharacterCreator trait実装
    - LLMを使用したSystem Prompt自動生成
    - Character保存・更新・削除・一覧取得
    - 削除時のCASCADE処理（Chat履歴、Memory、Thought全削除）
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

  - [x] 5.2 キャラクター機能のプロパティテスト
    - **Property 2: Character listing completeness**
    - **Validates: Requirements 1.4**

  - [x] 5.3 Tauri Commands（Character）実装
    - `src-tauri/src/commands/character.rs` に create_character, list_characters, get_character, update_character, delete_character コマンド実装
    - _Requirements: 1.1, 1.3, 1.4, 1.5, 1.6_

- [x] 6. チャット機能実装
  - [x] 6.1 Chat Engine実装
    - `src-tauri/src/chat/engine.rs` にChatEngine trait実装
    - セッション作成・メッセージ送信・履歴取得・セッション一覧・削除
    - コンテキスト組み立て（System Prompt + Memory + Chat History）
    - ストリーミングレスポンスのEvent emit
    - tool_callレスポンス処理（Plugin Systemへのディスパッチ → 結果をLLMに返却 → 最終応答生成）
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 5.3, 11.3_

  - [x] 6.2 チャットコンテキスト組み立てのプロパティテスト
    - **Property 4: Chat context assembly includes system prompt, history, and memories**
    - **Validates: Requirements 2.2, 5.3**

  - [x] 6.3 Tauri Commands（Chat）実装
    - `src-tauri/src/commands/chat.rs` に create_session, send_message, get_history, list_sessions, delete_session コマンド実装
    - Tauri Events定義（chat:stream, tool:executing, tool:result）
    - _Requirements: 2.1, 2.3, 2.5, 11.9_

- [ ] 7. 自発的発話実装
  - [x] 7.1 Spontaneous Speaker実装
    - `src-tauri/src/spontaneous/speaker.rs` にSpontaneousSpeaker trait実装
    - tokio timerによる定期評価
    - 最小間隔制御
    - System Prompt + 直近会話コンテキストに基づくメッセージ生成
    - role='spontaneous'でのメッセージ保存
    - Event emit（spontaneous:message）
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

  - [x] 7.2 自発的発話のプロパティテスト
    - **Property 7: Spontaneous speech interval enforcement**
    - **Property 8: Spontaneous speech context assembly**
    - **Property 9: Spontaneous messages have distinct role**
    - **Validates: Requirements 3.1, 3.2, 3.5**

- [x] 8. 独自思考実装
  - [x] 8.1 Thought Engine実装
    - `src-tauri/src/thought/engine.rs` にThoughtEngine trait実装
    - tokio timerによる定期思考生成
    - 直近会話コンテキスト + Memory参照による思考生成
    - Thought専用ストレージへの保存（chat_messagesとは分離）
    - 頻度設定・開始/停止制御
    - Event emit（thought:generated）
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6_

  - [x] 8.2 独自思考のプロパティテスト
    - **Property 10: Thought storage separation**
    - **Property 11: Thought generation context includes chat and memories**
    - **Validates: Requirements 4.2, 4.4**

- [x] 9. 記憶管理実装
  - [x] 9.1 Memory Manager実装
    - `src-tauri/src/memory/manager.rs` にMemoryManager trait実装
    - 閾値到達時のLLMによる会話要約・圧縮
    - 関連Memory取得（character_id + コンテキストベース）
    - Memory一覧・更新・削除
    - 更新日時・ソースChat情報の記録
    - Model_Configからの圧縮用モデル取得
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6_

  - [x] 9.2 記憶管理のプロパティテスト
    - **Property 12: Memory compression threshold trigger**
    - **Property 13: Memory metadata correctness**
    - **Validates: Requirements 5.1, 5.5**

  - [x] 9.3 Tauri Commands（Memory）実装
    - `src-tauri/src/commands/memory.rs` に list_memories, update_memory, delete_memory コマンド実装
    - _Requirements: 5.4_

- [x] 10. Checkpoint - コア機能確認
  - Ensure all tests pass, ask the user if questions arise.

- [x] 11. TTS連携実装
  - [x] 11.1 TTS Connector実装
    - `src-tauri/src/tts/connector.rs` にTTSConnector trait実装
    - `src-tauri/src/tts/irodori.rs` にIrodori-TTS API対応実装（reference_audio_path, caption パラメータ）
    - `src-tauri/src/tts/voicepeak.rs` にVoicePeak API対応実装（narrator, emotion, speed, pitch パラメータ）
    - 接続テスト機能
    - エラー時のフォールバック（テキストのみ継続）
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7_

  - [x] 11.2 TTS連携のプロパティテスト
    - **Property 14: TTS request format correctness per provider**
    - **Validates: Requirements 6.2, 6.4**

  - [x] 11.3 Tauri Commands（TTS）実装
    - `src-tauri/src/commands/tts.rs` に synthesize_speech, test_tts_connection コマンド実装
    - Tauri Events定義（tts:audio）
    - _Requirements: 6.1, 6.7_

- [x] 12. モデル設定・API連携実装
  - [x] 12.1 Model Config実装
    - `src-tauri/src/config/model_config.rs` にModelConfig管理ロジック実装
    - 用途別設定管理（chat, memory, thought, character_generation）
    - 設定ファイル永続化（~/.ai-character-chat/config.json）
    - APIキーマスク表示用ヘルパー
    - 環境変数 / .env からの読み込みフォールバック
    - _Requirements: 7.1, 7.2, 7.4, 7.5, 8.2, 8.6_

  - [x] 12.2 モデル設定のプロパティテスト
    - **Property 15: Model config per-purpose isolation**
    - **Property 16: Model config round-trip**
    - **Validates: Requirements 7.1, 7.2**

  - [x] 12.3 Tauri Commands（Config）実装
    - `src-tauri/src/commands/config.rs` に get_config, set_config, test_llm_connection コマンド実装
    - _Requirements: 7.2, 7.6, 7.7_

- [x] 13. ファイル添付実装
  - [x] 13.1 Attachment Processor実装
    - `src-tauri/src/attachment/processor.rs` にAttachmentProcessor trait実装
    - テキストファイル読み込み（.txt, .md, .csv）
    - PDF テキスト抽出（pdf-extract crate使用）
    - 画像Base64エンコード（.png, .jpg, .webp）
    - ファイルサイズ検証（10MB上限）
    - 拡張子検証（非対応形式エラー）
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.8_

  - [x] 13.2 ファイル添付のプロパティテスト
    - **Property 18: Attachment file size validation**
    - **Property 19: Attachment type detection correctness**
    - **Property 20: Attachment round-trip persistence**
    - **Validates: Requirements 10.2, 10.5, 10.6, 10.8**

  - [x] 13.3 Tauri Commands（Attachment）実装
    - `src-tauri/src/commands/attachment.rs` に process_attachment, get_supported_extensions コマンド実装
    - tauri-plugin-dialog によるファイル選択ダイアログ連携
    - _Requirements: 10.1, 10.7_

- [x] 14. プラグイン/Tool Use実装
  - [x] 14.1 Plugin System・Registry実装
    - `src-tauri/src/plugin/system.rs` にPluginSystem trait実装（tool_callディスパッチ、有効ツール一覧取得）
    - `src-tauri/src/plugin/registry.rs` にPluginRegistry trait実装（登録、一覧、有効化/無効化、設定管理）
    - PluginHandler trait定義（第三者拡張用インターフェース）
    - サンドボックス機構（ファイルアクセス範囲制限）
    - _Requirements: 11.1, 11.2, 11.3, 11.4, 11.7, 11.8, 11.10_

  - [x] 14.2 組み込みプラグイン実装
    - `src-tauri/src/plugin/builtin/file_ops.rs` — ファイル読み書きプラグイン
    - `src-tauri/src/plugin/builtin/web_search.rs` — Web検索プラグイン
    - `src-tauri/src/plugin/builtin/calculator.rs` — 計算プラグイン
    - 各プラグインのToolDefinition定義（OpenAI Function Calling互換JSON Schema）
    - _Requirements: 11.5_

  - [x] 14.3 プラグインのプロパティテスト
    - **Property 21: Plugin registration idempotence**
    - **Property 22: Plugin enable/disable isolation**
    - **Property 23: Tool definition format compliance**
    - **Property 24: Tool call dispatch correctness**
    - **Property 25: Tool result propagation to LLM**
    - **Property 26: Plugin config persistence round-trip**
    - **Validates: Requirements 11.1, 11.2, 11.3, 11.7, 11.8, 11.9**

  - [x] 14.4 Tauri Commands（Plugin）実装
    - `src-tauri/src/commands/plugin.rs` に list_plugins, enable_plugin, disable_plugin, get_plugin_config, set_plugin_config コマンド実装
    - _Requirements: 11.2, 11.6, 11.8_

- [x] 15. Checkpoint - バックエンド全機能確認
  - Ensure all tests pass, ask the user if questions arise.

- [x] 16. AppState・Tauriエントリーポイント統合
  - [x] 16.1 AppState定義とmain.rs統合
    - `src-tauri/src/state.rs` にAppState構造体定義（DB接続、各モジュールインスタンス保持）
    - `src-tauri/src/main.rs` にTauriアプリ初期化、State管理、Command登録、Plugin登録
    - `src-tauri/src/lib.rs` にモジュール宣言
    - _Requirements: 全体統合_

- [x] 17. フロントエンド型定義
  - [x] 17.1 TypeScript型定義作成
    - `src/types/character.ts` — Character, CharacterUpdate
    - `src/types/chat.ts` — ChatSession, ChatMessageRecord, MessageAttachment, ToolCall
    - `src/types/memory.ts` — Memory
    - `src/types/thought.ts` — Thought
    - `src/types/config.ts` — AppConfig, ModelSettings, ModelPurpose
    - `src/types/tts.ts` — TTSConfig, TTSProvider, EmotionParams
    - `src/types/attachment.ts` — Attachment, AttachmentType
    - `src/types/plugin.ts` — PluginInfo, ToolDefinition, ToolCall, ToolResult
    - _Requirements: 全体_

- [x] 18. フロントエンド状態管理
  - [x] 18.1 Zustand Store実装
    - `src/stores/character.store.ts` — キャラクター一覧・選択状態管理、Tauri invoke連携
    - `src/stores/chat.store.ts` — セッション一覧・メッセージ・ストリーミング状態管理、Tauri Event listen連携
    - `src/stores/config.store.ts` — アプリ設定管理
    - `src/stores/plugin.store.ts` — プラグイン一覧・有効/無効状態管理
    - `src/stores/ui.store.ts` — テーマ切り替え、サイドバー状態
    - _Requirements: 9.2, 9.4_

- [ ] 19. フロントエンドUI実装
  - [x] 19.1 レイアウト・ナビゲーション実装
    - `src/App.tsx` — メインレイアウト（サイドバー + メインコンテンツ）
    - `src/components/sidebar/Sidebar.tsx` — Chat一覧、Character一覧、設定ナビゲーション
    - `src/components/sidebar/ChatList.tsx` — セッション一覧（最終メッセージ日時・プレビュー表示）
    - `src/components/sidebar/CharacterList.tsx` — キャラクター一覧
    - Tailwind CSS + shadcn/ui によるスタイリング
    - ダークモード/ライトモード切り替え
    - _Requirements: 9.1, 9.2, 9.4_

  - [x] 19.2 チャット画面実装
    - `src/components/chat/ChatView.tsx` — チャットメイン画面
    - `src/components/chat/MessageBubble.tsx` — メッセージ表示（user/assistant/spontaneous/tool ロール別スタイル）
    - `src/components/chat/MessageInput.tsx` — メッセージ入力欄・送信ボタン・ファイル添付ボタン
    - `src/components/chat/StreamingIndicator.tsx` — ローディングインジケーター
    - `src/components/chat/AttachmentPreview.tsx` — 添付ファイルプレビュー表示
    - `src/components/chat/ToolCallIndicator.tsx` — ツール実行中インジケーター
    - 自動スクロール（新メッセージ追加時）
    - ストリーミング表示（Tauri Event listen）
    - エラー表示・再送信ボタン
    - _Requirements: 2.5, 2.6, 3.5, 9.3, 9.5, 9.7, 10.7, 11.9_

  - [~] 19.3 キャラクター管理画面実装
    - `src/components/character/CharacterForm.tsx` — キャラクター作成・編集フォーム（名前、概要、System Prompt編集）
    - `src/components/character/CharacterCard.tsx` — キャラクターカード表示（アバター、名前、概要）
    - アバター画像設定対応
    - _Requirements: 1.1, 1.3, 1.4, 1.5, 9.6_

  - [~] 19.4 設定画面実装
    - `src/components/settings/SettingsView.tsx` — 設定メイン画面
    - `src/components/settings/ModelConfigForm.tsx` — 用途別モデル設定（baseUrl, model, apiKey, temperature）、APIキーマスク表示、接続テストボタン
    - `src/components/settings/TTSConfigForm.tsx` — TTS設定（プロバイダー選択、話者、参照音声パス）、有効/無効トグル
    - `src/components/settings/PluginConfigForm.tsx` — プラグイン固有設定
    - 自発的発話設定（有効/無効トグル、最小間隔）
    - 思考生成設定（有効/無効、頻度）
    - _Requirements: 3.3, 3.4, 4.5, 6.3, 6.5, 7.1, 7.2, 7.4, 7.6, 7.7_

  - [~] 19.5 プラグイン管理画面実装
    - `src/components/plugin/PluginListView.tsx` — プラグイン一覧画面
    - `src/components/plugin/PluginCard.tsx` — プラグインカード（名前、説明、有効/無効、Tool一覧表示）
    - _Requirements: 11.6_

  - [~] 19.6 Memory・Thought閲覧画面実装
    - `src/components/memory/MemoryView.tsx` — Memory一覧・編集・削除画面
    - `src/components/thought/ThoughtView.tsx` — Thought履歴閲覧画面、思考中インジケーター
    - _Requirements: 4.3, 4.6, 5.4_

- [ ] 20. フロントエンドHooks実装
  - [~] 20.1 カスタムHooks実装
    - `src/hooks/useChat.ts` — チャット操作（送信、ストリーミング受信、再送信）
    - `src/hooks/useCharacter.ts` — キャラクター操作（作成、編集、削除）
    - `src/hooks/useAudio.ts` — TTS音声再生制御（tts:audio Event listen、再生状態管理）
    - `src/hooks/useAttachment.ts` — ファイル添付操作（ファイル選択、ドラッグ&ドロップ、プレビュー）
    - `src/hooks/usePlugin.ts` — プラグイン操作（一覧取得、有効/無効切り替え）
    - _Requirements: 2.5, 6.7, 10.1_

- [ ] 21. Checkpoint - フロントエンド確認
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 22. CI/CD・ドキュメント
  - [~] 22.1 GitHub Actions CI設定
    - `.github/workflows/ci.yml` 作成
    - バックエンド: `cargo fmt --check`, `cargo clippy`, `cargo test`
    - フロントエンド: `pnpm lint`, `pnpm type-check`, `pnpm test`
    - ビルド検証: `cargo tauri build --debug`
    - _Requirements: 8.3_

  - [~] 22.2 README作成
    - セットアップ手順（Rust, Node.js, pnpm インストール）
    - 必要な環境変数一覧
    - 開発コマンド（dev, build, test）
    - 使用方法
    - _Requirements: 8.4_

- [ ] 23. 最終統合・結合テスト
  - [~] 23.1 フロントエンド-バックエンド結合確認
    - 全Tauri Command呼び出しの動作確認
    - 全Tauri Event受信の動作確認
    - ストリーミングレスポンスのE2E動作確認
    - ファイル添付フローのE2E動作確認
    - Tool Use フローのE2E動作確認
    - _Requirements: 全体統合_

  - [~] 23.2 フロントエンドプロパティテスト
    - fast-checkによる状態管理ロジックのプロパティテスト
    - Character/Chat/Config storeの状態遷移検証
    - _Requirements: 全体_

- [ ] 24. Final checkpoint - 全テスト通過確認
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- `*` 付きタスクはオプション（スキップ可能）
- 各タスクは具体的なRequirements番号を参照し、トレーサビリティ確保
- チェックポイントで段階的に品質検証
- プロパティテストは対応する実装タスクの直後に配置（早期エラー検出）
- バックエンド（Rust）→ フロントエンド（TypeScript/React）の順で実装し、依存関係を解消
