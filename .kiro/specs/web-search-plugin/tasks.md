# Web Search Plugin Implementation Tasks

## Phase 1: Configuration & UI
- [x] バックエンドの設定管理実装
  - `src-tauri/src/models/config.rs` または `src-tauri/src/plugin/registry.rs` の設定保存機構を利用し、`WebSearchConfig` (プロバイダ、APIキー等) を保存・取得できるようにする。
  - ホワイトリスト用の `allowed_domains: Vec<String>` を追加する。
- [x] フロントエンド設定画面実装
  - Settings画面等のプラグイン管理タブ内に、各プラグインの設定セクションを「折り畳み可能（アコーディオン形式）」なUIコンポーネントとして実装する。
  - Web検索プロバイダ（Tavily/Brave）の選択用ドロップダウンを追加。
  - 選択されたプロバイダに応じた API Key 入力フィールドを追加。
  - `fetch_page` ツール用の「許可ドメイン（ホワイトリスト）」を入力するテキストエリアを追加する。

## Phase 2: Web Search API Integration
- [x] `src-tauri/src/plugin/builtin/web_search.rs` のリファクタリング
  - スタブ実装となっている `execute` メソッドを改修し、設定から `WebSearchConfig` を読み込む処理を追加。
  - APIキーが未設定の場合は、エラー結果ではなく「ユーザーにAPIキーの設定を促す文章」をJSON等で返し、LLMが自然に回答できるようにする。
- [x] Tavily API リクエスト実装 (`search` ツール)
  - `reqwest` を用いて `https://api.tavily.com/search` にPOSTリクエストを送信する。
  - JSONレスポンスをパースし、`title`, `url`, `content` を抽出して整形する。
- [x] Brave Search API リクエスト実装 (`search` ツール)
  - `reqwest` を用いて `https://api.search.brave.com/res/v1/web/search` にGETリクエストを送信する。
  - JSONレスポンスをパースし、`web.results` 内の `title`, `url`, `description` を抽出して整形する。

## Phase 3: Web Fetch Tool Integration
- [x] `fetch_page` ツールの追加
  - `web_search.rs` 内で提供するツール定義 (`tools` メソッド) に `fetch_page` を追加する。
  - パラメータは `{"url": "https://..."}` を受け取る。
- [x] ドメイン・ホワイトリスト検証の実装
  - 引数の URL をパースし、ホスト名が `WebSearchConfig.allowed_domains` に含まれるか検証する。
  - 不一致の場合はエラー文字列 ("Access to this domain is restricted...") を返す。
- [x] ページ本文の取得と抽出実装
  - `reqwest` の GET リクエストでHTMLを取得する。
  - `scraper` (または `regex`) クレートを用いてHTMLタグを取り除き、テキスト本文を抽出・整形して LLM に返す。

## Phase 4: Testing & Validation
- [x] バックエンドの通信とパースのテスト
  - Tavily / Brave API への実際のリクエスト、もしくはモックサーバーを用いたレスポンスパースのユニットテストを追加する。
  - `fetch_page` のホワイトリスト検証が正しく機能するかテストする。
- [x] E2E 動作確認
  - アプリケーションを起動し、チャットで「今日の東京の天気を調べて」などのプロンプトを入力。
  - 検索後、必要に応じてさらに `fetch_page` が呼ばれる一連の流れを確認する。