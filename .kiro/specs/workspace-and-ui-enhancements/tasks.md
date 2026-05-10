# Implementation Tasks (Workspace and UI Enhancements)

## Phase 1: データベース拡張と永続化基盤
目的: セッション固有のプラグイン設定（`file_ops` のディレクトリごとの権限リスト）を保存できる基盤を作る。
- [ ] `src-tauri/src/db/migrations.rs`: `chat_plugin_configs` テーブルのマイグレーションを追加。
- [ ] `src-tauri/src/models/chat.rs`: `ChatPluginConfig` 構造体を定義。
- [ ] `src-tauri/src/db/repositories/chat_plugin_config.rs`: CRUD操作を実装。
- [ ] `src-tauri/src/commands/`: `get_plugin_config` と `update_plugin_config` コマンドを追加。

## Phase 2: Pluginアーキテクチャの拡張 (コンテキスト渡し)
目的: プラグイン実行時にセッションIDや設定情報へアクセスできるようにする。
- [ ] `src-tauri/src/models/plugin.rs`: `ToolCall` 等に `session_id` を渡せるよう修正。
- [ ] `src-tauri/src/chat/engine.rs`: `execute_tool` 実行時に `config_json` (ACL) を渡す。

## Phase 3: file_ops プラグインの拡張 (ディレクトリ権限と画像対応)
- [ ] `src-tauri/src/plugin/builtin/file_ops.rs`: `validate_path` メソッドを改修し、ACLリストによる検証を行う。
- [ ] `src-tauri/src/plugin/builtin/file_ops.rs`: `request_directory_access` ツールを追加。
- [ ] `src-tauri/src/plugin/builtin/file_ops.rs`: `read_image` ツールを追加し、Base64エンコードして返す。
- [ ] `src-tauri/src/chat/engine.rs`: ツールの結果から `[IMAGE_BASE64]` を抽出して `ChatMessage` の `images` に追加する処理を実装。

## Phase 4: UI 右ペインのツールごとのアコーディオン化とリサイズ対応
目的: ツール設定を折り畳み可能なセクションにまとめ、使いやすくする。
- [ ] `src/components/chat/ToolManagementPane.tsx`:
      - 各プラグインごとに `<details>` またはステートによる開閉可能なアコーディオン UI を実装する。
      - アコーディオンの内部に、そのプラグインのツールごとのトグルスイッチを配置する。
      - プラグイン名が `file_ops` の場合、アコーディオン内部の末尾に `<FileOpsDirectoryManager />` を配置する。下部の固定配置は廃止する。
- [ ] `src/components/chat/ChatView.tsx`:
      - `ToolManagementPane` のラッパー要素の横にドラッグ用のハンドルを追加。
      - `paneWidth` ステートとマウスイベントを使用してリサイズ機能を実装。

## Phase 5: テーマ (ダーク/ライト) 切り替えとスクロールバー対応
目的: アプリ全体でテーマの切り替えをサポートし、スクロールバーを見やすくする。
- [ ] `src/styles/globals.css`:
      - `::-webkit-scrollbar` および `::-webkit-scrollbar-thumb` に `.dark` クラス時の色指定を追加し、ダークモードでも見やすいスクロールバーにする。
- [ ] `src/stores/ui.store.ts` (または `config.store.ts`):
      - `theme` 状態 ('light' | 'dark' | 'system') の管理と、`document.documentElement.classList` への `.dark` クラス適用/解除ロジックを追加。
- [ ] `src/components/sidebar/Sidebar.tsx` または `ChatHeaderControls.tsx`:
      - ライトモード・ダークモードを切り替えるトグルボタン (Sun/Moonアイコン) を実装・配置する。

## Phase 6: テストと動作確認
- [ ] ディレクトリ権限 (ACL) と Vision API が正しく機能するか確認。
- [ ] アコーディオンが開閉し、`file_ops` の中にディレクトリ管理UIが正しく表示されるか確認。
- [ ] 右ペインのリサイズが滑らかに動作するか確認。
- [ ] テーマ切り替えボタンを押すとダークモード/ライトモードが切り替わり、スクロールバーも適切に色が変化することを確認。
