import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { ChatSession, ChatMessageRecord } from '../types';

interface ChatState {
  sessions: ChatSession[];
  currentSessionId: string | null;
  messages: ChatMessageRecord[];
  isStreaming: boolean;
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
}

export const useChatStore = create<ChatState>((set, get) => ({
  sessions: [],
  currentSessionId: null,
  messages: [],
  isStreaming: false,
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
    const { currentSessionId } = get();
    if (!currentSessionId) return;

    set({ isStreaming: true, streamingContent: '', error: null });
    try {
      await invoke('send_message', {
        sessionId: currentSessionId,
        content,
        attachments: attachments ?? null,
      });
    } catch (e) {
      set({ error: String(e), isStreaming: false });
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
      streamingContent: '',
    });
  },
}));
