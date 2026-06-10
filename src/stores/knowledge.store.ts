import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { InjectionMode, KnowledgeEntryMeta } from '../types';

interface KnowledgeState {
  entries: KnowledgeEntryMeta[];
  loading: boolean;
  error: string | null;
  fetchEntries: (sessionId: string) => Promise<void>;
  addKnowledge: (sessionId: string, fileName: string, content: string) => Promise<void>;
  removeKnowledge: (sessionId: string, fileName: string) => Promise<void>;
  toggleKnowledge: (sessionId: string, fileName: string, enabled: boolean) => Promise<void>;
  setInjectionMode: (sessionId: string, fileName: string, mode: InjectionMode) => Promise<void>;
  exportKnowledge: (sessionId: string, fileName: string) => Promise<string>;
}

export const useKnowledgeStore = create<KnowledgeState>((set, get) => ({
  entries: [],
  loading: false,
  error: null,

  fetchEntries: async (sessionId: string) => {
    set({ loading: true, error: null });
    try {
      const entries = await invoke<KnowledgeEntryMeta[]>('list_knowledge', { sessionId });
      set({ entries, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  addKnowledge: async (sessionId: string, fileName: string, content: string) => {
    set({ loading: true, error: null });
    try {
      const entry = await invoke<KnowledgeEntryMeta>('add_knowledge', {
        sessionId,
        fileName,
        content,
      });
      const { entries } = get();
      // UPSERT: 同名ファイルが既存なら置換、なければ追加
      const existing = entries.findIndex((e) => e.file_name === entry.file_name);
      const updated =
        existing >= 0
          ? entries.map((e, i) => (i === existing ? entry : e))
          : [...entries, entry];
      set({ entries: updated, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  removeKnowledge: async (sessionId: string, fileName: string) => {
    set({ error: null });
    try {
      await invoke('remove_knowledge', { sessionId, fileName });
      const { entries } = get();
      set({ entries: entries.filter((e) => e.file_name !== fileName) });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  toggleKnowledge: async (sessionId: string, fileName: string, enabled: boolean) => {
    set({ error: null });
    try {
      await invoke('toggle_knowledge', { sessionId, fileName, enabled });
      const { entries } = get();
      set({
        entries: entries.map((e) =>
          e.file_name === fileName ? { ...e, enabled } : e
        ),
      });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  setInjectionMode: async (sessionId: string, fileName: string, mode: InjectionMode) => {
    const { entries } = get();
    const previous = entries.find((e) => e.file_name === fileName);
    if (!previous) return;

    // 楽観的更新
    set({
      error: null,
      entries: entries.map((e) =>
        e.file_name === fileName ? { ...e, injection_mode: mode } : e
      ),
    });

    try {
      await invoke('set_knowledge_injection_mode', {
        sessionId,
        fileName,
        injectionMode: mode,
      });
    } catch (e) {
      // ロールバック
      const { entries: currentEntries } = get();
      set({
        error: String(e),
        entries: currentEntries.map((e) =>
          e.file_name === fileName
            ? { ...e, injection_mode: previous.injection_mode }
            : e
        ),
      });
      throw e;
    }
  },

  exportKnowledge: async (sessionId: string, fileName: string) => {
    set({ error: null });
    try {
      const content = await invoke<string>('export_knowledge', { sessionId, fileName });
      return content;
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },
}));
