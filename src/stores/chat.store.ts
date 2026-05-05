import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { ChatSession, ChatMessageRecord } from '../types';

interface ChatState {
  sessions: ChatSession[];
  currentSessionId: string | null;
  messages: ChatMessageRecord[];
  isStreaming: boolean;
  isAbortable: boolean;
  streamingContent: string;
  error: string | null;
  fetchSessions: (characterId: string) => Promise<void>;
  createSession: (characterId: string) => Promise<string>;
  selectSession: (sessionId: string | null) => void;
  sendMessage: (content: string, attachments?: string[]) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
  fetchHistory: (sessionId: string) => Promise<void>;
  appendStreamChunk: (chunk: string) => void;
  finishStreaming: (fullContent: string) => void;
  regenerateMessage: (messageId: string) => Promise<void>;
  stopGeneration: () => Promise<void>;
}

export const useChatStore = create<ChatState>((set, get) => ({
  sessions: [],
  currentSessionId: null,
  messages: [],
  isStreaming: false,
  isAbortable: false,
  streamingContent: '',
  error: null,

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
    set({ currentSessionId: sessionId, messages: [], streamingContent: '' });
  },

  sendMessage: async (content: string, attachments?: string[]) => {
    const { currentSessionId, messages } = get();
    if (!currentSessionId) return;

    // ユーザーメッセージをローカルに即座に追加
    const userMessage: ChatMessageRecord = {
      id: crypto.randomUUID(),
      session_id: currentSessionId,
      role: 'user',
      content,
      created_at: new Date().toISOString(),
    };
    set({ messages: [...messages, userMessage], isStreaming: true, isAbortable: true, streamingContent: '', error: null });

    try {
      await invoke('send_message', {
        sessionId: currentSessionId,
        content,
        attachments: attachments ?? null,
      });
    } catch (e) {
      set({ error: String(e), isStreaming: false, isAbortable: false });
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
    }));
  },

  finishStreaming: (fullContent: string) => {
    const { currentSessionId, messages } = get();
    const assistantMessage: ChatMessageRecord = {
      id: crypto.randomUUID(),
      session_id: currentSessionId ?? '',
      role: 'assistant',
      content: fullContent,
      created_at: new Date().toISOString(),
    };
    set({
      messages: [...messages, assistantMessage],
      isStreaming: false,
      isAbortable: false,
      streamingContent: '',
    });
  },

  regenerateMessage: async (messageId: string) => {
    const { currentSessionId, messages } = get();
    if (!currentSessionId) return;

    // 対象メッセージをローカル状態から削除し、ストリーミング開始
    set({
      messages: messages.filter((m) => m.id !== messageId),
      isStreaming: true,
      isAbortable: true,
      streamingContent: '',
      error: null,
    });

    try {
      await invoke('regenerate_message', {
        sessionId: currentSessionId,
        messageId,
      });
    } catch (e) {
      set({ error: String(e), isStreaming: false, isAbortable: false });
    }
  },

  stopGeneration: async () => {
    const { currentSessionId } = get();
    if (!currentSessionId) return;

    try {
      await invoke('stop_generation', { sessionId: currentSessionId });
    } catch {
      // 停止コマンド失敗時は無視（ストリーミングは自然完了を待つ）
    }
    set({ isAbortable: false });
  },
}));
