# Implementation Plan: Chat UX Improvements

## Overview

AI Character Chat のチャットUXを包括的に改善する実装計画。11要件を依存関係順に実装し、各要件を独立したタスクグループとして構成（1要件 = 1コミット単位）。バックエンド拡張 → フロントエンド基盤 → UI改善の順で進行。

**技術スタック**: Rust (Tauri 2) + TypeScript (React 19 + Zustand 5) + Tailwind CSS 3
**テスト**: proptest (Rust), Vitest + fast-check (TypeScript)
**新規依存**: react-markdown, remark-gfm, rehype-highlight, rehype-sanitize

## Tasks

- [-] 1. Requirement 1: Thought Auto-Deletion（思考自動削除）
  - [-] 1.1 Backend: Config model に `auto_delete_threshold_minutes` フィールド追加
    - `src-tauri/src/models/config.rs` の `ThoughtConfig` に `auto_delete_threshold_minutes: u64` 追加（デフォルト1440）
    - `#[serde(default = "default_thought_auto_delete_threshold")]` で後方互換性確保
    - TypeScript側 `src/types/config.ts` の `ThoughtConfig` にも同フィールド追加
    - _Requirements: 1.1_

  - [ ] 1.2 Backend: thought_repo に削除系関数追加
    - `src-tauri/src/db/repositories/thought.rs` に `delete_thought(conn, id)` 追加
    - `delete_thoughts_older_than(conn, character_id, cutoff_time)` 追加（削除件数を返す）
    - `get_recent_thoughts(conn, character_id, since)` 追加
    - _Requirements: 1.2, 1.3_

  - [ ] 1.3 Backend: ThoughtEngine に `cleanup_old_thoughts` メソッド追加
    - `src-tauri/src/thought/engine.rs` に閾値超過思考の削除ロジック実装
    - `generate_thought` 内で新思考生成後に自動クリーンアップ呼び出し
    - threshold が 0 の場合はクリーンアップをスキップ（全保持）
    - _Requirements: 1.2, 1.5_

  - [ ] 1.4 Backend: Tauri Command `delete_thought` 追加
    - `src-tauri/src/commands/thought.rs` に `delete_thought` コマンド実装
    - 存在しないIDの場合は `AppError::NotFound` を返す
    - _Requirements: 1.3_

  - [ ]* 1.5 Backend: 思考自動削除のプロパティテスト
    - **Property 1: Thought auto-deletion preserves only recent thoughts**
    - ランダムなThoughtリスト（0〜50件）、ランダムなtimestamp、ランダムなthreshold（0〜2880分）で検証
    - threshold=0 の場合に全保持されることも検証
    - **Validates: Requirements 1.2, 1.5**

  - [ ]* 1.6 Backend: 手動削除のプロパティテスト
    - **Property 2: Manual thought deletion removes exactly the target**
    - ランダムなThought ID、ランダムな既存Thoughtセットで検証
    - 他の思考が影響を受けないことを確認
    - **Validates: Requirements 1.3**

  - [ ] 1.7 Frontend: ThoughtView に削除ボタン追加
    - `src/components/thought/ThoughtView.tsx` の各思考エントリに削除ボタン表示
    - `invoke('delete_thought', { id })` 呼び出し → 成功時にリストから除去
    - _Requirements: 1.4_

  - [ ] 1.8 Frontend: ConfigStore に `auto_delete_threshold_minutes` 反映
    - `src/stores/config.store.ts` で ThoughtConfig の新フィールドを管理
    - 設定画面（SettingsView）に閾値設定UIを追加（分単位入力 or プリセット選択）
    - _Requirements: 1.1_

- [ ] 2. Requirement 2: Thoughts Reflected in Chat（思考のチャット反映）
  - [ ] 2.1 Backend: ChatEngine.build_context に思考コンテキスト追加
    - `src-tauri/src/chat/engine.rs` の `build_context` シグネチャに `thoughts: &[Thought]` パラメータ追加
    - system prompt 内に recent thoughts セクションを挿入
    - `send_message` 内で `get_recent_thoughts` を呼び出し、閾値内の思考を取得して `build_context` に渡す
    - _Requirements: 2.1, 2.2_

  - [ ] 2.2 Backend: 思考なし時のフォールバック処理
    - 思考が0件の場合、思考セクションを省略して標準プロンプトで続行
    - _Requirements: 2.3_

  - [ ]* 2.3 Backend: 思考コンテキスト包含のプロパティテスト
    - **Property 3: Thought context inclusion respects threshold**
    - ランダムなThoughtリスト、ランダムなthreshold、ランダムなsystem_promptで検証
    - 閾値内の思考のみがプロンプトに含まれることを確認
    - **Validates: Requirements 2.1, 2.2, 2.3**

- [ ] 3. Checkpoint - バックエンド思考関連確認
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Requirement 4: Chat Regeneration（チャット再生成）
  - [ ] 4.1 Backend: ChatEngine に `regenerate` メソッド追加
    - `src-tauri/src/chat/engine.rs` に再生成ロジック実装
    - 対象assistantメッセージをDBから削除
    - 直前のuserメッセージを特定し、そのcontentで再送信
    - ストリーミングレスポンスを通常の `send_message` と同様にEvent emit
    - _Requirements: 4.1, 4.2_

  - [ ] 4.2 Backend: Tauri Command `regenerate_message` 追加
    - `src-tauri/src/commands/chat.rs` に `regenerate_message` コマンド実装
    - メッセージID不存在時は `AppError::NotFound`、先行userメッセージなし時は `AppError::InvalidInput`
    - _Requirements: 4.1, 4.2_

  - [ ]* 4.3 Backend: 再生成のプロパティテスト
    - **Property 4: Regeneration replaces target message correctly**
    - ランダムなメッセージ履歴（2〜20件、user/assistant交互）で検証
    - 対象メッセージ削除と先行userメッセージ特定の正確性を確認
    - **Validates: Requirements 4.1, 4.2**

  - [ ] 4.4 Frontend: ChatStore に `regenerateMessage` アクション追加
    - `src/stores/chat.store.ts` に `regenerateMessage(messageId)` 実装
    - `invoke('regenerate_message', { sessionId, messageId })` 呼び出し
    - 成功時: 対象メッセージをローカル状態から削除、ストリーミング開始
    - 失敗時: エラーバナー表示、isStreaming を false に戻す
    - _Requirements: 4.3, 4.4_

  - [ ] 4.5 Frontend: MessageBubble に再生成ボタン追加
    - `src/components/chat/MessageBubble.tsx` のassistantメッセージにregenerateボタン表示（ホバー時）
    - クリック時に `regenerateMessage` 呼び出し
    - _Requirements: 4.3_

- [ ] 5. Requirement 5: Stop Generation Button（生成停止ボタン）
  - [ ] 5.1 Backend: StreamAbortManager 実装
    - `src-tauri/src/chat/engine.rs` に `StreamAbortManager` 構造体追加（AppStateに保持）
    - `register(session_id, abort_handle, partial_content)` — ストリーム開始時に登録
    - `abort(session_id)` — 中断＋部分コンテンツ返却
    - `remove(session_id)` — 正常完了時にクリーンアップ
    - _Requirements: 5.2_

  - [ ] 5.2 Backend: send_message のストリーミング処理に AbortHandle 統合
    - ストリーミング開始時に `StreamAbortManager.register` 呼び出し
    - ストリーミング中に部分コンテンツを `Arc<Mutex<String>>` に蓄積
    - 正常完了時に `StreamAbortManager.remove` 呼び出し
    - 中断時: 部分コンテンツをassistantメッセージとしてDB保存
    - _Requirements: 5.2, 5.3_

  - [ ] 5.3 Backend: Tauri Command `stop_generation` 追加
    - `src-tauri/src/commands/chat.rs` に `stop_generation` コマンド実装
    - アクティブストリームがない場合は何もせず `Ok(())` を返す
    - _Requirements: 5.2_

  - [ ]* 5.4 Backend: 中断時の部分コンテンツ保存プロパティテスト
    - **Property 5: Abort preserves partial content**
    - ランダムな部分コンテンツ文字列（0〜10000文字）で検証
    - 中断後に部分コンテンツが正しく返却されることを確認
    - **Validates: Requirements 5.3**

  - [ ] 5.5 Frontend: ChatStore に `isAbortable` 状態と `stopGeneration` アクション追加
    - `src/stores/chat.store.ts` に `isAbortable: boolean` フィールド追加
    - ストリーミング開始時に `isAbortable = true`、完了/中断時に `false`
    - `stopGeneration()` で `invoke('stop_generation', { sessionId })` 呼び出し
    - _Requirements: 5.1, 5.4_

  - [ ] 5.6 Frontend: ChatView/MessageInput に停止ボタン表示
    - `isAbortable` が true の間、入力エリアに停止ボタン表示
    - クリック時に `stopGeneration()` 呼び出し
    - 停止後: ボタン非表示、入力再有効化
    - _Requirements: 5.1, 5.4_

- [ ] 6. Requirement 7: User Message Editing（ユーザーメッセージ編集）
  - [ ] 6.1 Backend: ChatEngine に `edit_and_resend` メソッド追加
    - `src-tauri/src/chat/engine.rs` に編集＋再送信ロジック実装
    - 対象メッセージ以降の全メッセージをDBから削除
    - 対象メッセージのcontentを新しい内容に更新
    - 更新後のcontentで再送信（ストリーミング）
    - 対象がuser roleでない場合は `AppError::InvalidInput`
    - _Requirements: 7.3, 7.4_

  - [ ] 6.2 Backend: Tauri Command `edit_and_resend` 追加
    - `src-tauri/src/commands/chat.rs` に `edit_and_resend` コマンド実装
    - パラメータ: session_id, message_id, new_content
    - _Requirements: 7.3, 7.4_

  - [ ]* 6.3 Backend: メッセージ編集のプロパティテスト
    - **Property 7: Message edit truncates history and updates content**
    - ランダムなメッセージ履歴、ランダムな編集位置で検証
    - 編集位置以降が削除され、結果の履歴長が正しいことを確認
    - **Validates: Requirements 7.3, 7.4**

  - [ ] 6.4 Frontend: ChatStore に `editAndResend` と `editingMessageId` 追加
    - `src/stores/chat.store.ts` に `editingMessageId: string | null` フィールド追加
    - `setEditingMessage(id | null)` — 編集モード切り替え
    - `editAndResend(messageId, newContent)` — invoke呼び出し → ローカル状態更新
    - _Requirements: 7.3, 7.4_

  - [ ] 6.5 Frontend: MessageBubble に編集ボタンと EditableMessage コンポーネント追加
    - `src/components/chat/MessageBubble.tsx` のuserメッセージに編集ボタン表示（ホバー時）
    - 編集モード時: `EditableMessage` コンポーネントに切り替え（textarea + 確認/キャンセルボタン）
    - 確認時: `editAndResend` 呼び出し
    - キャンセル時: `setEditingMessage(null)` で元の表示に復帰
    - _Requirements: 7.1, 7.2, 7.5_

- [ ] 7. Checkpoint - バックエンドチャット操作確認
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 8. Requirement 8: Pause Thought/Spontaneous Speech（思考・自発的発話の一時停止）
  - [ ] 8.1 Backend: ThoughtEngine に pause/resume 機能追加
    - `src-tauri/src/thought/engine.rs` に `paused: Arc<AtomicBool>` フラグ追加
    - `pause()` — フラグを true に設定、タイマーループ内でスキップ
    - `resume()` — フラグを false に設定、インターバルリセット
    - _Requirements: 8.3, 8.5_

  - [ ] 8.2 Backend: SpontaneousEngine に pause/resume 機能追加
    - 自発的発話エンジンに同様の `paused` フラグ追加
    - pause時はタイマーチェックをスキップ
    - resume時はインターバルリセットして再開
    - _Requirements: 8.4, 8.5_

  - [ ] 8.3 Backend: Tauri Commands（pause/resume）追加
    - `pause_thought_engine`, `resume_thought_engine` コマンド実装
    - `pause_spontaneous`, `resume_spontaneous` コマンド実装
    - _Requirements: 8.3, 8.4_

  - [ ] 8.4 Frontend: ChatView ヘッダーに一時停止トグル追加
    - `ChatHeaderControls` コンポーネント作成（思考トグル + 自発的発話トグル）
    - `src/components/chat/ChatView.tsx` のヘッダーエリアに配置
    - トグル操作時に対応する invoke 呼び出し
    - 失敗時: トグル状態を元に戻し、エラートースト表示
    - _Requirements: 8.1, 8.2_

- [ ] 9. Requirement 3: Chat Focus Retention After Send（送信後フォーカス保持）
  - [ ] 9.1 Frontend: MessageInput の送信後フォーカス保持
    - `src/components/chat/MessageInput.tsx` の送信処理後に `textareaRef.current?.focus()` 呼び出し
    - キーボードショートカット送信時も同様にフォーカス保持
    - `useEffect` でストリーミング完了後のフォーカス復帰も対応
    - _Requirements: 3.1, 3.2_

- [ ] 10. Requirement 10: Configurable Send Key（送信キー設定）
  - [ ] 10.1 Backend: Config model に `send_key` フィールド追加
    - `src-tauri/src/models/config.rs` の `UIConfig` に `send_key: SendKey` enum追加
    - `SendKey` enum定義: `Enter`, `CtrlEnter`, `ShiftEnter`（デフォルト `Enter`）
    - `#[serde(default)]` で後方互換性確保
    - TypeScript側 `src/types/config.ts` に `SendKey` 型追加
    - _Requirements: 10.1_

  - [ ] 10.2 Frontend: MessageInput のキーハンドリング改修
    - `src/components/chat/MessageInput.tsx` の `handleKeyDown` を設定値に基づいて分岐
    - 設定されたキーコンビネーション → 送信
    - それ以外のEnter系コンビネーション → 改行挿入
    - ConfigStore から `send_key` 設定を参照
    - _Requirements: 10.2, 10.3_

  - [ ]* 10.3 Frontend: 送信キー設定のプロパティテスト
    - **Property 9: Send key configuration determines send/newline behavior**
    - ランダムな `send_key` 設定値、ランダムなキーイベントで検証
    - 設定キーで送信、他のEnter系で改行が挿入されることを確認
    - **Validates: Requirements 10.2, 10.3**

  - [ ] 10.4 Frontend: SettingsView に送信キー設定ドロップダウン追加
    - `src/components/settings/SettingsView.tsx` の「一般」タブにドロップダウン追加
    - 選択肢: Enter / Ctrl+Enter / Shift+Enter
    - 変更時に ConfigStore 経由で保存
    - _Requirements: 10.4_

- [ ] 11. Requirement 6: Smooth Scrolling During Generation（スムーズスクロール）
  - [ ] 11.1 Frontend: ChatView のオートスクロール改善
    - `src/components/chat/ChatView.tsx` に `shouldAutoScroll` 判定関数追加
    - スクロール位置が底から200px以内 → オートスクロール有効
    - 200px超離れている → オートスクロール一時停止（ユーザーが上方を閲覧中）
    - ユーザーが底に戻った場合 → オートスクロール再開
    - _Requirements: 6.2, 6.3_

  - [ ] 11.2 Frontend: スムーズスクロールアニメーション適用
    - ストリーミング中のスクロールに `scrollBehavior: 'smooth'` 適用
    - `requestAnimationFrame` ベースの滑らかなスクロール実装
    - _Requirements: 6.1_

  - [ ]* 11.3 Frontend: オートスクロール判定のプロパティテスト
    - **Property 6: Auto-scroll pauses when user scrolls away**
    - ランダムな scrollHeight, scrollTop, clientHeight で検証
    - 200px閾値に基づくスクロール有効/無効の正確性を確認
    - **Validates: Requirements 6.2, 6.3**

- [ ] 12. Requirement 9: Markdown Rendering in Chat（Markdownレンダリング）
  - [ ] 12.1 依存パッケージインストール
    - `react-markdown`, `remark-gfm`, `rehype-highlight`, `rehype-sanitize` をインストール
    - _Requirements: 9.1_

  - [ ] 12.2 Frontend: MarkdownRenderer コンポーネント作成
    - `src/components/chat/MarkdownRenderer.tsx` 作成
    - `react-markdown` + `remark-gfm` でGFM対応Markdown表示
    - `rehype-sanitize` でXSS防止（script, on*属性, javascript: URI除去）
    - `rehype-highlight` でコードブロックのシンタックスハイライト
    - コードブロックにコピーボタン追加
    - レンダリングエラー時はプレーンテキストフォールバック
    - _Requirements: 9.1, 9.3, 9.4_

  - [ ]* 12.3 Frontend: Markdown XSSサニタイズのプロパティテスト
    - **Property 8: Markdown sanitization prevents XSS**
    - ランダムな文字列（XSSペイロード含む）で検証
    - 出力に `<script>`, `on*` 属性, `javascript:` が含まれないことを確認
    - **Validates: Requirements 9.3**

  - [ ] 12.4 Frontend: MessageBubble に MarkdownRenderer 統合
    - `src/components/chat/MessageBubble.tsx` でassistant/userメッセージの表示を MarkdownRenderer に切り替え
    - Tailwind CSS でMarkdown要素のスタイリング（prose クラス等）
    - _Requirements: 9.1, 9.2_

- [ ] 13. Checkpoint - フロントエンド主要機能確認
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 14. Requirement 11: Delete Button Visibility After Slide Animation（削除ボタン表示修正）
  - [ ] 14.1 Frontend: メッセージ削除アニメーション後のボタンアクセシビリティ修正
    - `src/components/chat/ChatView.tsx` / `MessageBubble.tsx` のCSS transition 修正
    - スライドアニメーション完了後に `pointer-events` と `overflow` を適切に設定
    - `transitionend` イベントで再配置後のボタンが確実にインタラクティブになるよう保証
    - ホバートリガーのアクションボタン（delete, edit, regenerate）がクリップされないよう `overflow: visible` 確保
    - _Requirements: 11.1, 11.2, 11.3_

- [ ] 15. Final checkpoint - 全テスト通過確認
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- `*` 付きサブタスクはオプション（スキップ可能）— プロパティテスト・ユニットテスト系
- 各タスクグループは1つの要件に対応し、独立してコミット可能
- 依存関係順: Config拡張(1) → 思考反映(2) → 再生成(4) → 停止(5) → 編集(7) → 一時停止(8) → フォーカス(3) → 送信キー(10) → スクロール(6) → Markdown(9) → アニメーション修正(11)
- バックエンド変更を先に実装し、フロントエンドが依存するAPIを確定させる
- Property tests は proptest (Rust) / fast-check (TypeScript) で最低100イテレーション実行
