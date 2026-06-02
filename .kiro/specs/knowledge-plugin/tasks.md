# Implementation Plan: Knowledge Plugin

## Overview

Knowledge Pluginの実装計画。バックエンドはRust（Tauri）、フロントエンドはTypeScript/React（Zustand）で構成。既存の`file_ops`プラグインパターンに準拠し、DBスキーマ → リポジトリ → プラグイン → Tauriコマンド → エンジン拡張 → フロントエンドの順で段階的に構築する。

## Tasks

- [x] 1. データベーススキーマとリポジトリ層の実装
  - [x] 1.1 session_knowledge テーブルのマイグレーション追加
    - `src-tauri/src/db/migrations.rs` に session_knowledge テーブルのCREATE文を追加
    - id, session_id, file_name, content, size_bytes, enabled, injection_mode, created_at カラム定義
    - FOREIGN KEY(session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
    - UNIQUE(session_id, file_name) 制約
    - CHECK(injection_mode IN ('system_prompt', 'tool_reference')) 制約
    - session_id カラムへのインデックス作成
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_

  - [x] 1.2 KnowledgeEntry / KnowledgeEntryMeta モデル定義
    - `src-tauri/src/models/` 配下に knowledge モデルを追加
    - KnowledgeEntry（content含む完全表現）と KnowledgeEntryMeta（content除外の軽量表現）を定義
    - serde::Serialize, serde::Deserialize を derive
    - models/mod.rs にモジュール登録
    - _Requirements: 9.1, 10.3_

  - [x] 1.3 Knowledge リポジトリの実装
    - `src-tauri/src/db/repositories/knowledge.rs` を作成
    - add_knowledge: UPSERT（INSERT OR REPLACE）でエントリ追加、512KB超過チェック
    - remove_knowledge: session_id + file_name で削除
    - list_knowledge: session_id でメタデータ一覧取得（content除外、created_at昇順）
    - toggle_knowledge: enabled フラグ更新
    - set_injection_mode: injection_mode 更新（値バリデーション付き）
    - get_knowledge_content: session_id + file_name でcontent取得
    - get_system_prompt_entries: enabled=true かつ injection_mode=system_prompt のエントリ取得
    - get_tool_reference_entries: enabled=true かつ injection_mode=tool_reference のエントリ取得
    - repositories/mod.rs にモジュール登録
    - _Requirements: 1.1, 1.3, 1.4, 1.5, 1.6, 2.2, 2.5, 3.1, 3.3, 4.1, 4.2, 4.3, 5.1, 6.1, 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7, 10.8_

  - [x] 1.4 リポジトリ層のプロパティテスト
    - `src-tauri/src/db/property_tests.rs` に knowledge 関連テストを追加
    - **Property 1: Knowledge entry creation round-trip**
    - **Property 2: Upsert replaces existing entry**
    - **Property 3: Oversized content rejection**
    - **Property 4: Delete removes target and preserves others**
    - **Property 5: Toggle round-trip preserves entry**
    - **Property 7: Injection mode persistence**
    - **Property 12: list_knowledge returns ordered metadata without content**
    - **Validates: Requirements 1.1, 1.3, 1.4, 1.5, 1.6, 2.2, 3.1, 3.3, 4.1, 4.2, 8.2, 9.3, 10.3**

- [x] 2. Checkpoint - データベース層の動作確認
  - Ensure all tests pass, ask the user if questions arise.

- [x] 3. KnowledgePlugin の実装
  - [x] 3.1 KnowledgePlugin 構造体と PluginHandler trait 実装
    - `src-tauri/src/plugin/builtin/knowledge.rs` を作成
    - name() → "knowledge", description() を実装
    - tools() → get_knowledge ToolDefinition を返す（file_name パラメータ付き）
    - execute() → get_knowledge ツール実行（file_name引数でcontent返却、不一致時エラー+利用可能一覧）
    - builtin/mod.rs にモジュール登録
    - PluginSystem の初期化に KnowledgePlugin を追加
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

  - [x] 3.2 KnowledgePlugin のプロパティテスト
    - `src-tauri/src/plugin/property_tests.rs` に knowledge 関連テストを追加
    - **Property 9: get_knowledge tool availability reflects current state**
    - **Property 10: get_knowledge content retrieval**
    - **Validates: Requirements 6.1, 6.2, 6.4, 6.5, 10.6**

- [x] 4. Tauri コマンドの実装
  - [x] 4.1 Knowledge 用 Tauri コマンド作成
    - `src-tauri/src/commands/knowledge.rs` を作成
    - add_knowledge: session_id, file_name, content を受け取りエントリ追加（KnowledgeEntryMeta返却）
    - remove_knowledge: session_id, file_name で削除
    - list_knowledge: session_id でメタデータ一覧取得
    - toggle_knowledge: session_id, file_name, enabled で有効/無効切替
    - set_knowledge_injection_mode: session_id, file_name, injection_mode で注入モード変更
    - export_knowledge: session_id, file_name でcontent返却
    - commands/mod.rs にモジュール登録
    - `src-tauri/src/lib.rs` にコマンドハンドラ登録
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7, 10.8_

  - [x] 4.2 Tauri コマンドのユニットテスト
    - 各コマンドの正常系・異常系テスト
    - 存在しないエントリへの操作時のエラーレスポンス検証
    - 無効な injection_mode 値のバリデーションエラー検証
    - _Requirements: 10.7, 10.8_

- [x] 5. Engine 拡張（ナレッジ注入ロジック）
  - [x] 5.1 build_context への system_prompt モード注入実装
    - `src-tauri/src/chat/engine.rs` の build_context メソッドを拡張
    - get_system_prompt_entries で有効な system_prompt エントリを取得
    - 各エントリを "## {file_name}\n{content}" 形式でフォーマット
    - ベースシステムプロンプトの後、thoughts/memories の前に配置
    - エントリ0件の場合はナレッジセクションを追加しない
    - _Requirements: 5.1, 5.2, 5.3, 5.4_

  - [x] 5.2 tool_reference モードのツール定義フィルタリング実装
    - Engine のツール定義構築ロジックを拡張
    - tool_reference エントリが1件以上の場合のみ get_knowledge ツールを含める
    - ツールの parameter description に利用可能な file_name 一覧を列挙
    - エントリ0件の場合は get_knowledge ツールを除外
    - enabled/injection_mode 変更後の次回リクエストで反映
    - _Requirements: 6.1, 6.4, 6.5, 3.2, 3.4_

  - [x] 5.3 Engine 拡張のプロパティテスト
    - `src-tauri/src/chat/property_tests.rs` に knowledge 注入テストを追加
    - **Property 6: Engine enabled-state filter**
    - **Property 8: System prompt injection ordering and format**
    - **Validates: Requirements 3.2, 3.4, 5.1, 5.2, 5.3, 5.4, 2.3**

- [x] 6. Checkpoint - バックエンド全体の動作確認
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. フロントエンド TypeScript 型定義とストア
  - [x] 7.1 Knowledge 用 TypeScript 型定義
    - `src/types/knowledge.ts` を作成
    - InjectionMode 型（'system_prompt' | 'tool_reference'）
    - KnowledgeEntryMeta インターフェース（id, file_name, size_bytes, enabled, injection_mode, created_at）
    - `src/types/index.ts` にエクスポート追加
    - _Requirements: 10.3_

  - [x] 7.2 Knowledge Zustand ストア実装
    - `src/stores/knowledge.store.ts` を作成
    - entries, loading, error の状態管理
    - fetchEntries: list_knowledge invoke
    - addKnowledge: add_knowledge invoke + entries 更新
    - removeKnowledge: remove_knowledge invoke + entries 更新
    - toggleKnowledge: toggle_knowledge invoke + entries 更新
    - setInjectionMode: set_knowledge_injection_mode invoke + エラー時ロールバック
    - exportKnowledge: export_knowledge invoke
    - `src/stores/index.ts` にエクスポート追加
    - _Requirements: 4.3, 4.4, 10.1, 10.2, 10.3, 10.4, 10.5, 10.6_

- [x] 8. フロントエンド UI コンポーネント
  - [x] 8.1 KnowledgeSection コンポーネント実装
    - `src/components/chat/KnowledgeSection.tsx` を作成
    - DropZone: ファイルドロップ受付（UTF-8テキスト読み取り、512KBサイズチェック）
    - Knowledge 一覧表示: file_name, size_bytes（人間可読フォーマット）, enabled toggle, injection_mode select
    - 削除ボタン + 確認ダイアログ
    - エクスポートボタン（システムファイルダイアログ呼び出し）
    - disabled エントリの opacity: 0.5 表示
    - 空状態プレースホルダ表示
    - created_at 昇順のエントリ表示
    - _Requirements: 1.1, 1.6, 2.1, 2.4, 3.1, 3.3, 4.1, 4.2, 7.1, 7.2, 7.3, 8.2, 8.3, 8.5, 8.6_

  - [x] 8.2 ToolManagementPane への KnowledgeSection 統合
    - `src/components/chat/ToolManagementPane.tsx` を修正
    - Knowledge_Plugin をアコーディオンアイテムとして追加
    - アコーディオンヘッダにエントリ数バッジ表示（0件時は非表示）
    - アコーディオン展開時に KnowledgeSection を表示
    - _Requirements: 8.1, 8.4_

  - [x] 8.3 サイズフォーマットユーティリティのプロパティテスト
    - サイズフォーマット関数のユニットテスト/プロパティテスト
    - **Property 13: Size formatting correctness**
    - **Validates: Requirements 8.3**

- [x] 9. セッション削除時の CASCADE 動作確認
  - [x] 9.1 CASCADE 削除の統合テスト
    - セッション削除時に関連 knowledge エントリが自動削除されることをテストで確認
    - 既存のセッション削除フロー（DB の FOREIGN KEY CASCADE）が正しく動作することを検証
    - _Requirements: 9.2_

  - [x] 9.2 CASCADE 削除のプロパティテスト
    - **Property 11: Cascade delete removes knowledge entries**
    - **Validates: Requirements 9.2**

- [x] 10. Final checkpoint - 全テスト通過確認
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- タスクに `*` マークのあるサブタスクはオプション（スキップ可能）
- 各タスクは対応する Requirements への参照を含む
- チェックポイントで段階的に動作確認
- プロパティテストは設計ドキュメントの Correctness Properties セクションに基づく
- ユニットテストは特定のエッジケースとエラー条件を検証
- 既存の `file_ops.rs` プラグインパターンに準拠した構造で実装

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1", "1.2"] },
    { "id": 1, "tasks": ["1.3", "7.1"] },
    { "id": 2, "tasks": ["1.4", "3.1", "4.1"] },
    { "id": 3, "tasks": ["3.2", "4.2", "5.1", "5.2"] },
    { "id": 4, "tasks": ["5.3", "7.2"] },
    { "id": 5, "tasks": ["8.1"] },
    { "id": 6, "tasks": ["8.2", "8.3"] },
    { "id": 7, "tasks": ["9.1", "9.2"] }
  ]
}
```
