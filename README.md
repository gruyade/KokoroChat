# AI Character Chat

ローカルLLMを活用したAIキャラクターチャットデスクトップアプリケーション。

ユーザーはAIキャラクターを作成し、個性的な会話を楽しめる。キャラクターは自発的に発話し、独自に思考し、会話の記憶を蓄積する。TTS連携による音声出力にも対応。

## 機能

- **キャラクター作成** — 名前と概要からLLMがSystem Promptを自動生成。手動編集も可能
- **チャット** — ストリーミング応答、複数セッション管理、エラー時の再送信
- **自発的発話** — キャラクターが設定間隔で自発的に話しかける
- **独自思考** — チャットとは独立したキャラクターの内部思考を閲覧可能
- **記憶管理** — 会話内容をLLMで要約・圧縮し長期記憶として保持
- **TTS連携** — IrodoriTTS / VoicePeak対応の音声出力
- **ファイル添付** — テキスト、PDF、画像ファイルをチャットに添付
- **プラグイン** — Tool Use（Function Calling）によるファイル操作、Web検索、計算
- **用途別モデル設定** — 会話、記憶整理、思考、キャラクター生成で異なるLLMを指定可能
- **ダークモード** — ライト/ダークテーマ切り替え対応

## 必要環境

| ツール | バージョン |
|--------|-----------|
| Rust | 1.75+ |
| Node.js | 20+ |
| pnpm | 9+ |

### Windows追加要件

- Visual Studio Build Tools（C++ビルドツール）
- WebView2（Windows 10/11は標準搭載）

### Linux追加要件

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf \
  libssl-dev \
  libgtk-3-dev
```

## セットアップ

```bash
# 1. リポジトリをクローン
git clone https://github.com/your-username/ai-character-chat.git
cd ai-character-chat

# 2. フロントエンド依存関係インストール
pnpm install

# 3. 環境変数設定
cp .env.example .env
# .env を編集し、LLMのエンドポイントとAPIキーを設定

# 4. 開発サーバー起動
cargo tauri dev

# 5. プロダクションビルド
cargo tauri build
```

## 環境変数

`.env.example` を `.env` にコピーして設定する。

| 変数名 | 説明 |
|--------|------|
| `AI_CHAT_LLM_BASE_URL` | 会話用LLMのベースURL |
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
| `AI_CHAT_TTS_IRODORI_BASE_URL` | IrodoriTTSのベースURL |
| `AI_CHAT_TTS_VOICEPEAK_BASE_URL` | VoicePeakのベースURL |

## 開発コマンド

```bash
# 開発サーバー（ホットリロード対応）
cargo tauri dev

# プロダクションビルド
cargo tauri build

# フロントエンドテスト
pnpm test

# バックエンドテスト
cd src-tauri && cargo test --lib

# リント
pnpm lint

# 型チェック
pnpm type-check

# Rustフォーマットチェック
cd src-tauri && cargo fmt --check

# Rust静的解析
cd src-tauri && cargo clippy
```

## アーキテクチャ

```
ai-character-chat/
├── src/                    # フロントエンド（React + TypeScript）
│   ├── components/         # UIコンポーネント
│   ├── stores/             # Zustand状態管理
│   ├── hooks/              # カスタムHooks
│   └── types/              # 型定義
├── src-tauri/              # バックエンド（Rust + Tauri v2）
│   └── src/
│       ├── llm/            # LLMクライアント（OpenAI互換API）
│       ├── character/      # キャラクター作成・管理
│       ├── chat/           # チャットエンジン
│       ├── spontaneous/    # 自発的発話
│       ├── thought/        # 独自思考
│       ├── memory/         # 記憶管理
│       ├── tts/            # TTS連携
│       ├── attachment/     # ファイル添付処理
│       ├── plugin/         # プラグインシステム
│       ├── config/         # モデル設定管理
│       ├── db/             # SQLiteデータベース
│       ├── models/         # データモデル
│       └── commands/       # Tauriコマンド
└── .github/workflows/      # CI/CD
```

- **フロントエンド**: React 19 + TypeScript + Vite + Tailwind CSS + Zustand
- **バックエンド**: Rust + Tauri v2 + SQLite（rusqlite）
- **テスト**: Vitest + fast-check（フロントエンド）、proptest（バックエンド）
- **LLM通信**: OpenAI互換APIフォーマット（reqwest + tokio）

## ライセンス

MIT
