## 1. 設定管理 (Models & Store)
- `src-tauri/src/models/config.rs` の `AppConfig` に新しく `web_search` フィールドを追加するか、既存の `PluginConfig` の仕組みを利用して保存する。
- 構造体の設計例：
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub enum SearchProvider {
      Tavily,
      Brave,
  }

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct WebSearchConfig {
      pub provider: SearchProvider,
      pub api_key: Option<String>,
      pub allowed_domains: Vec<String>, // fetch_page 用ホワイトリスト
  }
  ```
- フロントエンドでは `SettingsView` などにプロバイダ切り替えのドロップダウン、APIキーの入力フィールド、許可ドメインを改行区切りで入力できるテキストエリアを追加する。
- **UI要件:** プラグインごとの設定項目が多くなることを防ぐため、各プラグインの設定セクションは折り畳み可能（アコーディオン形式）な UI コンポーネントとして実装する。

## 2. ツール定義と動作基盤
- `web_search` プラグインは以下の2つのツールを提供する。
  1. `search`: クエリでWeb検索を行う。
  2. `fetch_page`: 指定されたURLのWebページ本文を取得する。
- APIキーが未設定（`None`）の場合は、例外を投げるのではなく、正常系(`is_error: false`) として `{"note": "Web検索を利用するには設定画面からAPIキーを設定してください"}` のようなJSONまたはテキストを返し、LLMがユーザーに案内できるようにする。

## 3. Web Fetch (fetch_page) の実装詳細
- **パラメータ:** `{ "url": "https://..." }`
- **バリデーション:** リクエストされた URL のホスト名 (ドメイン) が `allowed_domains` リストに含まれているか（またはサブドメインとしてマッチするか）を検証する。マッチしなければ `{"error": "Access to this domain is restricted by the user's whitelist."}` を返す。
- **データ取得:** `reqwest` の GET リクエストを使用し、HTMLを取得。
- **テキスト抽出:** `scraper` または正規表現による簡易なタグ除去を用い、`<script>` や `<style>` を除外したプレーンテキスト（本文）を抽出して返す。トークン数制限を考慮し、一定文字数で切り詰める。

## 4. Tavily API の実装詳細
- **API エンドポイント:** `POST https://api.tavily.com/search`
- **リクエストボディ:**
  ```json
  {
    "api_key": "{api_key}",
    "query": "{query}",
    "search_depth": "basic",
    "include_answer": false,
    "max_results": 5
  }
  ```
- レスポンスの `results` 配列内の `title`, `url`, `content` を抽出し、LLM に返す。

## 5. Brave Search API の実装詳細
- **API エンドポイント:** `GET https://api.search.brave.com/res/v1/web/search`
- **ヘッダー:**
  - `Accept`: `application/json`
  - `X-Subscription-Token`: `{api_key}`
- **クエリパラメータ:**
  - `q`: `{query}`
- レスポンスの `web.results` 配列内の `title`, `url`, `description` を抽出し、LLM に返す。
