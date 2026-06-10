# Implementation Plan: Thinking/Reasoning Content Support

## Overview

LLMプロバイダーが返すthinking/reasoning contentを受信・保持・表示する機能の実装。バックエンド（Rust/Tauri）でのストリーミング受信・イベント伝達・DB永続化と、フロントエンド（React/TypeScript）での状態管理・UI表示を段階的に実装する。

## Tasks

- [x] 1. バックエンドモデル層の拡張
  - [x] 1.1 LLMResponse列挙型をnamed fieldsに変更しthinkingフィールドを追加
    - `LLMResponse::Text(String)` → `LLMResponse::Text { content: String, thinking: Option<String> }`
    - `LLMResponse::ToolCalls(Vec<ToolCall>)` → `LLMResponse::ToolCalls { calls: Vec<ToolCall>, thinking: Option<String> }`
    - `text()`, `into_text()`, `is_tool_calls()` メソッドを新構造に合わせて更新
    - 既存の全`LLMResponse`使用箇所（engine.rs、各プロバイダー実装）のパターンマッチを修正
    - _Requirements: 1.5, 1.6_

  - [x] 1.2 ChatStreamEvent構造体にthinkingフィールドを追加
    - `pub thinking: Option<String>` フィールドを追加
    - 既存のイベント構築箇所でthinking: Noneを明示的に設定（後方互換性維持）
    - _Requirements: 2.1, 2.5, 2.6_

  - [x] 1.3 ChatMessageRecord構造体にthinking_contentフィールドを追加
    - Rust側: `pub thinking_content: Option<String>` を追加
    - 既存のChatMessageRecord構築箇所で`thinking_content: None`を設定
    - _Requirements: 4.1, 4.4_

  - [x] 1.4 thinking_content切り詰め関数を実装
    - `truncate_thinking_content(content: &str) -> &str` — 200,000文字上限、UTF-8境界考慮
    - Chat Engineのメッセージ保存時に呼び出し
    - _Requirements: 4.5_

  - [x] 1.5 Property test: thinking content truncation invariant
    - **Property 6: Thinking content truncation invariant**
    - proptestでランダムな長さの文字列（マルチバイト含む）を生成し、切り詰め後の長さ≤200,000かつ元文字列のprefixであることを検証
    - **Validates: Requirements 4.5**

- [x] 2. LLMClient trait拡張とThinkTagBuffer実装
  - [x] 2.1 LLMClient traitのchat_streamシグネチャをStreamCallbacksタプルに変更
    - `callback: Box<dyn Fn(String) + Send>` → `callbacks: StreamCallbacks`（text_callback, thinking_callback のタプル）
    - StreamCallbacks型エイリアスを定義
    - 全プロバイダー実装（OpenAICompatibleClient）のシグネチャを更新
    - _Requirements: 1.1, 1.2, 1.3_

  - [x] 2.2 ThinkTagBuffer構造体を新規実装
    - `src-tauri/src/llm/think_tag_buffer.rs` を新規作成
    - `<think>`タグのチャンク境界またぎ検出バッファ
    - `process_chunk(&mut self, chunk: &str) -> (Vec<String>, Vec<String>)` — (text_parts, thinking_parts)
    - `flush(&mut self) -> (Vec<String>, Vec<String>)` — ストリーム終了時の確定処理
    - _Requirements: 1.4_

  - [x] 2.3 Property test: Think tag extraction across chunk boundaries
    - **Property 2: Think tag extraction across chunk boundaries**
    - proptestでランダムなHTML文字列を生成し、ランダムな位置で分割。一括処理と分割処理の結果を比較
    - **Validates: Requirements 1.4**

  - [x] 2.4 OpenAI互換プロバイダーにthinking content検出を実装
    - SSEチャンクの`reasoning_content`/`reasoning`フィールドを検出
    - thinking_callbackで通知し、thinking文字列を蓄積
    - 最終LLMResponseにthinkingフィールドを設定
    - ThinkTagBufferを使用して`<think>`タグベースの検出にも対応
    - _Requirements: 1.2, 1.4, 1.5_

  - [x] 2.5 Anthropicプロバイダーのthinking/redacted_thinking検出を実装
    - `thinking`タイプcontent_blockのテキストデルタをthinking_callbackで通知
    - `redacted_thinking`ブロック検出時に`[REDACTED_THINKING]`マーカーをthinking_callbackで通知
    - 通常thinking + redacted_thinkingの混在時に出現順序を保持
    - _Requirements: 1.1, 6.1, 6.4_

  - [x] 2.6 Geminiプロバイダーのthought part検出を実装
    - `thought: true`フラグを持つpartのテキストをthinking_callbackで通知
    - _Requirements: 1.3_

  - [x] 2.7 Property test: Thinking content separation from text content
    - **Property 1: Thinking content separation from text content**
    - ランダムなSSEチャンク列を生成し、thinking/textの分離を検証
    - **Validates: Requirements 1.1, 1.2, 1.3, 1.5**

  - [x] 2.8 Property test: Thinking block type and order preservation
    - **Property 7: Thinking block type and order preservation**
    - Anthropic thinking/redacted_thinkingブロックのランダム列を生成し、順序・型保持を検証
    - **Validates: Requirements 6.1, 6.4**

- [x] 3. Checkpoint - バックエンドモデル・LLMClient層の確認
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Chat Engine層のthinking content伝達
  - [x] 4.1 Chat Engineのchat_stream呼び出しをStreamCallbacksに対応
    - text_callback: 既存のchunk emit処理
    - thinking_callback: ChatStreamEvent.thinkingフィールドにデルタを設定してemit
    - 同時受信時は単一のChatStreamEventで両フィールドを設定
    - _Requirements: 2.2, 2.3, 2.4_

  - [x] 4.2 Chat Engineのメッセージ保存にthinking_contentを含める
    - LLMResponse完了後、thinkingフィールドをtruncate_thinking_contentで切り詰めてDBに保存
    - tool_break発生時の確定メッセージにもthinking_contentを付与
    - _Requirements: 4.2, 4.3, 4.4, 4.5_

  - [x] 4.3 Property test: Stream event field assignment invariant
    - **Property 3: Stream event field assignment invariant**
    - thinking/textデルタをランダムに生成し、emitされるイベントのフィールドを検証
    - **Validates: Requirements 2.2, 2.3, 2.5**

- [x] 5. DBマイグレーション
  - [x] 5.1 chat_messagesテーブルにthinking_contentカラムを追加
    - `migrations.rs`のCREATE TABLE文に`thinking_content TEXT`を追加
    - `run_migrations()`に`ALTER TABLE chat_messages ADD COLUMN thinking_content TEXT`を追加（既存DBへの対応、エラー無視）
    - chat_repo のinsert_message / get_messagesクエリにthinking_contentカラムを含める
    - _Requirements: 4.1, 4.2, 4.3_

  - [x] 5.2 Property test: DB persistence round-trip for thinking content
    - **Property 5: DB persistence round-trip for thinking content**
    - ランダムなthinking_content文字列でinsert/getの往復を検証
    - **Validates: Requirements 4.2, 4.3**

- [x] 6. Checkpoint - バックエンド全体の動作確認
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. フロントエンド型定義とストアの拡張
  - [x] 7.1 TypeScript型定義にthinking_contentフィールドを追加
    - `ChatMessageRecord`に`thinking_content?: string | null`を追加
    - _Requirements: 4.1_

  - [x] 7.2 Chat Storeにthinking状態管理を追加
    - `streamingThinkingContent: string` 状態フィールド追加（初期値: 空文字列）
    - `isThinking: boolean` 状態フラグ追加
    - `appendThinkingChunk(chunk: string)` アクション追加
    - ストリーミング開始時にthinkingバッファをリセット
    - thinking chunk受信時に`isThinking: true`を設定
    - 最初のtext chunk受信時に`isThinking: false`に切り替え
    - finishStreaming時にthinkingContentをメッセージレコードに含めてコミット
    - ストリーミング完了/中断時にisThinkingをfalseにリセット
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 7.1, 7.3, 7.4_

  - [x] 7.3 Chat Storeのtool_break処理にthinking content保存を追加
    - commitPreToolContent時に蓄積済みthinkingContentをメッセージのthinking_contentに設定
    - tool_break後にthinkingバッファをリセット
    - thinking空時はnullのまま保持
    - _Requirements: 8.1, 8.2, 8.3_

  - [x] 7.4 useChat hookのstream eventハンドラにthinkingフィールド処理を追加
    - `chat:stream`イベント受信時にevent.thinkingを検出してappendThinkingChunkを呼び出し
    - _Requirements: 3.2_

  - [x] 7.5 Property test: Thinking content accumulation preserves concatenation
    - **Property 4: Thinking content accumulation preserves concatenation**
    - ランダムなデルタ列を生成し、最終accumulated valueが連結と一致することを検証（Vitest + fast-check）
    - **Validates: Requirements 3.2, 3.3**

  - [x] 7.6 Property test: Tool break preserves accumulated thinking content
    - **Property 8: Tool break preserves accumulated thinking content**
    - ランダムなthinkingデルタ後にtool_breakを発火し、確定バブルへの関連付けを検証（Vitest + fast-check）
    - **Validates: Requirements 8.1, 8.2**

- [x] 8. UIコンポーネントの実装
  - [x] 8.1 ThinkingSectionコンポーネントを新規作成
    - `src/components/chat/ThinkingSection.tsx` を新規作成
    - Props: `thinkingContent`, `isStreaming`, `isRedacted`, `defaultExpanded`
    - 折り畳みトグル（ChevronRight/ChevronDown）
    - ストリーミング中はデフォルト展開、完了後はデフォルト折り畳み
    - 背景色の差異・左ボーダー・イタリック体で通常コンテンツと視覚的区別
    - `[REDACTED_THINKING]`マーカー検出時に「思考内容は非表示です」プレースホルダー表示
    - CSS max-height + スクロールで長大なthinking contentに対応
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 6.3_

  - [x] 8.2 MessageBubbleにThinkingSectionを統合
    - アシスタントメッセージバブルの上部にThinkingSectionを配置
    - `message.thinking_content`が存在する場合のみレンダリング
    - ストリーミング中は`streamingThinkingContent`を使用
    - _Requirements: 5.1, 5.6_

  - [x] 8.3 ストリーミング中のThinking状態インジケーター表示
    - `isThinking`フラグがtrueの場合に「思考中...」テキストとアニメーション付きインジケーターを表示
    - 点滅またはバウンスドットアニメーション
    - _Requirements: 7.1, 7.2_

  - [x] 8.4 ThinkingSectionコンポーネントのユニットテスト
    - デフォルト折り畳み状態の確認
    - トグル動作の確認
    - redacted表示の確認
    - thinking_content空時に非表示の確認
    - _Requirements: 5.1, 5.2, 5.3, 5.6, 6.3_

- [x] 9. 結合とワイヤリング
  - [x] 9.1 全層の結合テスト: thinking付きストリーム→DB保存→履歴取得→UI表示
    - Chat Engineからのthinking付きChatStreamEvent発行を確認
    - DB保存後のget_messagesでthinking_contentが含まれることを確認
    - フロントエンドのメッセージ一覧にthinking_contentが反映されることを確認
    - _Requirements: 1.5, 2.2, 4.2, 4.3, 5.1_

  - [x] 9.2 後方互換性テスト
    - thinking=nullのChatStreamEventで既存動作が維持されることを確認
    - thinking_content=Noneの既存メッセージが正常に表示されることを確認
    - _Requirements: 2.5, 2.6, 4.4, 5.6_

- [x] 10. Final checkpoint - 全テスト通過確認
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties (proptest for Rust, fast-check for TypeScript)
- Unit tests validate specific examples and edge cases
- 既存の`LLMResponse`使用箇所が多いため、タスク1.1の変更は全プロバイダー・engine.rsに波及する

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1", "1.2", "1.3", "1.4"] },
    { "id": 1, "tasks": ["1.5", "2.1", "2.2", "5.1", "7.1"] },
    { "id": 2, "tasks": ["2.3", "2.4", "2.5", "2.6"] },
    { "id": 3, "tasks": ["2.7", "2.8", "4.1", "4.2"] },
    { "id": 4, "tasks": ["4.3", "5.2", "7.2", "7.3", "7.4"] },
    { "id": 5, "tasks": ["7.5", "7.6", "8.1"] },
    { "id": 6, "tasks": ["8.2", "8.3", "8.4"] },
    { "id": 7, "tasks": ["9.1", "9.2"] }
  ]
}
```
