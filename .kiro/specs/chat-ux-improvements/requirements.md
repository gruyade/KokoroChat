# Requirements Document

## Introduction

AI Character Chat アプリケーションのチャットUX改善。思考の自動削除、メッセージ編集、生成停止、Markdownレンダリング、送信キー設定など、チャット操作の利便性と快適性を向上させる複合的な改善を行う。

## Glossary

- **Chat_UI**: チャットメッセージの表示・入力を行うフロントエンドコンポーネント群（ChatView, MessageInput, MessageBubble等）
- **Thought_Manager**: キャラクターの独自思考（Thought）を管理するバックエンドモジュール
- **Chat_Engine**: チャットセッション・メッセージの送受信を管理するバックエンドエンジン
- **Message_Input**: チャットメッセージの入力・送信を行うテキストエリアコンポーネント
- **Streaming_Indicator**: LLM応答のストリーミング中に表示されるUIコンポーネント
- **Config_Store**: アプリケーション設定の読み書きを管理するフロントエンドストア
- **Spontaneous_Timer**: 自発的発話のタイミングを制御するタイマー機構
- **Thought_Engine**: 独自思考の生成タイミングを制御するバックエンドエンジン
- **Send_Key**: メッセージ送信に使用するキーボードショートカット（Enter / Ctrl+Enter / Shift+Enter）

## Requirements

### Requirement 1: Thought Auto-Deletion

**User Story:** As a user, I want old thoughts to be automatically deleted after a configurable time threshold, so that the context window does not grow unboundedly large.

#### Acceptance Criteria

1. THE Config_Store SHALL provide a `thought_auto_delete_threshold_minutes` setting with a default value of 1440 (24 hours)
2. WHEN the Thought_Engine generates a new thought, THE Thought_Manager SHALL delete all thoughts for that character whose `created_at` exceeds the configured threshold
3. WHEN a user manually triggers thought deletion, THE Thought_Manager SHALL delete the specified thought and return a success confirmation
4. THE Chat_UI SHALL display a delete button on each thought entry in the ThoughtView component
5. WHILE the `thought_auto_delete_threshold_minutes` setting is set to 0, THE Thought_Manager SHALL retain all thoughts indefinitely (auto-deletion disabled)

### Requirement 2: Thoughts Reflected in Chat

**User Story:** As a user, I want to verify that character thoughts are being used in chat responses, so that the character's internal reasoning influences conversation.

#### Acceptance Criteria

1. WHEN the Chat_Engine constructs the LLM prompt for a chat response, THE Chat_Engine SHALL include recent thoughts for the active character as context in the system prompt
2. THE Chat_Engine SHALL include thoughts generated within the configured threshold period only
3. WHEN no thoughts exist for the active character, THE Chat_Engine SHALL proceed with the standard prompt without thought context

### Requirement 3: Chat Focus Retention After Send

**User Story:** As a user, I want focus to remain on the chat input after sending a message, so that I can continue typing without clicking the input again.

#### Acceptance Criteria

1. WHEN a message is sent via the Message_Input, THE Message_Input SHALL retain keyboard focus on the textarea element
2. WHEN a message is sent via keyboard shortcut, THE Message_Input SHALL retain keyboard focus on the textarea element

### Requirement 4: Chat Regeneration (Replace, Not Append)

**User Story:** As a user, I want response regeneration to replace the existing response rather than appending a new one, so that the conversation history remains clean.

#### Acceptance Criteria

1. WHEN a user triggers regeneration on an assistant message, THE Chat_Engine SHALL delete the target assistant message from the database
2. WHEN a user triggers regeneration on an assistant message, THE Chat_Engine SHALL resend the preceding user message to generate a new response
3. WHEN regeneration completes, THE Chat_UI SHALL display the new response in place of the deleted one
4. IF regeneration fails, THEN THE Chat_UI SHALL display an error message and the original message SHALL remain deleted from history

### Requirement 5: Stop Generation Button

**User Story:** As a user, I want a stop button to cancel response generation while it is in progress, so that I can interrupt unwanted or lengthy responses.

#### Acceptance Criteria

1. WHILE the Chat_Engine is streaming a response, THE Chat_UI SHALL display a stop button in the input area
2. WHEN the user clicks the stop button, THE Chat_Engine SHALL abort the active LLM request
3. WHEN generation is stopped, THE Chat_UI SHALL display the partially generated content as the final assistant message
4. WHEN generation is stopped, THE Chat_UI SHALL hide the stop button and re-enable the message input

### Requirement 6: Smooth Scrolling During Generation

**User Story:** As a user, I want auto-scrolling during response generation to be smooth, so that the streaming text is comfortable to read.

#### Acceptance Criteria

1. WHILE the Chat_Engine is streaming a response, THE Chat_UI SHALL scroll the message container smoothly using CSS `scroll-behavior: smooth` or equivalent animation
2. WHILE the user has manually scrolled up (more than 200px from bottom), THE Chat_UI SHALL pause auto-scrolling to allow reading previous messages
3. WHEN the user scrolls back to the bottom during streaming, THE Chat_UI SHALL resume smooth auto-scrolling

### Requirement 7: User Message Editing

**User Story:** As a user, I want to edit my sent messages and resend them, so that I can correct mistakes or rephrase without starting a new conversation.

#### Acceptance Criteria

1. THE Chat_UI SHALL display an edit button on user message bubbles (visible on hover)
2. WHEN the user clicks the edit button, THE Chat_UI SHALL replace the message bubble with an editable textarea pre-filled with the original content
3. WHEN the user confirms the edit, THE Chat_Engine SHALL delete all messages after the edited message in the session
4. WHEN the user confirms the edit, THE Chat_Engine SHALL update the edited message content and resend it to generate a new response
5. WHEN the user cancels the edit, THE Chat_UI SHALL restore the original message display without changes

### Requirement 8: Pause Thought and Spontaneous Speech Generation

**User Story:** As a user, I want to pause thought generation and spontaneous speech from within the chat UI, so that I can control interruptions without navigating to settings.

#### Acceptance Criteria

1. THE Chat_UI SHALL display a toggle control in the chat header area to pause/resume thought generation
2. THE Chat_UI SHALL display a toggle control in the chat header area to pause/resume spontaneous speech
3. WHEN the user pauses thought generation, THE Thought_Engine SHALL stop generating new thoughts until resumed
4. WHEN the user pauses spontaneous speech, THE Spontaneous_Timer SHALL stop triggering spontaneous checks until resumed
5. WHEN the user resumes either feature, THE respective engine SHALL resume operation with the configured interval reset

### Requirement 9: Markdown Rendering in Chat

**User Story:** As a user, I want chat messages to be rendered as Markdown, so that formatted content (code blocks, lists, bold text) displays properly.

#### Acceptance Criteria

1. THE Chat_UI SHALL render assistant message content as Markdown with support for: headings, bold, italic, code blocks, inline code, lists, and links
2. THE Chat_UI SHALL render user message content as Markdown with the same formatting support
3. THE Chat_UI SHALL sanitize rendered Markdown to prevent XSS attacks (no raw HTML execution)
4. WHILE rendering code blocks, THE Chat_UI SHALL apply syntax highlighting with a copy-to-clipboard button

### Requirement 10: Configurable Send Key

**User Story:** As a user, I want to choose which key combination sends messages (Enter, Ctrl+Enter, or Shift+Enter), so that I can use my preferred workflow for multi-line input.

#### Acceptance Criteria

1. THE Config_Store SHALL provide a `send_key` setting with options: `enter`, `ctrl_enter`, `shift_enter` and a default value of `enter`
2. WHEN the configured Send_Key is pressed in the Message_Input, THE Message_Input SHALL send the message
3. WHEN a non-send key combination is pressed (e.g., Shift+Enter when send key is Enter), THE Message_Input SHALL insert a newline character
4. THE Settings view SHALL display a dropdown to select the send key preference under the "一般" (General) tab

### Requirement 11: Fix Memory Compression Triggering Every Message

**User Story:** As a user, I want memory compression to only trigger after a certain number of new messages since the last compression, so that it does not run on every single message once the threshold is exceeded.

#### Acceptance Criteria

1. WHEN the Memory_Manager checks whether to compress, THE Memory_Manager SHALL count only messages created after the last compression point (identified by `source_message_to` of the most recent memory for that session)
2. WHEN no previous compression exists for the session, THE Memory_Manager SHALL count all messages in the session against the threshold
3. THE Memory_Manager SHALL only compress messages that were created after the last compression point (not re-compress already-compressed messages)

### Requirement 12: Delete Button Visibility After Slide Animation

**User Story:** As a user, I want the delete button on newly slid-in chat messages to always be visible and clickable, so that I can delete messages immediately after they appear.

#### Acceptance Criteria

1. WHEN a message is deleted and the subsequent messages slide up to fill the gap, THE Chat_UI SHALL ensure the delete button on the newly positioned message is fully rendered and interactive
2. THE Chat_UI SHALL NOT clip or hide action buttons (delete, edit, regenerate) on messages that have been repositioned by a slide/transition animation
3. WHEN a slide animation completes, THE Chat_UI SHALL ensure all hover-triggered action buttons on affected messages are accessible without requiring the user to move the mouse away and back
