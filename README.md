# KokoroChat

ローカルLLMおよび大手クラウドLLMを活用した、AIキャラクターと深く交流するためのデスクトップアプリケーション。

ユーザーは独自のAIキャラクターを作成し、個性的な会話を楽しめます。キャラクターは自発的に発話し、独自に思考し、会話の記憶を蓄積します。最新のTTS（音声合成）連携により、キャラクターの声で会話することも可能です。

## 機能

- **キャラクター作成** — 名前と概要からLLMがSystem Promptを自動生成。手動での詳細編集も可能。
- **マルチLLMプロバイダー対応** — OpenAI, Anthropic (Messages API), Google Gemini (Generative Language API), およびOpenAI互換APIに対応。
- **チャット** — ストリーミング応答、複数セッション管理、メッセージ編集・再送信機能。システムメッセージのバッジ表示対応。ツール呼び出し前後でバブルを自動分割し会話フローを明確化。
- **自発的発話** — キャラクターが設定された間隔と確率で自発的に話しかけます。
- **独自思考** — チャットとは独立したキャラクターの内部思考（思索）プロセスを閲覧可能。
- **記憶管理** — 会話内容を要約・圧縮し、長期記憶として保持。
- **高度なTTS連携**
    - **VoicePeak** — 公式CLIを直接呼び出す高品質な音声出力。VoicePeak側の仕様により長文の生成には時間がかかります。
    - **IrodoriTTS** — サーバー連携による音声出力。
    - キャラクターごとに個別のボイス設定が可能。
- **ファイル添付** — テキスト、Markdown、CSV、PDF、画像（PNG, JPG, WebP）をチャットに添付し、内容について会話。
- **プラグイン（Tool Use）** — 計算、ファイル操作、Web検索、ナレッジ参照などの機能をLLMがツールとして実行可能。
- **ナレッジ（Knowledge）** — セッション単位でテキストファイルを登録し、キャラクターが参照しながら会話。tool_referenceモードでツール使用も誘導。
- **Thinking/Reasoning表示** — LLMの思考プロセス（Gemini, Claude, OpenAI o系モデル対応）をUI上で折りたたみ表示。
- **用途別モデル設定** — 会話、記憶整理、思考、キャラクター生成の各タスクに最適なモデルを個別に指定可能。
- **モダンなUI** — React 19 + Tailwind CSSによる洗練されたインターフェース。ダーク/ライトモード対応。

## 必要環境

| ツール | バージョン |
|--------|-----------|
| Rust | 1.75+ |
| Node.js | 20+ |
| pnpm | 9+ |
| tauri-cli | 2.0+ (`cargo install tauri-cli`) |

### Windows追加要件
- Visual Studio Build Tools（C++ビルドツール）
- WebView2

### Linux追加要件
```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libssl-dev libgtk-3-dev
```

## セットアップ

```bash
# 1. リポジトリをクローン
git clone https://github.com/your-username/kokoro-chat.git
cd kokoro-chat

# 2. フロントエンド依存関係インストール
pnpm install

# 3. 環境変数設定
cp .env.example .env
# .env を編集し、LLMのAPIキーやベースURLを設定（詳細は後述）

# 4. 開発サーバー起動
cargo tauri dev

# 5. プロダクションビルド
cargo tauri build
```

## 環境変数 (`.env`)

設定画面（UI）で直接設定することも可能ですが、`.env` ファイルでデフォルト値を指定できます。

| 変数名 | 説明 |
|--------|------|
| `AI_CHAT_LLM_BASE_URL` | 会話用LLMのベースURL（OpenAI互換の場合） |
| `AI_CHAT_LLM_API_KEY` | 会話用LLMのAPIキー |
| `AI_CHAT_LLM_MODEL` | 会話用モデル名 |
| `AI_CHAT_MEMORY_LLM_BASE_URL` | 記憶整理用LLMのベースURL |
| `AI_CHAT_MEMORY_LLM_API_KEY` | 記憶整理用LLMのAPIキー |
| `AI_CHAT_MEMORY_LLM_MODEL` | 記憶整理用モデル名 |
| `AI_CHAT_THOUGHT_LLM_BASE_URL` | 思考用LLMのベースURL |
| `AI_CHAT_THOUGHT_LLM_API_KEY` | 思考用LLMのAPIキー |
| `AI_CHAT_THOUGHT_LLM_MODEL` | 思考用モデル名 |
| `AI_CHAT_CHARGEN_LLM_BASE_URL` | キャラクター生成用LLMのベースURL |
| `AI_CHAT_CHARGEN_LLM_API_KEY` | キャラクター生成用LLMのAPIキー |
| `AI_CHAT_CHARGEN_LLM_MODEL` | キャラクター生成用モデル名 |

※ TTS（VoicePeak/IrodoriTTS）の設定は、アプリケーション内のキャラクター設定または全体設定画面から行います。

## アーキテクチャ

```
kokoro-chat/
├── src/                    # フロントエンド（React 19 + TypeScript）
│   ├── components/         # UIコンポーネント
│   ├── stores/             # Zustand状態管理
│   ├── hooks/              # カスタムHooks
│   └── types/              # 型定義
├── src-tauri/              # バックエンド（Rust + Tauri v2）
│   └── src/
│       ├── llm/            # LLMクライアント（OpenAI / Anthropic / Google対応）
│       ├── chat/           # チャットエンジン
│       ├── character/      # キャラクター管理
│       ├── spontaneous/    # 自発的発話ロジック
│       ├── thought/        # 思考生成
│       ├── memory/         # 記憶管理（SQLite + LLM要約）
│       ├── tts/            # TTS連携（VoicePeak CLI / IrodoriTTS）
│       ├── attachment/     # ファイル解析（PDF, 画像等）
│       ├── plugin/         # プラグインシステム（Tool Use: calculator, file_ops, web_search, knowledge）
│       ├── config/         # アプリケーション設定
│       ├── db/             # SQLiteマイグレーション・リポジトリ
│       └── commands/       # Tauri Commandハンドラ
└── .github/workflows/      # CI（ビルド・テスト）
```

- **フロントエンド**: React 19, Vite, Tailwind CSS, Zustand, Lucide React, React Markdown
- **バックエンド**: Rust, Tauri v2, SQLite (rusqlite), tokio, reqwest
- **テスト**: Vitest + fast-check (Frontend), proptest (Backend)

## 開発ツール

本プロジェクトは [Kiro](https://kiro.dev/) および [Roo Code](https://roocode.com/) を使用して開発されている。

## 免責事項

このプロジェクトはAIツールの支援を受けて、個人の趣味として開発したものです。

- 通常の使い方では正常に動作しますが、**エッジケースについては網羅していません**。
- 本ソフトウェアの利用によって生じた損害・損失について、**いかなる保証もいたしません**。
- ご利用は自己責任でお願いします。

## ライセンス

MIT
