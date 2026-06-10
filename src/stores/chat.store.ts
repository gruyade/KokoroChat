import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Attachment, ChatSession, ChatMessageRecord } from '../types';

interface ChatState {
  sessions: ChatSession[];
  currentSessionId: string | null;
  messages: ChatMessageRecord[];
  isStreaming: boolean;
  isAbortable: boolean;
  streamingContent: string;
  /** ストリーミング中のthinking/reasoning content蓄積バッファ */
  streamingThinkingContent: string;
  /** LLMがthinking/reasoning contentを生成中かどうか */
  isThinking: boolean;
  error: string | null;
  editingMessageId: string | null;
  isTTSGenerating: boolean;
  executingToolName: string | null;
  fetchSessions: (characterId: string) => Promise<void>;
  createSession: (characterId: string) => Promise<string>;
  selectSession: (sessionId: string | null) => void;
  sendMessage: (content: string, attachments?: Attachment[]) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
  fetchHistory: (sessionId: string) => Promise<void>;
  appendStreamChunk: (chunk: string) => void;
  /** thinking/reasoning chunkを蓄積バッファに追記し、isThinkingをtrueに設定 */
  appendThinkingChunk: (chunk: string) => void;
  /** ツール実行前テキストを確定バブルとして追加し、streamingContent をリセット（isStreaming は維持）*/
  commitPreToolContent: (content: string) => void;
  finishStreaming: (fullContent: string) => void;
  setTTSGenerating: (value: boolean) => void;
  finishWithAudio: (text: string, audio: string) => void;
  regenerateMessage: (messageId: string) => Promise<void>;
  stopGeneration: () => Promise<void>;
  setEditingMessage: (id: string | null) => void;
  editAndResend: (messageId: string, newContent: string) => Promise<void>;
  setExecutingTool: (toolName: string | null) => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  sessions: [],
  currentSessionId: null,
  messages: [],
  isStreaming: false,
  isAbortable: false,
  streamingContent: '',
  streamingThinkingContent: '',
  isThinking: false,
  error: null,
  editingMessageId: null,
  isTTSGenerating: false,
  executingToolName: null,

  fetchSessions: async (characterId: string) => {
    set({ error: null });
    try {
      const sessions = await invoke<ChatSession[]>('list_sessions', { characterId });
      set({ sessions });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  createSession: async (characterId: string) => {
    set({ error: null });
    try {
      const sessionId = await invoke<string>('create_session', { characterId });
      const { sessions } = get();
      const newSession: ChatSession = {
        id: sessionId,
        character_id: characterId,
        created_at: new Date().toISOString(),
      };
      set({
        sessions: [newSession, ...sessions],
        currentSessionId: sessionId,
        messages: [],
      });
      return sessionId;
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  selectSession: (sessionId: string | null) => {
    set({ currentSessionId: sessionId, messages: [], streamingContent: '', streamingThinkingContent: '', isThinking: false });
  },

  sendMessage: async (content: string, attachments?: Attachment[]) => {
    const { currentSessionId, messages, isStreaming, isTTSGenerating } = get();
    if (!currentSessionId) return;

    // ストリーミング中・TTS生成中は送信をブロック（連打防止）
    if (isStreaming || isTTSGenerating) return;

    // ユーザーメッセージをローカルに即座に追加（楽観的更新）
    // 添付ファイル情報をMessageAttachment形式に変換してローカル表示用に保持
    const messageAttachments = attachments?.map((a) => ({
      file_name: a.file_name,
      attachment_type: a.attachment_type,
      extracted_text: a.extracted_text,
      base64_data: a.base64_data,
    }));
    const userMessage: ChatMessageRecord = {
      id: crypto.randomUUID(),
      session_id: currentSessionId,
      role: 'user',
      content,
      created_at: new Date().toISOString(),
      attachments: messageAttachments,
    };
    const previousMessages = messages;
    set({ messages: [...messages, userMessage], isStreaming: true, isAbortable: true, streamingContent: '', streamingThinkingContent: '', isThinking: false, error: null });

    try {
      await invoke('send_message', {
        sessionId: currentSessionId,
        content,
        attachments: attachments ?? null,
      });
    } catch (e) {
      // 送信失敗時はローカルに追加したメッセージをロールバック
      set({ messages: previousMessages, error: String(e), isStreaming: false, isAbortable: false });
      throw e;
    }
  },

  deleteSession: async (sessionId: string) => {
    set({ error: null });
    try {
      await invoke('delete_session', { sessionId });
      const { sessions, currentSessionId } = get();
      set({
        sessions: sessions.filter((s) => s.id !== sessionId),
        currentSessionId: currentSessionId === sessionId ? null : currentSessionId,
        messages: currentSessionId === sessionId ? [] : get().messages,
      });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  fetchHistory: async (sessionId: string) => {
    set({ error: null });
    try {
      const messages = await invoke<ChatMessageRecord[]>('get_history', { sessionId });
      set({ messages });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  appendStreamChunk: (chunk: string) => {
    set((state) => ({
      streamingContent: state.streamingContent + chunk,
      isStreaming: true,
      isThinking: false,
      executingToolName: null,
    }));
  },

  appendThinkingChunk: (chunk: string) => {
    set((state) => ({
      streamingThinkingContent: state.streamingThinkingContent + chunk,
      isThinking: true,
    }));
  },

  commitPreToolContent: (content: string) => {
    const { currentSessionId, messages, streamingThinkingContent } = get();
    // テキストがある場合のみバブルとして追加
    if (!content.trim()) return;
    const assistantMessage: ChatMessageRecord = {
      id: crypto.randomUUID(),
      session_id: currentSessionId ?? '',
      role: 'assistant',
      content,
      // thinking contentが蓄積済みならメッセージに設定、空ならnullのまま保持
      thinking_content: streamingThinkingContent || null,
      created_at: new Date().toISOString(),
    };
    set({
      messages: [...messages, assistantMessage],
      streamingContent: '',              // 次のストリーミングのためリセット
      streamingThinkingContent: '',      // thinkingバッファをリセット
      isThinking: false,                 // thinking状態をリセット
      executingToolName: null,           // tool:executing イベントで上書きされる
      // isStreaming は true のまま維持
    });
  },

  finishStreaming: (fullContent: string) => {
    const { currentSessionId, messages, streamingThinkingContent } = get();
    const assistantMessage: ChatMessageRecord = {
      id: crypto.randomUUID(),
      session_id: currentSessionId ?? '',
      role: 'assistant',
      content: fullContent,
      thinking_content: streamingThinkingContent || null,
      created_at: new Date().toISOString(),
    };
    set({
      messages: [...messages, assistantMessage],
      isStreaming: false,
      isAbortable: false,
      streamingContent: '',
      streamingThinkingContent: '',
      isThinking: false,
      executingToolName: null,
    });
  },

  setTTSGenerating: (value: boolean) => {
    set({ isTTSGenerating: value });
  },

  finishWithAudio: (text: string, _audio: string) => {
    const { currentSessionId, messages } = get();
    const assistantMessage: ChatMessageRecord = {
      id: crypto.randomUUID(),
      session_id: currentSessionId ?? '',
      role: 'assistant',
      content: text,
      created_at: new Date().toISOString(),
    };
    set({
      messages: [...messages, assistantMessage],
      isStreaming: false,
      isAbortable: false,
      isTTSGenerating: false,
      streamingContent: '',
      streamingThinkingContent: '',
      isThinking: false,
    });
  },

  regenerateMessage: async (messageId: string) => {
    const { currentSessionId, messages } = get();
    if (!currentSessionId) return;

    // 対象メッセージをローカル状態から削除し、ストリーミング開始（楽観的更新）
    const previousMessages = messages;
    set({
      messages: messages.filter((m) => m.id !== messageId),
      isStreaming: true,
      isAbortable: true,
      streamingContent: '',
      streamingThinkingContent: '',
      isThinking: false,
      error: null,
    });

    try {
      await invoke('regenerate_message', {
        sessionId: currentSessionId,
        messageId,
      });
    } catch (e) {
      // 失敗時はメッセージをロールバック
      set({ messages: previousMessages, error: String(e), isStreaming: false, isAbortable: false });
    }
  },

  stopGeneration: async () => {
    const { currentSessionId } = get();
    if (!currentSessionId) return;

    try {
      await invoke('stop_generation', { sessionId: currentSessionId });
      // isAbortable は finishStreaming（chat:stream done イベント受信時）でクリアされる
      // ここでは送信ボタンの連打を防ぐためのみ設定
      set({ isAbortable: false, isThinking: false });
    } catch {
      // 停止コマンド失敗時は無視（ストリーミングは自然完了を待つ）
      set({ isAbortable: false, isThinking: false });
    }
  },

  setEditingMessage: (id: string | null) => {
    set({ editingMessageId: id });
  },

  editAndResend: async (messageId: string, newContent: string) => {
    const { currentSessionId, messages } = get();
    if (!currentSessionId) return;

    // 編集対象メッセージ以降をローカル状態から削除し、対象メッセージの内容を更新（楽観的更新）
    const targetIndex = messages.findIndex((m) => m.id === messageId);
    if (targetIndex === -1) return;

    const previousMessages = messages;
    const updatedMessages = messages.slice(0, targetIndex + 1).map((m) =>
      m.id === messageId ? { ...m, content: newContent } : m
    );

    set({
      messages: updatedMessages,
      isStreaming: true,
      isAbortable: true,
      streamingContent: '',
      streamingThinkingContent: '',
      isThinking: false,
      error: null,
      editingMessageId: null,
    });

    try {
      await invoke('edit_and_resend', {
        sessionId: currentSessionId,
        messageId,
        newContent,
      });
    } catch (e) {
      // 失敗時はメッセージをロールバック
      set({ messages: previousMessages, error: String(e), isStreaming: false, isAbortable: false });
    }
  },

  setExecutingTool: (toolName: string | null) => {
    set({ executingToolName: toolName });
  },
}));
