# カスタムツール及びUI拡張 仕様書

## 1. カスタムツールの定義・登録手法

ユーザーが動的に追加できるカスタムツール（プラグイン）は、以下の2つの実行方式をサポートします。

### A. HTTP Webhook 方式
外部の REST API にリクエストを送信して結果を受け取ります。
- **定義フォーマット (JSON):**
  ```json
  {
    "type": "http",
    "name": "get_weather",
    "description": "指定した地域の現在の天気を取得します",
    "parameters": {
      "type": "object",
      "properties": {
        "location": { "type": "string" }
      },
      "required": ["location"]
    },
    "config": {
      "url": "https://api.example.com/weather",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer {API_KEY}"
      }
    }
  }
  ```

### B. ローカルスクリプト方式 (CLI)
PC上の実行ファイルやスクリプト（Python, Node.js など）を実行し、標準出力を結果として受け取ります。
- **定義フォーマット (JSON):**
  ```json
  {
    "type": "cli",
    "name": "read_local_file",
    "description": "ローカルのファイルを読み込みます",
    "parameters": {
      "type": "object",
      "properties": {
        "path": { "type": "string" }
      },
      "required": ["path"]
    },
    "config": {
      "command": "python",
      "args": ["/path/to/script.py", "--path", "{{path}}"]
    }
  }
  ```

これらのカスタムツールはJSON形式でエクスポート/インポート可能とし、内部的には `PluginRegistry` がこれらをラップした `PluginHandler` インスタンスとして動的に生成・登録します。

---

## 2. カスタムUIコンポーネントのレンダリング仕様

ツールが実行された結果として、単なるプレーンテキストだけでなく、チャット画面上にリッチなUI（地図、グラフ、特別なウィジェット）を描画するための仕様です。

### 2.1. ツールからの応答フォーマット
ツールは処理結果の文字列として、以下の特定のXML風タグを含めることができます。

```xml
<ChatWidget type="widget_name" data='{"key": "value"}' />
```
- `type`: 描画すべきフロントエンドのコンポーネントの種類（例: `map`, `chart`, `image_grid`）。
- `data`: コンポーネントに渡されるプロパティ（JSON文字列）。シングルクォートで囲み、内部は有効なJSONにするか、Base64エンコードして渡します。

### 2.2. フロントエンドでの解釈と描画
- フロントエンドの `MarkdownRenderer`（または専用のパーサー層）で、正規表現などを使い `<ChatWidget ... />` を検知します。
- 検知された場合、事前に登録された React コンポーネント（例: `WidgetRegistry`）から `type` に対応するコンポーネントを探し出し、インラインで描画します。
- 未知の `type` の場合は、`data` をフォーマットされたJSONテキストとしてフォールバック表示します。

### 2.3. インタラクティブな動作
描画されたカスタムUI（例：ボタン）がクリックされた際に再度LLMをトリガーしたい場合は、ウィジェットから親の `ChatView` に対して `onSendMessage` コールバックを呼び出すことで、自然にチャットのフローに組み込むことが可能です。
