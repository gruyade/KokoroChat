# LLM Tool Use Design

## アーキテクチャ

1. **Plugin System Integration & Permission Management**
   - `PluginSystem` および `PluginRegistry` はグローバルなツールの状態（有効/無効）を管理する。
   - `ChatSession` に紐づくツール許可状態（チャット内設定）を保存するDBテーブル (`chat_tool_permissions` など) を追加。
   - チャット設定には「未設定」状態を設けず、グローバル設定の値をデフォルト値として必ず（有効 または 無効 に）設定する。
   - 許可優先順位: 
     1. グローバルで `disabled` → 常に使用不可（チャット内設定に関わらず強制無効化・選択不可）
     2. グローバルで `enabled` かつ チャットで `disabled` → 使用不可
     3. グローバルで `enabled` かつ チャットで `enabled` → 使用可能
   - `ChatEngine` はツールのリストを取得する際、グローバル状態とチャット内設定を掛け合わせて、最終的な利用可能ツールリストを決定する。
   - カスタムツール作成の仕様は `custom_tool_spec.md` に定義された HTTP Webhook / CLI ベースの動的実行基盤を実装する。デフォルトツールには削除不可フラグ（アンインストール不可）を持たせる。

2. **LLM Client Streaming Enhancement**
   - ストリーミングレスポンス (`chat_stream`) で Tool Call を受信するための変更を検討する。
   - アプローチ案A: `chat_stream` を拡張して `ToolCalls` や `Text` のイベントをコールバックで逐次返せるようにする。
   - アプローチ案B: ストリーミング中には Tool Call に関連するチャンクをバッファリングし、Tool Call だった場合はコールバックにはテキストとして流さず、結果として `LLMResponse::ToolCalls` を返すように変更する。

3. **Execution Loop (Chat Engine)**
   - `send_message` / `regenerate` / `edit_and_resend` メソッドで再帰的な処理を追加。
   - **ステップ:**
     1. User Message を DB 保存し、コンテキスト構築。
     2. `LLMClient` を呼び出す。
     3. 戻り値が `Text` の場合: 通常通り終了（またはストリーミング完了）。
     4. 戻り値が `ToolCalls` の場合:
        - フロントエンドに `tool:executing` イベントを発火。
        - ツール呼び出しメッセージ (Assistant role, tool_calls含む) を DB に保存。
        - `PluginSystem::handle_tool_calls` にディスパッチ。
        - ツール実行結果 (Tool role) をコンテキストに追加し、DB に保存。
        - 再度 2. からループ。

4. **Custom UI Component Rendering Mechanism**
   - ツールが特定のJSONフォーマットや独自Markdownタグ（例: `<CustomWidget type="..." data={...} />`）を返せるようにする。
   - フロントエンドの `MarkdownRenderer` または専用の `ToolResultRenderer` を拡張し、これらの特定のタグ/ペイロードを検知した際に、対応する React コンポーネントに動的に置換して描画する。
   - 状態を持たせる必要がある複雑なコンポーネントのために、必要に応じてフロントエンド側のイベントリスナー(`tool:ui_update` など)も併用可能とする仕様を策定する。

5. **Database & Models**
   - `ChatMessageRecord` の `tool_calls` フィールドにシリアライズされたツール呼び出し内容を保存。
   - `tool_call_id` フィールドに、実行結果を関連付ける ID を保存。

6. **Frontend Indicator & Tool Management UI**
   - チャットストリーム中の `tool:executing` をトリガーとして `ToolCallIndicator` コンポーネントを表示。
   - ツール名などをプログレスメッセージとして表示する。
   - チャット画面右側に開閉可能なツール管理ペインを配置する。
