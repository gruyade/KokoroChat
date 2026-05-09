# Tool Use Implementation Tasks

## Phase 1: Core System Integration
- [x] `AppState` に `PluginSystem` を追加し、起動時に初期化する
- [x] `DefaultChatEngine::new` の引数に `PluginSystem` を追加し、DIを行う
- [x] 組み込みツール (Calculator, WebSearch, FileOps 等) を `PluginRegistry` に登録する

## Phase 2: LLM Client Enhancements
**目的:** LLM クライアントが `tools` パラメータを受け取り、ストリーミング中に Tool Call 応答を適切にパースできるようにする。
- [x] `LLMClient::chat_stream` メソッドのシグネチャを変更し、`tools` 引数を受け取れるようにする
- [x] OpenAI, Anthropic, Gemini クライアントの `build_request_body` で、ストリーミング時にも tools を含めるように修正
  - `src-tauri/src/llm/client.rs` の各プロバイダのロジックにおいて、`stream: true` の場合でも `tools` 配列を含めるようにする。
- [x] ストリーミングパーサー (SSE) を拡張し、Tool Call のチャンクを受信した際に適切にハンドリング（またはバッファリング）する実装を追加
  - テキストチャンクと ToolCall チャンクを区別するため、コールバックの引数または戻り値を `enum LLMStreamChunk { Text(String), ToolCall(...) }` 等に変更する。
- [x] ツール呼び出し応答と通常のテキスト応答を区別して返す仕組み (`LLMStreamResponse` 列挙型などの導入) を検討・実装
  - ToolCallのストリーミングは複雑なため、チャンク受信時にJSON引数をバッファリングし、ToolCall完了時（ストリーム終了時）に `LLMResponse::ToolCalls(Vec<ToolCall>)` として返す設計で統一する。

## Phase 3: Chat Engine Execution Loop
**目的:** チャットエンジン内で LLM の応答が Tool Call だった場合、自動的にツールを実行して結果を再度LLMに投げる再帰ループを構築する。
- [x] `ChatEngine::send_message` にツール実行の再帰ループ（または while ループ）を実装する
  - LLM呼び出し前に、グローバルの `PluginSystem::get_enabled_tools()` と DBの `chat_tool_permissions` をマージ（グローバル無効は除外、未設定時はグローバル設定をデフォルトとする）して利用可能 `tools` リストを確定しLLMに渡す。
  - ループ処理: LLM が `ToolCalls` を返した場合、`tool:executing` イベントを発火 -> `PluginSystem::handle_tool_calls` で実行 -> 結果を `ChatMessageRecord` (`role: tool`) として DB 保存 -> コンテキストに追記して再度 `chat_stream` を呼び出す。
  - `Text` を返すか、最大再帰回数（例: 5回）に達するまでループする。
- [x] `regenerate` および `edit_and_resend` メソッドにも同様のループ処理を適用する
  - 再生成・編集フローでも上記のツール実行再帰ロジックを共通利用できるようにリファクタリングする。

## Phase 4: Frontend UI / UX
**目的:** ユーザーが権限を管理でき、ツールの実行状態やカスタムUI結果を視覚的に確認できるようにする。
- [x] チャット内ツール管理UIの実装
  - チャット画面 (`ChatView`) の右側に、開閉可能なサイドバーペインを追加。
  - セッションに紐づくツール許可設定トグルを一覧表示し、グローバルで無効化されているものは `disabled` 状態でグレーアウト表示する。
- [x] グローバルツール管理画面の実装 (SettingsView 等)
  - `SettingsView` に「プラグイン/ツール管理」タブを追加し、カスタムツールの追加・編集・削除フォームを実装。
  - グローバルの有効化/無効化トグルを実装（デフォルトツールは削除ボタン非表示）。
- [x] `ChatView` で `tool:executing` イベントをリッスンし、ローディングインジケーター (`ToolCallIndicator` など) を表示する
- [x] メッセージ履歴のレンダリングにおいて、`tool_calls` を持つアシスタントメッセージと、`tool_call_id` を持つ結果メッセージを適切に折り畳み表示・スタイリングする
- [x] カスタムUIコンポーネントのレンダリング基盤実装
  - `src/components/chat/MarkdownRenderer.tsx` を拡張する。
  - `custom_tool_spec.md` に定義した `<ChatWidget type="..." data="..." />` タグを正規表現等で検知し、安全に JSON パースした上で対応する React コンポーネント (Map, Chart 等) を動的レンダリングする仕組みを構築する。

## Phase 5: Database & Implementation of Specs
**目的:** チャット別の権限とカスタムツールの設定を永続化するためのDB基盤を整備する。
- [x] DBマイグレーション: `chat_tool_permissions` テーブルの追加
  - `src-tauri/src/db/migrations.rs` に新しいマイグレーションを追加。
  - カラム定義: `session_id` (TEXT, FK), `tool_name` (TEXT), `is_enabled` (BOOLEAN), PRIMARY KEY (`session_id`, `tool_name`)
- [x] `custom_tool_spec.md` に基づく動的カスタムツール機構 (HTTP / CLI) のバックエンド実装
  - DBマイグレーション: `custom_tools` テーブルの追加 (`id`, `name`, `type`, `description`, `parameters_schema`, `config_json`, `created_at`)
  - `src-tauri/src/plugin/custom/` モジュールを新設し、DBからロードした `custom_tools` レコードを元に、動的に `PluginHandler` トレイトを実装するラッパー (`HttpToolHandler`, `CliToolHandler`) を作成・登録するロジックを実装。
- [x] ツール呼び出しを含む LLM のやり取りのモックテストを追加する
  - 権限マージロジック (`グローバル設定` × `チャット別設定`) のテスト。
  - カスタムツールハンドラのパース・実行テスト。
  - `MockLLMClient` を使用し、ToolCall が複数回返るシナリオのループ動作を確認。
- [x] 実際に組み込みツール（Calculatorなど）を使った E2E テスト・手動検証を行う
  - ツールを呼び出し、テキストと UI（カスタムタグ）の両方が正しくチャットに描画されるか確認。
