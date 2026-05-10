# Architecture & Design (Workspace and UI Enhancements)

## 1. データベース・データモデルの拡張
チャット（セッション）ごとにプラグインの高度な設定（`file_ops` のディレクトリ別権限など）を永続化するため、新しいテーブルを追加します。

### マイグレーション: `chat_plugin_configs` テーブル追加
```sql
CREATE TABLE IF NOT EXISTS chat_plugin_configs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    plugin_name TEXT NOT NULL,
    config_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE,
    UNIQUE(session_id, plugin_name)
);
```

### 許可ディレクトリの JSON 構造 (config_json)
`file_ops` の `config_json` は以下のような構造を持ちます。
```json
{
  "directories": [
    {
      "path": "D:/project",
      "allow_read": true,
      "allow_write": true
    }
  ]
}
```

## 2. file_ops プラグインの機能拡張

### 2.1 アクセス権限 (ACL) によるサンドボックスの高度化
- **パスの検証 (`validate_path`)**: `config_json` に登録された **`directories` リストのいずれかに内包されているか** の検証へ変更します。

### 2.2 新規ツール: ディレクトリ権限の要求
- 新規ツール: `request_directory_access`
  - パラメータ: `path` (String), `requires_write` (Boolean)
  - 処理: AIが「このディレクトリへのアクセス権が欲しい」とリクエストするツール。ユーザーに許可を求め、許可されれば追加する。

### 2.3 画像読み込み (Vision API) 対応
- 新規ツール: `read_image`
  - パラメータ: `path` (String)
  - 処理: 画像ファイルをBase64エンコードし `[IMAGE_BASE64]:<data>` を返す。`ChatEngine` で抽出しメッセージへ添付する。

## 3. UI の高度化とテーマ対応

### 3.1 右ペインのツールごとの折り畳み (アコーディオン) 管理
- **課題**: 現在の `ToolManagementPane.tsx` では、プラグインのリストがフラットに表示され、かつ `file_ops` 専用のディレクトリ管理画面が最下部に固定表示されているため拡張性が低い。
- **設計**: 
  - 各プラグイン（例: `file_ops`, `calculator`, `web_search`）を「折り畳み可能なセクション（アコーディオン）」としてレンダリングする。
  - セクションを開くと、そのプラグインが持つツール一覧の有効/無効トグルと、**そのプラグイン固有の設定コンポーネント**（例: `FileOpsDirectoryManager`）が表示されるようにする。

### 3.2 右ペインのリサイズ対応
- メインチャットエリアと `ToolManagementPane` の境界にリサイズ用のハンドル（`<div onMouseDown={...}>`）を配置し、`paneWidth` の状態に応じて幅を動的に変更する。

### 3.3 テーマ (ライト / ダーク) の対応とスクロールバー
- **スクロールバースタイル**: `tailwind.config.js` または `globals.css` で、ダークモード (`.dark`) クラスが当たっている場合のスクロールバーのスタイル（`::-webkit-scrollbar` 関連の色）を適切に定義する。
- **テーマ切り替えボタン**: ヘッダーまたはサイドバー（設定画面など適切な場所）に、ライトモードとダークモードをトグルするボタン（Sun / Moon アイコン）を配置する。現在のテーマは localStorage 等で永続化するか、既存の UI store (Zustandなど) の `theme` ステートを活用して `document.documentElement.classList.toggle('dark')` を切り替える設計とする。
