# Bugfix Requirements Document

## Introduction

設定保存の永続化失敗とモデル変更時の二重呼び出しに関するバグ修正。2つの問題が存在する：

1. UIで保存した設定がアプリ再起動時に環境変数の値で上書きされ、永続化されない
2. モデル名を変更して保存した後、チャット送信時に変更前のモデルが呼ばれてVRAM不足でクラッシュする

## Bug Analysis

### Root Cause

**問題1: 設定永続化失敗**

`dotenvy::dotenv()` はカレントディレクトリから親ディレクトリを再帰的に探索して `.env` を見つける。ビルド済みexeを `D:\repos\ChatGame\src-tauri\target\release\` から実行すると、親を遡って `D:\repos\ChatGame\.env` が発見・読み込まれる。

その後 `ModelConfigManager::load_or_default` 内の `apply_env_fallback` が、環境変数の値で `config.json` の保存値を**常に上書き**する。結果、UIで保存した設定が起動時に `.env` の値に戻る。

**問題2: モデル二重呼び出し**

問題1と同根。再起動なしでも発生するメカニズム：
- `set_config` でメモリ上の設定は正しく更新される
- しかしバックグラウンドタスク（記憶圧縮・思考エンジン）がチャット送信中/直後に発火し、LM Studioに別のリクエストを送信する
- 記憶圧縮は `send_message` 完了直後に `tokio::spawn` で即座に実行される
- 思考エンジンは `interval_minutes` 間隔でループ実行される
- これらのリクエストが異なるモデル名を使用した場合（設定変更のタイミング問題、または環境変数上書きの影響）、LM Studioが複数の大型モデルを同時ロードしようとしてVRAM不足で落ちる

### Current Behavior (Defect)

1.1 WHEN ユーザーがUIでモデル設定を変更して保存し、アプリを再起動する THEN the system は`load_or_default`内の`apply_env_fallback`が環境変数の値で常にconfig.jsonの保存値を上書きし、設定が元に戻る

1.2 WHEN ユーザーがチャットを送信し、メッセージ数が圧縮閾値を超えている THEN the system はチャット応答完了直後に`tokio::spawn`でMemory用モデルによる記憶圧縮リクエストを即座に送信し、Chat用ストリーミングと並行してLM Studioにリクエストが飛ぶ

1.3 WHEN 思考エンジンが有効でチャット送信中にinterval_minutesが経過する THEN the system はThought用モデルでLLMリクエストを送信し、Chat用ストリーミングと並行してLM Studioにリクエストが飛ぶ

### Expected Behavior (Correct)

2.1 WHEN ユーザーがUIでモデル設定を変更して保存し、アプリを再起動する THEN the system SHALL config.jsonに保存された値を優先し、環境変数はconfig.jsonに値が未設定（空文字列）の場合のみフォールバックとして適用する

2.2 WHEN ユーザーがチャットを送信し、メッセージ数が圧縮閾値を超えている THEN the system SHALL チャット応答のストリーミングが完全に完了した後に記憶圧縮リクエストを送信し、LLMリクエストが同時に複数送信されないようにする

2.3 WHEN 思考エンジンが有効でチャット送信中にinterval_minutesが経過する THEN the system SHALL チャット応答のストリーミングが完全に完了するまで思考生成リクエストの送信を待機し、LLMリクエストが同時に複数送信されないようにする

### Unchanged Behavior (Regression Prevention)

3.1 WHEN config.jsonが存在せず環境変数にモデル設定が定義されている THEN the system SHALL CONTINUE TO 環境変数の値をデフォルト設定として使用する

3.2 WHEN config.jsonのモデル設定が空文字列で環境変数に値が設定されている THEN the system SHALL CONTINUE TO 環境変数の値をフォールバックとして適用する

3.3 WHEN チャット送信が完了しストリーミングが終了した後 THEN the system SHALL CONTINUE TO 記憶圧縮チェックを実行する

3.4 WHEN 思考エンジンが有効でLLMリクエストが競合しない THEN the system SHALL CONTINUE TO interval_minutes間隔で思考を生成する

3.5 WHEN ユーザーがUIで設定を保存する THEN the system SHALL CONTINUE TO config.jsonにJSON形式で設定を永続化する
