# Requirements Document

## Introduction

LLMプロバイダーが返すthinking/reasoningコンテンツ（モデルの思考過程）を、破棄せずに受信・保持し、ユーザーに表示可能にする機能。現在の実装では全プロバイダーのthinking/reasoningが完全にスキップされているため、バックエンドのストリーミング処理、イベント伝達、フロントエンドの表示を一貫して再設計する。

## Glossary

- **Thinking_Content**: LLMプロバイダーが返すモデルの思考過程テキスト。Anthropicの`thinking`ブロック、OpenAI互換の`reasoning_content`/`reasoning`フィールド、Geminiの`thought: true`パート、`<think>`タグ内テキストを含む
- **Chat_Engine**: Rustバックエンドのチャットストリーミングエンジン。LLMクライアントからのレスポンスを処理し、フロントエンドにイベントを発行する
- **Stream_Event**: バックエンドからフロントエンドに送信されるTauriイベント（`chat:stream`）のペイロード
- **Message_Bubble**: フロントエンドでチャットメッセージを表示するUIコンポーネント
- **LLM_Client**: 各プロバイダー（Anthropic、OpenAI互換、Gemini）のAPIと通信するRustモジュール
- **Thinking_Block**: ストリーミング中にThinking_Contentが連続して送信される単位。1回のレスポンスに対して0個または1個存在する
- **Chat_Store**: フロントエンドのZustandストア。ストリーミング状態とメッセージ履歴を管理する

## Requirements

### Requirement 1: Thinking Contentのストリーミング受信

**User Story:** As a user, I want the application to receive thinking/reasoning content from LLM providers during streaming, so that the model's thought process is not lost.

#### Acceptance Criteria

1. WHEN Anthropicプロバイダーが`thinking`または`redacted_thinking`タイプのcontent_blockを返した場合, THE LLM_Client SHALL 当該ブロックのテキストデルタをThinking_Content文字列として蓄積し、通常テキストのコールバックには含めない
2. WHEN OpenAI互換プロバイダーが`reasoning_content`または`reasoning`フィールドを含むチャンクを返した場合, THE LLM_Client SHALL 当該フィールドの値をThinking_Content文字列として蓄積し、通常テキストのコールバックには含めない
3. WHEN Geminiプロバイダーが`thought: true`フラグを持つpartを返した場合, THE LLM_Client SHALL 当該partのテキストをThinking_Content文字列として蓄積し、通常テキストのコールバックには含めない
4. WHEN コンテンツに`<think>`タグが含まれる場合, THE LLM_Client SHALL 開始タグと終了タグの間のテキストをThinking_Content文字列として抽出し、チャンク境界をまたぐタグについてはバッファリングにより正しく検出する
5. WHEN ストリーミングが完了した時点で, THE LLM_Client SHALL Thinking_Content文字列をLLMResponse内の独立したフィールドとして返し、通常コンテンツ（テキストまたはToolCalls）とは別のフィールドで保持する
6. IF ストリーミング中にThinking_Contentが一切検出されなかった場合, THEN THE LLM_Client SHALL Thinking_ContentフィールドをNone（空）として返す

### Requirement 2: Thinking Content用のイベント伝達

**User Story:** As a frontend developer, I want thinking content to be delivered separately from regular content in stream events, so that the UI can handle them independently.

#### Acceptance Criteria

1. THE Stream_Event SHALL thinkingフィールド（文字列型またはnull）を持ち、Thinking_Contentのデルタテキストを格納する
2. WHEN LLMプロバイダからThinking_Contentのデルタを受信した場合（Anthropicのthinkingブロック、Geminiのthought part、`<think>`タグ内テキスト）, THE Chat_Engine SHALL 当該デルタを除外せず、Stream_Eventのthinkingフィールドに設定して発行する
3. WHEN 通常コンテンツのデルタを受信した場合, THE Chat_Engine SHALL Stream_Eventのchunkフィールドにデルタテキストを設定して発行する（既存動作を維持）
4. WHEN 1つのストリーミングチャンクにThinking_Contentと通常コンテンツが同時に含まれる場合, THE Chat_Engine SHALL それぞれを別のフィールド（thinking, chunk）に設定した単一のStream_Eventとして発行する
5. IF Stream_EventにThinking_Contentのデルタが含まれない場合, THEN THE Chat_Engine SHALL thinkingフィールドをnullとして送信する（後方互換性維持）
6. WHEN done=trueまたはtool_break=trueのStream_Eventを発行する場合, THE Chat_Engine SHALL thinkingフィールドをnullに設定する

### Requirement 3: フロントエンドでのThinking Content状態管理

**User Story:** As a user, I want the application to track thinking content during streaming, so that it can be displayed in the UI.

#### Acceptance Criteria

1. THE Chat_Store SHALL ストリーミング中のThinking_Contentを蓄積するための状態フィールド（初期値: 空文字列）を持つ
2. WHEN Stream_Eventのthinkingフィールドがnullでなく空文字でもない値を含む場合, THE Chat_Store SHALL 蓄積中のthinkingContent状態フィールドに当該デルタ文字列を末尾に連結する
3. WHEN ストリーミングが完了した場合, THE Chat_Store SHALL 蓄積されたThinking_Contentをメッセージ一覧に追加するアシスタントメッセージレコードのthinkingフィールドに格納する
4. IF ストリーミング完了時にthinkingContent蓄積フィールドが空文字列の場合, THEN THE Chat_Store SHALL メッセージレコードのthinkingフィールドをnullとして設定する
5. WHEN 新しいストリーミングが開始された場合, THE Chat_Store SHALL thinkingContent蓄積フィールドを空文字列にリセットする

### Requirement 4: Thinking Contentの永続化

**User Story:** As a user, I want thinking content to be saved with messages, so that I can review the model's reasoning later.

#### Acceptance Criteria

1. THE ChatMessageRecord SHALL thinking_contentフィールド（Option<String>型）を持ち、LLMが返した推論テキストを格納する
2. WHEN アシスタントメッセージをDBに保存する際にLLMレスポンスにthinkingブロックが含まれている場合, THE Chat_Engine SHALL 抽出したthinking_contentをメッセージレコードのthinking_contentフィールドに格納して保存する
3. WHEN チャット履歴をDBから取得した場合, THE Chat_Engine SHALL 保存されたthinking_contentをメッセージレコードに含めて返す
4. IF LLMレスポンスにthinkingブロックが含まれていない場合, THEN THE Chat_Engine SHALL メッセージレコードのthinking_contentフィールドをnullとして保存する
5. WHEN thinking_contentを保存する場合, THE Chat_Engine SHALL 200,000文字を上限として格納し、上限を超過した場合は先頭から200,000文字までを切り詰めて保存する

### Requirement 5: Thinking ContentのUI表示

**User Story:** As a user, I want to view the model's thinking process in the chat interface, so that I can understand how the model arrived at its response.

#### Acceptance Criteria

1. WHEN アシスタントメッセージにThinking_Contentが関連付けられている場合, THE Message_Bubble SHALL メッセージバブルの上部に折り畳み可能なThinking_Content表示セクションを提供する
2. THE Message_Bubble SHALL Thinking_Contentをデフォルトで折り畳んだ状態で表示する（ストリーミング完了後）
3. WHEN ユーザーが折り畳みトグルを操作した場合, THE Message_Bubble SHALL Thinking_Contentの表示/非表示を切り替える（トグル状態はメッセージ単位で管理し、セッション内で非永続）
4. WHILE ストリーミング中にThinking_Contentを受信している場合, THE Message_Bubble SHALL Thinking_Contentセクションを展開状態で表示し、受信したトークンを逐次追記表示する
5. THE Message_Bubble SHALL Thinking_Contentを背景色の差異・左ボーダー・イタリック体により通常コンテンツと視覚的に区別する
6. IF Thinking_Contentがnullまたは空文字列の場合, THEN THE Message_Bubble SHALL Thinking_Content表示セクションを一切レンダリングしない

### Requirement 6: Anthropic redacted_thinkingの処理

**User Story:** As a user, I want redacted thinking blocks to be handled gracefully, so that the UI clearly indicates when thinking content has been redacted.

#### Acceptance Criteria

1. WHEN Anthropicプロバイダーが`redacted_thinking`タイプのcontent_blockを返した場合, THE LLM_Client SHALL 当該ブロックをThinking_Contentとして蓄積し、テキストデルタの代わりに編集済み（redacted）であることを示すマーカーを設定する
2. WHEN 編集済み思考ブロックのStream_Eventを受信した場合, THE Chat_Store SHALL 当該Thinking_Contentが編集済みであることを示すフラグを保持する
3. WHEN アシスタントメッセージに編集済みThinking_Contentが関連付けられている場合, THE Message_Bubble SHALL Requirement 5の折り畳みUI内に「思考内容は非表示です」という固定プレースホルダーテキストを表示し、通常のThinking_Content表示とは異なることを視覚的に区別する
4. IF 1つのレスポンスに通常の`thinking`ブロックと`redacted_thinking`ブロックが混在する場合, THEN THE LLM_Client SHALL 各ブロックの種別（通常/編集済み）を個別に保持し、出現順序を維持する

### Requirement 7: ストリーミング中のThinking状態表示

**User Story:** As a user, I want to know when the model is currently thinking, so that I can understand what phase of response generation is in progress.

#### Acceptance Criteria

1. WHILE LLMがThinking_Content（Anthropicのthinking/redacted_thinkingブロック、または`<think>`タグ内コンテンツ）を生成中の場合, THE Chat_Store SHALL `isThinking`状態フラグをtrueに設定して保持する
2. WHEN ストリーミング開始後にisThinkingフラグがtrueに設定された場合, THE Message_Bubble SHALL テキスト「思考中...」とアニメーション付きインジケーター（点滅またはバウンスドットアニメーション）を表示する
3. WHEN LLMから最初のテキストコンテンツ（thinking以外）のストリーミングチャンクを受信した場合, THE Chat_Store SHALL `isThinking`フラグをfalseに設定する
4. IF ストリーミングがthinking状態のまま完了または中断された場合, THEN THE Chat_Store SHALL `isThinking`フラグをfalseにリセットする

### Requirement 8: tool_break時のThinking Content処理

**User Story:** As a developer, I want thinking content to be properly handled when tool calls interrupt the stream, so that thinking is preserved across tool execution boundaries.

#### Acceptance Criteria

1. WHEN tool_breakイベントが発生した場合, THE Chat_Store SHALL その時点までに蓄積されたThinking_Contentを、直前に確定されるアシスタントバブル（commitPreToolContentで生成されるメッセージ）のメタデータとして関連付けて保持する
2. WHEN tool実行後にストリーミングが再開された場合, THE Chat_Store SHALL Thinking_Content蓄積バッファを空文字列にリセットし、新たなチャンクの蓄積を開始する
3. IF tool_break発生時にThinking_Contentが空（0文字）の場合, THEN THE Chat_Store SHALL 確定バブルにThinking_Contentフィールドを付与しない（undefinedまたはnullのまま保持する）
