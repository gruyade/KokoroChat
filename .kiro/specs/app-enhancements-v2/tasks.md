# 実装計画: App Enhancements V2

## 概要

AI Character Chatアプリの包括的機能強化。非同期オペレーションキュー（基盤）→ キャラクターエクスポート/インポート → UI/UX改善の順で実装する。バックエンドはRust、フロントエンドはReact + TypeScript + Zustand。

## タスク

- [x] 1. グローバルオペレーションキュー（Zustandストア）の実装
  - [x] 1.1 `src/stores/operation-queue.ts` を新規作成
    - Zustandストアとしてグローバルシングルトンキューを実装
    - `pendingCount`, `processing`, `currentTaskLabel`, `enqueue` を公開
    - キュー内タスクは追加順に逐次実行、失敗時はconsole.errorでログ出力し次タスクへ継続
    - コンポーネントのアンマウントに依存しないモジュールスコープのキュー配列を使用
    - `src/stores/index.ts` からre-export追加
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

  - [x] 1.2 オペレーションキューのプロパティテスト作成
    - **Property 3: キュー順序保証と障害耐性**
    - **Property 5: キュー状態の正確性**
    - テストファイル: `src/stores/operation-queue.test.ts`
    - ランダムなタスク列（成功/失敗混在）を生成し、実行順序と状態の正確性を検証
    - **Validates: Requirements 4.3, 4.4, 4.5**

- [x] 2. キャラクターエクスポート機能（バックエンド）
  - [x] 2.1 エクスポート用データ型とコマンドの実装
    - `src-tauri/src/commands/character.rs` に `export_character` コマンドを追加
    - `ExportOptions` 構造体（include_chats, include_thoughts, include_memories）を定義
    - `CharacterExportData` 構造体（version, exported_at, character, chat_sessions?, thoughts?, memories?）を定義
    - DBからキャラクター設定・チャット履歴・思考・記憶を取得しJSON構造に組み立て
    - `src-tauri/src/commands/mod.rs` にコマンド登録
    - `src-tauri/src/lib.rs` のinvoke_handler登録
    - _Requirements: 1.2, 1.3, 1.4, 1.5, 1.6_

  - [x] 2.2 インポート用コマンドの実装
    - `src-tauri/src/commands/character.rs` に `import_character` コマンドを追加
    - `ImportOptions` 構造体（include_chats, include_thoughts, include_memories）を定義
    - JSONデータのバリデーション（version, 必須フィールド確認）
    - 新規キャラクターIDを生成し、関連データのIDも再生成してDB保存
    - トランザクション内で実行し、失敗時はロールバック
    - _Requirements: 2.3, 2.4, 2.5, 2.6, 2.8, 2.9_

  - [x] 2.3 エクスポート/インポートのプロパティテスト
    - **Property 1: エクスポート/インポート ラウンドトリップ**
    - テストファイル: `src-tauri/src/character/property_tests.rs` に追加
    - proptestでランダムなキャラクターデータを生成 → export → import → re-export → 内容比較（ID・タイムスタンプ除外）
    - **Validates: Requirements 1.2, 1.3, 1.4, 1.5, 2.3, 2.4, 2.5, 2.6, 2.9**

  - [x] 2.4 不正フォーマット拒否のプロパティテスト
    - **Property 2: 不正フォーマット拒否**
    - テストファイル: `src-tauri/src/character/property_tests.rs` に追加
    - proptestで必須フィールド欠落・不正型のJSONを生成 → import → エラー返却確認・DB変更なし確認
    - **Validates: Requirements 2.8**

- [x] 3. キャラクターエクスポート/インポート機能（フロントエンド）
  - [x] 3.1 エクスポートダイアログUIの実装
    - `src/components/character/ExportDialog.tsx` を新規作成
    - チャット履歴・思考・記憶の各オプションチェックボックス
    - エクスポート実行ボタン → `invoke('export_character')` → Tauri file dialog（save）でJSON保存
    - `@tauri-apps/plugin-dialog` の `save` と `@tauri-apps/plugin-fs` の `writeTextFile` を使用
    - エラー時はトースト通知
    - _Requirements: 1.1, 1.6, 1.7_

  - [x] 3.2 インポートダイアログUIの実装
    - `src/components/character/ImportDialog.tsx` を新規作成
    - Tauri file dialog（open）でJSONファイル選択 → `readTextFile` で読み込み → JSON解析
    - インポートオプションダイアログ（チャット・思考・記憶の選択）
    - `invoke('import_character')` 実行 → 成功時にキャラクター一覧更新 + トースト
    - 不正フォーマット時はエラーメッセージ表示
    - _Requirements: 2.1, 2.2, 2.7, 2.8_

  - [x] 3.3 CharacterCard・CharacterViewへの統合
    - `src/components/character/CharacterCard.tsx` にエクスポートボタン（Download アイコン）追加
    - `src/components/character/CharacterView.tsx` にインポートボタン（Upload アイコン）追加
    - エクスポートボタン押下 → ExportDialog表示
    - インポートボタン押下 → ImportDialog表示
    - _Requirements: 1.1, 2.1_

- [x] 4. チェックポイント - エクスポート/インポート機能の動作確認
  - すべてのテストが通ることを確認し、不明点があればユーザーに質問する。

- [x] 5. モデル設定UIの改善
  - [x] 5.1 モデル一覧取得バックエンドコマンドの実装
    - `src-tauri/src/commands/config.rs` に `fetch_available_models` コマンドを追加
    - `base_url` と `api_key` を受け取り、`GET {base_url}/models` を実行
    - レスポンスの `data[].id` を `Vec<String>` として返却
    - ネットワークエラー・認証エラー時は適切な `AppError` を返却
    - `src-tauri/src/lib.rs` のinvoke_handler登録
    - _Requirements: 8.4, 8.5, 8.6_

  - [x] 5.2 プロバイダー選択とモデル一覧UIの実装
    - `src/types/config.ts` に `LLMProvider` 型を追加
    - `ModelSettings` に `provider?: LLMProvider` フィールドを追加（後方互換）
    - `src/components/settings/ModelConfigForm.tsx` を改修:
      - プロバイダー選択コンボボックス（OpenAI, Anthropic, Google, OpenAI互換）
      - 既知プロバイダー選択時: Base URLフィールドをオプション化（デフォルト値自動適用）
      - OpenAI互換選択時: Base URLフィールドを必須表示
      - 「モデル一覧取得」ボタン追加 → `invoke('fetch_available_models')` 実行
      - 取得成功: ドロップダウン + 手動入力のコンボボックス
      - 取得失敗: エラー表示 + 手動テキスト入力のみ
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6_

- [x] 6. 手動メモリ生成ボタンの実装
  - [x] 6.1 バックエンドコマンドの追加
    - `src-tauri/src/commands/memory.rs` に `generate_memory_manual` コマンドを追加
    - `MemoryManager` に `force_compress` メソッドを追加（閾値チェックスキップで強制実行）
    - `src-tauri/src/lib.rs` のinvoke_handler登録
    - _Requirements: 5.2_

  - [x] 6.2 ChatHeaderControlsへのボタン追加
    - `src/components/chat/ChatHeaderControls.tsx` にメモリ生成ボタン（Brain + Sparkles等のアイコン）追加
    - ボタン押下 → `invoke('generate_memory_manual', { sessionId })` 実行
    - ローディング状態表示、重複実行防止
    - 成功時: トースト「記憶を生成した」
    - 失敗時: エラートースト
    - currentSessionIdがnull時はボタンdisabled
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

- [x] 7. システムメッセージのUX変更
  - [x] 7.1 MessageBubbleのシステムメッセージ表示変更
    - `src/components/chat/MessageBubble.tsx` を改修
    - `isSystemMessage` の場合の表示を中央寄せバッジから右寄せバブル（ユーザーメッセージスタイル）に変更
    - ホバー時に編集・削除ボタンを表示
    - 編集確定時: `editAndResend` を呼び出し（後続メッセージリセット + 再送信）
    - 削除時: `onDelete` を呼び出し
    - _Requirements: 6.1, 6.2, 6.3, 6.4_

- [x] 8. UI/UXバグ修正・改善（一括）
  - [x] 8.1 TTS WIPラベルの追加
    - `src/components/settings/SettingsView.tsx` のTABS定義に `badge?: string` フィールド追加
    - TTSタブに `badge: 'WIP'` を設定
    - タブボタンレンダリングにバッジ表示ロジック追加（黄色の小さなバッジ）
    - _Requirements: 7.1_

  - [x] 8.2 キャラクターフォームのスクロール修正
    - `src/components/character/CharacterView.tsx` のフォーム親コンテナから `overflow-y-auto max-h-[70vh]` を削除
    - CharacterView全体のコンテンツ領域で `overflow-y-auto` を制御
    - _Requirements: 3.1, 3.2, 3.3_

  - [x] 8.3 条件付きボタン表示制御
    - `src/components/chat/ChatHeaderControls.tsx`: `useConfigStore` から設定読み取り
      - `thought.enabled === false` → 思考一時停止ボタン非表示
      - `spontaneous.enabled === false` → 自発的発話一時停止ボタン非表示
    - `src/components/chat/MessageBubble.tsx`: `tts.enabled === false` → 音声生成ボタン非表示（既存実装確認・修正）
    - _Requirements: 9.1, 9.2, 9.3, 9.4_

  - [x] 8.4 ホバーアクションボタンのバグ修正
    - `src/components/chat/MessageBubble.tsx` のアクションボタン表示ロジック修正
    - `pointer-events-none` を `opacity-0 invisible` に変更（pointer-events-autoを常に維持）
    - `onMouseLeave` でのステート更新に `requestAnimationFrame` を使用し、ボタンクリック中のホバー解除を防止
    - 削除アニメーション後のホバー検出が正常に動作することを確認
    - _Requirements: 10.1, 10.2, 10.3, 10.4_

- [x] 9. チェックポイント - 全機能の統合確認
  - すべてのテストが通ることを確認し、不明点があればユーザーに質問する。

## 備考

- `*` マーク付きタスクはオプション（スキップ可能）
- 各タスクは特定の要件を参照しトレーサビリティを確保
- チェックポイントで段階的に動作検証
- プロパティテストは設計文書のCorrectness Propertiesに基づく
- 既存の `src/hooks/useOperationQueue.ts` は非推奨とし、新しいZustandストア（タスク1.1）に段階的に移行
