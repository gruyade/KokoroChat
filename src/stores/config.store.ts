import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, ModelSettings } from '../types';

interface ConfigState {
  config: AppConfig | null;
  loading: boolean;
  error: string | null;
  fetchConfig: () => Promise<void>;
  updateConfig: (config: AppConfig) => Promise<void>;
  testLLMConnection: (settings: ModelSettings) => Promise<void>;
}

export const useConfigStore = create<ConfigState>((set) => ({
  config: null,
  loading: false,
  error: null,

  fetchConfig: async () => {
    set({ loading: true, error: null });
    try {
      const config = await invoke<AppConfig>('get_config');
      set({ config, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  updateConfig: async (config: AppConfig) => {
    set({ loading: true, error: null });
    try {
      await invoke('set_config', { config });
      set({ config, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  testLLMConnection: async (settings: ModelSettings) => {
    set({ error: null });
    try {
      await invoke('test_llm_connection', { settings });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },
}));
