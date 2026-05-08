# Requirements Document

## Introduction

AI Character Chatデスクトップアプリ（Tauri + React + TypeScript）の包括的な機能強化仕様。キャラクターのインポート/エクスポート、非同期データ処理アーキテクチャ、UI/UX改善、モデル設定UI改善、バグ修正を含む。

## Glossary

- **App**: AI Character Chatデスクトップアプリケーション全体
- **Character_Manager**: キャラクターのCRUD操作およびインポート/エクスポートを管理するコンポーネント
- **Export_Engine**: キャラクターデータをJSONファイルとしてエクスポートする処理エンジン
- **Import_Engine**: JSONファイルからキャラクターデータを読み込みDBに保存する処理エンジン
- **Character_Form**: キャラクター編集フォームUIコンポーネント
- **Operation_Queue**: DB書き込み等の非同期操作をキューイングして順次実行するグローバルオブジェクト
- **Memory_Generator**: 会話履歴からLLMを使って記憶（要約）を生成するエンジン
- **Message_Bubble**: チャットメッセージの表示コンポーネント
- **Chat_Header_Controls**: チャット画面ヘッダーの操作ボタン群コンポーネント
- **Model_Config_Form**: LLMモデル接続設定のフォームコンポーネント
- **Settings_View**: アプリケーション設定画面全体
- **Hover_Action_Buttons**: メッセージバブル上のホバー時に表示されるアクションボタン群

## Requirements

### Requirement 1: キャラクターエクスポート

**User Story:** ユーザーとして、キャラクターデータをファイルにエクスポートしたい。バックアップや他環境への移行のため。

#### Acceptance Criteria

1. WHEN ユーザーがキャラクターカードのエクスポートボタンを押下した場合、THE Character_Manager SHALL エクスポートオプションダイアログを表示する
2. THE Export_Engine SHALL キャラクター設定（name, description, system_prompt, tts_config）を必須データとしてエクスポートに含める
3. WHERE チャット履歴オプションが選択された場合、THE Export_Engine SHALL 該当キャラクターの全ChatSessionおよびChatMessageRecordをエクスポートデータに含める
4. WHERE 思考オプションが選択された場合、THE Export_Engine SHALL 該当キャラクターの全Thoughtをエクスポートデータに含める
5. WHERE 記憶オプションが選択された場合、THE Export_Engine SHALL 該当キャラクターの全Memoryをエクスポートデータに含める
6. WHEN エクスポートが実行された場合、THE Export_Engine SHALL JSON形式のファイルとしてファイルシステムに保存する
7. IF エクスポート中にエラーが発生した場合、THEN THE App SHALL エラーメッセージをトースト通知で表示する

### Requirement 2: キャラクターインポート

**User Story:** ユーザーとして、エクスポートしたキャラクターデータをインポートしたい。他環境からの移行やバックアップ復元のため。

#### Acceptance Criteria

1. WHEN ユーザーがキャラクター管理画面のインポートボタンを押下した場合、THE Character_Manager SHALL ファイル選択ダイアログを表示する
2. WHEN JSONファイルが選択された場合、THE Import_Engine SHALL ファイル内容を解析しインポートオプションダイアログを表示する
3. THE Import_Engine SHALL キャラクター設定を必須データとしてインポートする
4. WHERE チャット履歴オプションが選択された場合、THE Import_Engine SHALL チャット履歴データを新規キャラクターに紐付けてインポートする
5. WHERE 思考オプションが選択された場合、THE Import_Engine SHALL 思考データを新規キャラクターに紐付けてインポートする
6. WHERE 記憶オプションが選択された場合、THE Import_Engine SHALL 記憶データを新規キャラクターに紐付けてインポートする
7. WHEN インポートが完了した場合、THE Character_Manager SHALL キャラクター一覧を更新し成功トーストを表示する
8. IF インポートファイルのフォーマットが不正な場合、THEN THE Import_Engine SHALL 具体的なエラー内容をユーザーに表示する
9. FOR ALL エクスポートされたキャラクターデータ、インポート後に再エクスポートした結果は元データと等価な内容を含む（ラウンドトリップ特性）

### Requirement 3: キャラクター編集フォームのスクロール修正

**User Story:** ユーザーとして、キャラクター編集フォームを快適にスクロールしたい。フォーム内部ではなく親コンテナでスクロールするため。

#### Acceptance Criteria

1. WHILE キャラクター編集フォームが表示されている場合、THE Character_Form SHALL フォーム自体にoverflow-y-autoを設定しない
2. WHILE キャラクター編集フォームが表示されている場合、THE Character_Manager SHALL フォームの親コンテナ（1階層上）でスクロールを制御する
3. THE Character_Form SHALL フォーム内容が画面高さを超える場合でも、親コンテナのスクロールバーのみを使用して全内容にアクセス可能にする

### Requirement 4: 非同期データ処理アーキテクチャ

**User Story:** ユーザーとして、画面切り替え時にデータ処理が中断されないようにしたい。バックグラウンドで確実にタスクが完了するため。

#### Acceptance Criteria

1. THE Operation_Queue SHALL 画面コンポーネントのライフサイクルに依存しないグローバルオブジェクトとして存在する
2. WHEN 画面が切り替えられた場合、THE Operation_Queue SHALL 実行中および待機中のタスクを中断せず継続する
3. THE Operation_Queue SHALL DB書き込み操作をキューに追加された順序で逐次実行する
4. IF キュー内のタスクが失敗した場合、THEN THE Operation_Queue SHALL エラーをログに記録し次のタスクの実行を継続する
5. THE Operation_Queue SHALL 現在の処理状態（処理中/待機中タスク数）を購読可能なステートとして公開する

### Requirement 5: 手動メモリ生成ボタン

**User Story:** ユーザーとして、任意のタイミングで記憶生成を手動トリガーしたい。チャット切り替え前に現在の会話を記憶に保存するため。

#### Acceptance Criteria

1. THE Chat_Header_Controls SHALL 手動メモリ生成ボタンを表示する
2. WHEN ユーザーが手動メモリ生成ボタンを押下した場合、THE Memory_Generator SHALL 現在のアクティブセッションの会話履歴から記憶を生成する
3. WHILE メモリ生成が実行中の場合、THE Chat_Header_Controls SHALL ボタンをローディング状態で表示し重複実行を防止する
4. WHEN メモリ生成が完了した場合、THE App SHALL 成功トーストを表示する
5. IF メモリ生成に失敗した場合、THEN THE App SHALL エラーメッセージをトースト通知で表示する

### Requirement 6: システムメッセージのUX変更

**User Story:** ユーザーとして、システムメッセージをユーザーメッセージと同様に操作したい。編集・削除・再生成リセットを行うため。

#### Acceptance Criteria

1. THE Message_Bubble SHALL システムメッセージ（[SYSTEM]プレフィックス付きメッセージ）を右寄せで表示する
2. WHEN ユーザーがシステムメッセージにホバーした場合、THE Hover_Action_Buttons SHALL 編集・削除ボタンを表示する
3. WHEN ユーザーがシステムメッセージの編集を確定した場合、THE App SHALL 該当メッセージ以降のメッセージをリセットし再送信する
4. WHEN ユーザーがシステムメッセージを削除した場合、THE App SHALL 該当メッセージおよび後続メッセージを削除する

### Requirement 7: TTS WIPラベル

**User Story:** ユーザーとして、TTS機能が開発中であることを認識したい。未完成機能への期待値を適切に設定するため。

#### Acceptance Criteria

1. THE Settings_View SHALL TTSタブのヘッダーに「WIP」バッジを表示する
2. THE App SHALL READMEファイルのTTS関連セクションにWIP注記を含める

### Requirement 8: モデル設定UIの改善

**User Story:** ユーザーとして、LLMプロバイダーを簡単に選択し設定したい。手動でBase URLを調べる手間を省くため。

#### Acceptance Criteria

1. THE Model_Config_Form SHALL プロバイダー選択コンボボックス（OpenAI, Anthropic, Google, OpenAI互換）を表示する
2. WHEN OpenAI, Anthropic, またはGoogleが選択された場合、THE Model_Config_Form SHALL Base URLフィールドをオプション扱いとし、未入力時はデフォルト値を自動適用する
3. WHEN OpenAI互換が選択された場合、THE Model_Config_Form SHALL Base URLフィールドを必須入力として表示する
4. WHEN Base URLとAPI Keyが入力された場合、THE Model_Config_Form SHALL 利用可能なモデル一覧の取得を試行する
5. WHEN モデル一覧の取得に成功した場合、THE Model_Config_Form SHALL ドロップダウンリストからの選択と手動テキスト入力の両方を許可する
6. IF モデル一覧の取得に失敗した場合、THEN THE Model_Config_Form SHALL 手動テキスト入力のみを許可しエラーを非破壊的に表示する

### Requirement 9: 条件付きボタン表示制御

**User Story:** ユーザーとして、無効化された機能のボタンが表示されないようにしたい。UIの混乱を避けるため。

#### Acceptance Criteria

1. WHILE 思考機能がグローバル設定で無効の場合、THE Chat_Header_Controls SHALL 思考一時停止ボタンを非表示にする
2. WHILE 自発的発話機能がグローバル設定で無効の場合、THE Chat_Header_Controls SHALL 自発的発話一時停止ボタンを非表示にする
3. WHILE TTS機能がグローバル設定で無効の場合、THE Message_Bubble SHALL 音声生成ボタンを非表示にする
4. WHEN グローバル設定が変更された場合、THE App SHALL ボタンの表示/非表示を即座に反映する

### Requirement 10: ホバーアクションボタンのバグ修正

**User Story:** ユーザーとして、ホバーアクションボタンが常に正しく表示・動作してほしい。メッセージ操作を確実に行うため。

#### Acceptance Criteria

1. WHEN ユーザーがメッセージバブルにマウスを乗せた場合、THE Hover_Action_Buttons SHALL 確実にアクションボタンを表示する
2. WHEN アクションボタンが押下された場合、THE Hover_Action_Buttons SHALL ボタン押下後もマウスがバブル上にある限り表示を維持する
3. WHEN メッセージ削除アニメーション（スライド）が完了した場合、THE Hover_Action_Buttons SHALL 新たに表示されたメッセージバブルに対して正常にホバー検出を行う
4. THE Hover_Action_Buttons SHALL pointer-eventsの状態管理を適切に行い、ボタンのクリック可能状態を維持する
