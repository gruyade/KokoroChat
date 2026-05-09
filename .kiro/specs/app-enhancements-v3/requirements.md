# Requirements Document

## Introduction

AI Character Chatデスクトップアプリ（Tauri v2 + React + TypeScript + Zustand）のv3機能強化仕様。システムメッセージ表示の修正、TTS関連UI改善、思考・記憶の即時反映、IrodoriTTS設定構造の変更、プロバイダー別API仕様対応、プロバイダー設定永続化バグの修正を含む。

## Glossary

- **App**: AI Character Chatデスクトップアプリケーション全体
- **Message_Bubble**: チャットメッセージの表示コンポーネント（MessageBubble.tsx）
- **System_Message_Badge**: システムメッセージを中央寄せバッジスタイルで表示するUI要素
- **Chat_Header_Controls**: チャット画面ヘッダーの操作ボタン群コンポーネント（ChatHeaderControls.tsx）
- **Volume_Control**: 音量スライダーおよびミュートボタンを含むUI要素
- **Thought_View**: 思考履歴を表示するビューコンポーネント（ThoughtView.tsx）
- **Memory_View**: 記憶一覧を表示するビューコンポーネント（MemoryView.tsx）
- **Config_Store**: アプリケーション設定のZustandストア（config.store.ts）
- **Settings_View**: アプリケーション設定画面全体
- **Model_Config_Form**: LLMモデル接続設定のフォームコンポーネント（ModelConfigForm.tsx）
- **LLM_Client**: Rust側のLLM API通信クライアント（OpenAICompatibleClient）
- **Backend_Config**: Rust側のAppConfig/ModelSettings構造体（models/config.rs）
- **IrodoriTTS_Config**: IrodoriTTS固有の設定（ベースURL、モード別設定）
- **TTS_Global_Config**: TTSグローバル設定（TTSGlobalConfig構造体）
- **Tauri_Event**: Tauriのイベントシステムによるバックエンド→フロントエンド通知

## Requirements

### Requirement 1: システムメッセージの中央寄せバッジ表示復元

**User Story:** ユーザーとして、システムメッセージを元の中央寄せバッジスタイルで表示しつつ、ホバー時に編集・削除操作を行いたい。視覚的にユーザーメッセージと区別するため。

#### Acceptance Criteria

1. THE Message_Bubble SHALL システムメッセージ（[SYSTEM]プレフィックス付き）を中央寄せのバッジスタイルで表示する
2. THE System_Message_Badge SHALL 背景色をmuted系、テキストをmuted-foreground系とし、コンパクトなバッジ形状で表示する
3. WHEN ユーザーがSystem_Message_Badgeにホバーした場合、THE Message_Bubble SHALL 編集ボタンおよび削除ボタンを表示する
4. WHEN ユーザーが編集ボタンを押下した場合、THE Message_Bubble SHALL インライン編集モードに切り替え、確定時に該当メッセージ以降をリセットし再送信する
5. WHEN ユーザーが削除ボタンを押下した場合、THE Message_Bubble SHALL 該当メッセージおよび後続メッセージを削除する
6. THE System_Message_Badge SHALL 右寄せバブルスタイル（ユーザーメッセージスタイル）を使用しない

### Requirement 2: TTS無効時のボリュームコントロール非表示

**User Story:** ユーザーとして、TTS機能が無効の場合にボリュームコントロールが表示されないようにしたい。使用できない機能のUIが表示されると混乱するため。

#### Acceptance Criteria

1. WHILE TTS機能がグローバル設定で無効の場合、THE Chat_Header_Controls SHALL Volume_Control（ミュートボタンおよび音量スライダー）を非表示にする
2. WHILE TTS機能がグローバル設定で有効の場合、THE Chat_Header_Controls SHALL Volume_Controlを表示する
3. WHEN TTS設定が有効から無効に変更された場合、THE Chat_Header_Controls SHALL Volume_Controlを即座に非表示にする

### Requirement 3: 思考・記憶生成の即時反映

**User Story:** ユーザーとして、思考や記憶が生成された際に、対応するタブを開いていれば即座に新しいデータが表示されるようにしたい。画面切り替えなしで最新状態を確認するため。

#### Acceptance Criteria

1. WHEN バックエンドで新しい思考が生成された場合、THE App SHALL Tauri_Eventを発行してフロントエンドに通知する
2. WHEN バックエンドで新しい記憶が生成された場合、THE App SHALL Tauri_Eventを発行してフロントエンドに通知する
3. WHILE Thought_Viewが表示されている場合、WHEN 思考生成イベントを受信した場合、THE Thought_View SHALL 思考一覧を再取得して即座に表示を更新する
4. WHILE Memory_Viewが表示されている場合、WHEN 記憶生成イベントを受信した場合、THE Memory_View SHALL 記憶一覧を再取得して即座に表示を更新する
5. THE App SHALL イベントリスナーをコンポーネントのマウント時に登録し、アンマウント時にクリーンアップする

### Requirement 4: IrodoriTTSベースURL設定のグローバル化

**User Story:** ユーザーとして、IrodoriTTSのベースURLをグローバル設定で一元管理しつつ、キャプションモードと参照音声モードで別々のベースURLを指定できるようにしたい。複数キャラクターで同じサーバーを使う際の設定重複を避けるため。

#### Acceptance Criteria

1. THE TTS_Global_Config SHALL IrodoriTTSのデフォルトベースURLフィールドを持つ
2. THE Settings_View SHALL TTSグローバル設定セクションにIrodoriTTSベースURL入力欄を表示する
3. WHERE キャラクター個別のTTS設定でベースURLが指定されている場合、THE App SHALL キャラクター個別のベースURLをグローバル設定より優先して使用する
4. WHERE キャラクター個別のTTS設定でベースURLが未指定の場合、THE App SHALL グローバル設定のベースURLをフォールバックとして使用する
5. THE IrodoriTTS_Config SHALL キャプションモード用ベースURLと参照音声モード用ベースURLを個別に指定可能にする
6. THE Backend_Config SHALL IrodoriTTSベースURL設定の追加に伴い、TTSGlobalConfig構造体にirodori_base_urlフィールドを追加する

### Requirement 5: プロバイダー別API仕様対応

**User Story:** ユーザーとして、Google・Anthropicを選択しデフォルトエンドポイントを使用する場合に、各社固有のAPI形式で通信してほしい。OpenAI互換ではないAPIを正しく利用するため。

#### Acceptance Criteria

1. WHEN プロバイダーがGoogleでエンドポイントがデフォルトの場合、THE LLM_Client SHALL Google Gemini API形式でリクエストを構築する
2. WHEN プロバイダーがAnthropicでエンドポイントがデフォルトの場合、THE LLM_Client SHALL Anthropic Messages API形式でリクエストを構築する
3. WHEN プロバイダーがOpenAIでエンドポイントがデフォルトの場合、THE LLM_Client SHALL OpenAI Chat Completions API形式でリクエストを構築する
4. WHEN プロバイダーがOpenAI互換の場合、THE LLM_Client SHALL OpenAI互換API形式でリクエストを構築する
5. WHEN プロバイダーがGoogle/Anthropicでカスタムエンドポイントが指定されている場合、THE LLM_Client SHALL OpenAI互換API形式でリクエストを構築する
6. THE LLM_Client SHALL プロバイダー情報をModelSettingsから受け取り、API形式の判定に使用する
7. THE LLM_Client SHALL 各プロバイダーのレスポンス形式を正しくパースし、統一されたLLMResponse型に変換する

### Requirement 6: プロバイダー設定の永続化バグ修正

**User Story:** ユーザーとして、保存したプロバイダー選択が画面切り替え後も正しく表示されてほしい。設定が失われると毎回再設定が必要になるため。

#### Acceptance Criteria

1. THE Backend_Config SHALL ModelSettings構造体にproviderフィールド（Option型）を追加する
2. WHEN 設定が保存される場合、THE Backend_Config SHALL providerフィールドを含めてJSONにシリアライズする
3. WHEN 設定が読み込まれる場合、THE Backend_Config SHALL providerフィールドをJSONからデシリアライズする
4. WHEN 画面が切り替えられた場合、THE Model_Config_Form SHALL 保存済みのprovider値を正しく表示する
5. IF 既存の設定ファイルにproviderフィールドが存在しない場合、THEN THE Backend_Config SHALL デフォルト値（None）として読み込み、既存設定との後方互換性を維持する
6. FOR ALL ModelSettings値、シリアライズ後にデシリアライズした結果は元の値と等価な内容を含む（ラウンドトリップ特性）
