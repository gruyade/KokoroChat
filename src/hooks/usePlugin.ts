import { useEffect } from 'react';
import { usePluginStore } from '../stores/plugin.store';

/**
 * プラグイン操作Hook
 * - マウント時にプラグイン一覧を取得
 * - 有効/無効切り替え操作を提供
 */
export function usePlugin() {
  const plugins = usePluginStore((s) => s.plugins);
  const loading = usePluginStore((s) => s.loading);
  const error = usePluginStore((s) => s.error);
  const fetchPlugins = usePluginStore((s) => s.fetchPlugins);
  const enablePlugin = usePluginStore((s) => s.enablePlugin);
  const disablePlugin = usePluginStore((s) => s.disablePlugin);

  useEffect(() => {
    fetchPlugins();
  }, [fetchPlugins]);

  return {
    plugins,
    loading,
    error,
    enablePlugin,
    disablePlugin,
  };
}
