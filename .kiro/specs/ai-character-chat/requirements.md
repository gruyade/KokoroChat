# Requirements Document

## Introduction

ローカルLLMを活用したAIキャラクターチャットデスクトップアプリケーション。ユーザーはAIキャラクターを作成し、個性的な会話を楽しめる。キャラクターは自発的に発話し、独自に思考し、会話の記憶を蓄積する。TTS連携による音声出力にも対応。GitHub公開前提のオープンソースデスクトップアプリとして新規開発する。

## Glossary

- **App**: 本AIキャラクターチャットデスクトップアプリケーション全体（Electron/Tauri等で構築）
- **Character_Creator**: キャラクター作成機能を担うモジュール
- **Chat_Engine**: チャット処理を担うモジュール（メッセージ送受信、履歴管理）
- **Spontaneous_Speaker**: キャラクターの自発的発話を制御するモジュール
- **Thought_Engine**: キャラクターの独自思考を管理するモジュール
- **Memory_Manager**: 会話記憶の整理・圧縮・保存を担うモジュール
- **TTS_Connector**: TTS APIとの連携を担うモジュール
- **LLM_Client**: LLM APIとの通信を担うモジュール
- **Model_Config**: 用途別のモデル設定を管理するモジュール
- **Character**: システムプロンプト（性格、バックグラウンド、口調）を持つAIキャラクター
- **Chat**: ユーザーとキャラクター間の一連の会話セッション
- **Memory**: 会話内容をLLMで整理・圧縮した長期記憶データ
- **Thought**: チャットとは独立したキャラクターの内部思考
- **System_Prompt**: キャラクターの性格・口調・背景を定義するプロンプト
- **File_Attachment**: ユーザーがチャットに添付するファイル（テキスト、PDF、画像等）
- **Attachment_Processor**: 添付ファイルの読み込み・内容抽出を担うモジュール
- **Plugin_System**: Tool Use機能を管理し、プラグインの登録・実行を担うモジュール
- **Plugin**: Plugin_Systemに登録される拡張機能の単位。ツール定義とハンドラを持つ
- **Tool**: LLMのFunction Calling経由でキャラクターが呼び出せる外部機能の定義
- **Plugin_Registry**: 利用可能なPluginを管理し、有効/無効の切り替えを行うモジュール

## Requirements

### Requirement 1: キャラクター作成

**User Story:** As a ユーザー, I want AIキャラクターを作成して性格や口調を設定したい, so that 個性的なキャラクターと会話できる。

#### Acceptance Criteria

1. WHEN ユーザーがキャラクター名と概要説明を入力した時, THE Character_Creator SHALL LLMを使用してSystem_Promptを自動生成する
2. WHEN System_Promptが生成された時, THE Character_Creator SHALL 性格、バックグラウンド、口調を含むCharacter設定ファイルをJSON形式で保存する
3. WHEN ユーザーが生成されたSystem_Promptを確認した時, THE Character_Creator SHALL ユーザーによる手動編集を許可する
4. THE Character_Creator SHALL キャラクター一覧画面にすべての作成済みCharacterを表示する
5. WHEN ユーザーが既存Characterを選択した時, THE Character_Creator SHALL Character設定の編集画面を表示する
6. WHEN ユーザーがCharacterを削除した時, THE Character_Creator SHALL 関連するすべてのChat履歴とMemoryも削除する

### Requirement 2: チャット機能

**User Story:** As a ユーザー, I want キャラクターを選択して会話したい, so that 複数の会話を管理しながらAIキャラクターとやり取りできる。

#### Acceptance Criteria

1. WHEN ユーザーがCharacterを選択した時, THE Chat_Engine SHALL 新規Chatセッションを作成する
2. WHEN ユーザーがメッセージを送信した時, THE Chat_Engine SHALL LLM APIにSystem_Promptとチャット履歴を含めてリクエストを送信し、キャラクターの応答を返す
3. THE Chat_Engine SHALL すべてのChatセッションをリスト形式で管理し、ユーザーが過去のChatを選択して再開できるようにする
4. WHEN ユーザーがChat一覧を表示した時, THE Chat_Engine SHALL 各Chatの最終メッセージ日時とプレビューを表示する
5. WHEN LLM APIからストリーミングレスポンスを受信した時, THE Chat_Engine SHALL リアルタイムにメッセージを画面に表示する
6. IF LLM APIへのリクエストが失敗した時, THEN THE Chat_Engine SHALL エラーメッセージを表示し、再送信ボタンを提供する

### Requirement 3: 自発的発話

**User Story:** As a ユーザー, I want キャラクターから自発的に話しかけてほしい, so that より自然で生き生きとした会話体験を得られる。

#### Acceptance Criteria

1. WHILE Chatセッションがアクティブな状態で, THE Spontaneous_Speaker SHALL 設定された間隔でキャラクターの自発的発話を評価する
2. WHEN 自発的発話の条件が満たされた時, THE Spontaneous_Speaker SHALL キャラクターのSystem_Promptと直近の会話コンテキストに基づいてメッセージを生成する
3. THE Spontaneous_Speaker SHALL 自発的発話の有効/無効をユーザーが切り替えられるトグルを提供する
4. THE Spontaneous_Speaker SHALL 自発的発話の最小間隔（秒単位）をユーザーが設定できるインターフェースを提供する
5. WHEN 自発的発話が生成された時, THE Spontaneous_Speaker SHALL 通常のキャラクター応答と視覚的に区別して表示する

### Requirement 4: 独自思考

**User Story:** As a ユーザー, I want キャラクターが独自に思考する様子を見たい, so that キャラクターの内面をより深く理解できる。

#### Acceptance Criteria

1. THE Thought_Engine SHALL チャット会話とは独立した思考プロセスを管理する
2. WHEN キャラクターの思考が生成された時, THE Thought_Engine SHALL Thought履歴をChat履歴とは別のストレージに保存する
3. THE Thought_Engine SHALL ユーザーがキャラクターのThought履歴を閲覧できる専用画面を提供する
4. WHEN 思考を生成する時, THE Thought_Engine SHALL 直近の会話コンテキストとMemoryを参照して思考内容を生成する
5. THE Thought_Engine SHALL 思考生成の頻度をユーザーが設定できるインターフェースを提供する
6. WHILE 思考生成が実行中の状態で, THE Thought_Engine SHALL 思考中であることを示すインジケーターを表示する

### Requirement 5: 記憶管理

**User Story:** As a ユーザー, I want キャラクターが過去の会話を記憶してほしい, so that 長期的に一貫性のある会話ができる。

#### Acceptance Criteria

1. WHEN 会話メッセージ数が設定された閾値に達した時, THE Memory_Manager SHALL LLMを使用して会話内容を要約・圧縮しMemoryとして保存する
2. THE Memory_Manager SHALL Memoryをコンテキストウィンドウとは独立したストレージに永続化する
3. WHEN Chat_EngineがLLMにリクエストを送信する時, THE Memory_Manager SHALL 関連するMemoryをプロンプトに含める
4. THE Memory_Manager SHALL ユーザーがMemory一覧を閲覧・編集・削除できる管理画面を提供する
5. WHEN Memoryが更新された時, THE Memory_Manager SHALL 更新日時とソースとなったChat情報を記録する
6. THE Memory_Manager SHALL Memory圧縮に使用するLLMモデルをModel_Configから取得する

### Requirement 6: TTS連携

**User Story:** As a ユーザー, I want キャラクターの発話を音声で聞きたい, so that より没入感のある会話体験を得られる。

#### Acceptance Criteria

1. WHEN キャラクターがメッセージを生成した時, THE TTS_Connector SHALL 設定されたTTS APIにテキストを送信し音声データを取得する
2. THE TTS_Connector SHALL IrodoriTTSおよびVoicePeakのAPIに対応する
3. THE TTS_Connector SHALL キャラクターごとにTTS設定（使用API、話者、参照音声ファイルパス）を保存する
4. WHEN TTS APIにリクエストを送信する時, THE TTS_Connector SHALL キャプション指示（感情、速度等）をパラメータとして含める
5. THE TTS_Connector SHALL TTS機能の有効/無効をユーザーが切り替えられるトグルを提供する
6. IF TTS APIへのリクエストが失敗した時, THEN THE TTS_Connector SHALL エラーを表示しテキストのみで会話を継続する
7. WHEN 音声データを受信した時, THE TTS_Connector SHALL 自動再生し、再生中であることを視覚的に示す

### Requirement 7: API連携・モデル設定

**User Story:** As a ユーザー, I want 用途ごとに異なるLLMモデルやAPIエンドポイントを設定したい, so that 最適なモデルを使い分けられる。

#### Acceptance Criteria

1. THE Model_Config SHALL 以下の用途ごとに個別のモデル設定を管理する: 会話用、記憶整理用、思考用、キャラクター生成用
2. THE Model_Config SHALL 各用途に対してベースURL、モデル名、APIキー、温度パラメータを設定できるインターフェースを提供する
3. THE LLM_Client SHALL OpenAI互換APIフォーマットでリクエストを送信する
4. WHEN APIキーが設定画面に表示される時, THE Model_Config SHALL マスク表示する
5. THE Model_Config SHALL 設定値をローカルの設定ファイルに永続化する
6. IF LLM APIへの接続テストが失敗した時, THEN THE Model_Config SHALL 接続エラーの詳細を表示する
7. THE Model_Config SHALL 設定画面から各用途のAPI接続テストを実行できるボタンを提供する

### Requirement 8: セキュリティとGitHub公開対応

**User Story:** As a 開発者, I want セキュリティを確保しつつGitHubで公開したい, so that 安全にオープンソースとして配布できる。

#### Acceptance Criteria

1. THE App SHALL APIキー、設定ファイル等の機密情報を.gitignoreに含めてリポジトリから除外する
2. THE App SHALL 環境変数または.envファイルから機密設定を読み込む仕組みを提供する
3. THE App SHALL GitHub Actionsによるリント・テスト・ビルドのCIパイプラインを含む
4. THE App SHALL READMEにセットアップ手順、必要な環境変数一覧、使用方法を記載する
5. THE App SHALL .env.exampleファイルで必要な環境変数のテンプレートを提供する
6. THE App SHALL ソースコード内にAPIキーやシークレットをハードコードせず、実行時にユーザーが設定ファイルまたは環境変数から提供する方式を採用する
7. THE App SHALL git-secretsまたはpre-commitフックにより、コミット時に機密情報の混入を検出する仕組みを提供する

### Requirement 9: モダンUI

**User Story:** As a ユーザー, I want モダンで使いやすいデスクトップチャットUIを使いたい, so that 快適にキャラクターと会話できる。

#### Acceptance Criteria

1. THE App SHALL デスクトップアプリケーションとしてウィンドウのリサイズに対応し、最小ウィンドウサイズを設定する
2. THE App SHALL ダークモードとライトモードの切り替えに対応する
3. THE App SHALL チャット画面にメッセージ入力欄、送信ボタン、会話履歴表示エリアを配置する
4. THE App SHALL サイドバーにChat一覧、Character一覧、設定へのナビゲーションを配置する
5. WHEN 新しいメッセージが追加された時, THE App SHALL チャット表示エリアを自動スクロールして最新メッセージを表示する
6. THE App SHALL キャラクターのアバター画像設定に対応する
7. WHILE LLMからの応答を待機中の状態で, THE App SHALL ローディングインジケーターを表示する

### Requirement 10: ファイル添付

**User Story:** As a ユーザー, I want チャットにファイルを添付してキャラクターに読ませたい, so that ファイルの内容を踏まえた会話ができる。

#### Acceptance Criteria

1. WHEN ユーザーがファイル選択ダイアログまたはドラッグ&ドロップでファイルを添付した時, THE Attachment_Processor SHALL ファイルを読み込み内容を抽出する
2. THE Attachment_Processor SHALL テキストファイル（.txt, .md, .csv）、PDFファイル（.pdf）、画像ファイル（.png, .jpg, .webp）の添付に対応する
3. WHEN テキストまたはPDFファイルが添付された時, THE Attachment_Processor SHALL ファイル内容をテキストとして抽出しLLMプロンプトに含める
4. WHEN 画像ファイルが添付された時, THE Attachment_Processor SHALL マルチモーダル対応LLMに画像データをBase64エンコードして送信する
5. IF 添付ファイルのサイズが上限（10MB）を超えた時, THEN THE Attachment_Processor SHALL エラーメッセージを表示し添付を拒否する
6. IF 添付ファイルの形式が非対応の時, THEN THE Attachment_Processor SHALL 対応形式の一覧を含むエラーメッセージを表示する
7. WHEN ファイルが添付されたメッセージを表示する時, THE Chat_Engine SHALL ファイル名とアイコンを添付インジケーターとして表示する
8. THE Attachment_Processor SHALL 添付ファイルの内容抽出結果をChatMessageRecordのattachmentsフィールドに保存する

### Requirement 11: Tool Use / プラグイン

**User Story:** As a ユーザー, I want キャラクターが外部ツールを呼び出せるようにしたい, so that ファイル操作やWeb検索など会話を超えた機能を利用できる。

#### Acceptance Criteria

1. THE Plugin_System SHALL OpenAI Function Calling互換のツール定義フォーマット（name, description, parameters JSON Schema）を採用する
2. THE Plugin_Registry SHALL プラグインの登録・一覧取得・有効化・無効化を管理する
3. WHEN LLMがtool_callレスポンスを返した時, THE Plugin_System SHALL 対応するPluginのハンドラを実行し結果をLLMに返す
4. THE Plugin_System SHALL Rustのtrait（PluginHandler）としてプラグインインターフェースを定義し、第三者が機能拡張可能な構造を提供する
5. THE Plugin_System SHALL 組み込みプラグインとして以下を提供する: ファイル読み書き、Web検索、計算
6. WHEN ユーザーがプラグイン管理画面を開いた時, THE Plugin_Registry SHALL 各Pluginの名前、説明、有効/無効状態、提供するTool一覧を表示する
7. IF Pluginのハンドラ実行が失敗した時, THEN THE Plugin_System SHALL エラー内容をLLMにtoolレスポンスとして返し、キャラクターがエラーを説明する応答を生成する
8. THE Plugin_System SHALL プラグインごとの設定（APIキー、ファイルアクセス範囲等）をAppConfigに保存する
9. WHEN tool_callの実行中, THE Chat_Engine SHALL ツール実行中であることを示すインジケーターとツール名を表示する
10. THE Plugin_System SHALL Tauri側のネイティブ機能（ファイルシステム、シェル実行等）をプラグインから安全に呼び出せるサンドボックス機構を提供する
