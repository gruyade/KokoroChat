import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { PluginInfo } from '../types';

interface PluginState {
  plugins: PluginInfo[];
  loading: boolean;
  error: string | null;
  fetchPlugins: () => Promise<void>;
  enablePlugin: (name: string) => Promise<void>;
  disablePlugin: (name: string) => Promise<void>;
}

export const usePluginStore = create<PluginState>((set, get) => ({
  plugins: [],
  loading: false,
  error: null,

  fetchPlugins: async () => {
    set({ loading: true, error: null });
    try {
      const plugins = await invoke<PluginInfo[]>('list_plugins');
      set({ plugins, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  enablePlugin: async (name: string) => {
    set({ error: null });
    try {
      await invoke('enable_plugin', { name });
      const { plugins } = get();
      set({
        plugins: plugins.map((p) => (p.name === name ? { ...p, enabled: true } : p)),
      });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  disablePlugin: async (name: string) => {
    set({ error: null });
    try {
      await invoke('disable_plugin', { name });
      const { plugins } = get();
      set({
        plugins: plugins.map((p) => (p.name === name ? { ...p, enabled: false } : p)),
      });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },
}));
