# Web Search Plugin Implementation Spec

`web_search` 組み込みプラグインは、以下の2つのプロバイダをサポートする形で実装します。
ユーザーが設定（グローバルコンフィグ等）でどちらのAPIを使用するか、またそのAPIキーを設定できるようにします。

## 1. 共通設定と仕様
- **設定データ構造:**
  ```rust
  pub enum SearchProvider {
      Tavily,
      Google,
  }

  pub struct WebSearchConfig {
      pub provider: SearchProvider,
      pub api_key: String,
      pub search_engine_id: Option<String>, // Google Custom Search用 (cx)
  }
  ```
- **ツールの挙動:**
  - `web_search` プラグインは初期状態では「設定未完了」を示すエラーを返すか、設定画面への誘導を LLM に返す。
  - 有効なAPIキーが設定されていれば、指定されたプロバイダの API に HTTP リクエストを送信し、結果を整形して JSON 文字列として返す。

---

## 2. Tavily Search API 実装 (推奨・デフォルト想定)
LLM エージェント向けの検索 API。クリーンなテキストを抽出して返すため、LLM との相性が非常に良い。

- **API エンドポイント:** `POST https://api.tavily.com/search`
- **リクエストボディ:**
  ```json
  {
    "api_key": "tvly-...",
    "query": "{query}",
    "search_depth": "basic",
    "include_answer": false,
    "max_results": 5
  }
  ```
- **レスポンス処理:**
  - `results` 配列内の `title`, `url`, `content` を抽出し、LLM が読みやすい形式（JSON配列または Markdown 箇条書き）に整形して返す。

---

## 3. Google Custom Search API 実装
最も標準的で安定した検索エンジン。

- **API エンドポイント:** `GET https://www.googleapis.com/customsearch/v1`
- **クエリパラメータ:**
  - `key`: `{api_key}`
  - `cx`: `{search_engine_id}`
  - `q`: `{query}`
- **レスポンス処理:**
  - `items` 配列内の `title`, `link`, `snippet` を抽出する。
  - Google の snippet は短い概要文のため、Tavily ほどの情報量はないが、概要を把握するには十分。JSON 配列などに整形して返す。
