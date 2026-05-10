import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { CustomToolRequest, PluginInfo } from '../types';

/** 組み込みプラグイン名リスト（削除不可） */
const BUILTIN_PLUGIN_NAMES = ['calculator', 'web_search', 'file_ops'];

/** セッション単位のツール許可状態: tool_name -> enabled */
export type SessionToolPermissions = Record<string, boolean>;

interface PluginState {
  plugins: PluginInfo[];
  loading: boolean;
  error: string | null;
  /** セッションごとのツール許可マップ: session_id -> { tool_name -> enabled } */
  sessionPermissions: Record<string, SessionToolPermissions>;
  fetchPlugins: () => Promise<void>;
  enablePlugin: (name: string) => Promise<void>;
  disablePlugin: (name: string) => Promise<void>;
  /** カスタムプラグインを登録 */
  registerCustomPlugin: (request: CustomToolRequest) => Promise<void>;
  /** カスタムプラグインを削除（組み込みは削除不可） */
  removePlugin: (name: string) => Promise<void>;
  /** プラグインが組み込みかどうか判定 */
  isBuiltinPlugin: (name: string) => boolean;
  /** セッション単位でツールの有効/無効を切り替える */
  setSessionToolEnabled: (sessionId: string, toolName: string, enabled: boolean) => void;
  /** セッションの許可状態を初期化（グローバル設定をデフォルトとして適用） */
  initSessionPermissions: (sessionId: string) => void;
  /** セッションのツール許可状態を取得 */
  getSessionPermissions: (sessionId: string) => SessionToolPermissions;
}

export const usePluginStore = create<PluginState>((set, get) => ({
  plugins: [],
  loading: false,
  error: null,
  sessionPermissions: {},

  fetchPlugins: async () => {
    set({ loading: true, error: null });
    try {
      const plugins = await invoke<PluginInfo[]>('list_plugins');
      // 組み込みフラグを付与
      const enriched = plugins.map((p) => ({
        ...p,
        builtin: BUILTIN_PLUGIN_NAMES.includes(p.name),
      }));
      set({ plugins: enriched, loading: false });
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

  registerCustomPlugin: async (request: CustomToolRequest) => {
    set({ error: null });
    try {
      await invoke('register_custom_plugin', { request });
      // 再取得して最新状態を反映
      await get().fetchPlugins();
    } catch {
      // バックエンドコマンドが未実装の場合はフロントエンドでローカル追加
      const { plugins } = get();
      const newPlugin: PluginInfo = {
        name: request.name,
        description: request.description,
        version: '1.0.0',
        enabled: true,
        tools: [
          {
            name: request.name,
            description: request.description,
            parameters: {},
          },
        ],
        config: {
          type: request.type,
          ...(request.type === 'http' ? { url: request.endpoint } : { command: request.command }),
        },
        builtin: false,
      };
      set({ plugins: [...plugins, newPlugin] });
    }
  },

  removePlugin: async (name: string) => {
    const { isBuiltinPlugin } = get();
    if (isBuiltinPlugin(name)) {
      set({ error: '組み込みプラグインは削除できない' });
      return;
    }
    set({ error: null });
    try {
      await invoke('unregister_plugin', { name });
    } catch {
      // バックエンドコマンドが未実装でもフロントエンドから除去
    }
    const { plugins } = get();
    set({ plugins: plugins.filter((p) => p.name !== name) });
  },

  isBuiltinPlugin: (name: string) => {
    return BUILTIN_PLUGIN_NAMES.includes(name);
  },

  setSessionToolEnabled: (sessionId: string, toolName: string, enabled: boolean) => {
    const { sessionPermissions } = get();
    const current = sessionPermissions[sessionId] ?? {};
    set({
      sessionPermissions: {
        ...sessionPermissions,
        [sessionId]: { ...current, [toolName]: enabled },
      },
    });
  },

  initSessionPermissions: (sessionId: string) => {
    const { plugins, sessionPermissions } = get();
    // 既に初期化済みならスキップ
    if (sessionPermissions[sessionId]) return;
    // グローバル設定をデフォルト値として適用
    const permissions: SessionToolPermissions = {};
    for (const plugin of plugins) {
      for (const tool of plugin.tools) {
        permissions[tool.name] = plugin.enabled;
      }
    }
    set({
      sessionPermissions: {
        ...sessionPermissions,
        [sessionId]: permissions,
      },
    });
  },

  getSessionPermissions: (sessionId: string) => {
    const { sessionPermissions } = get();
    return sessionPermissions[sessionId] ?? {};
  },
}));
