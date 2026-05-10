import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FolderOpen, Plus, X, Loader2 } from 'lucide-react';

/** ディレクトリごとの読み書き権限 */
interface DirectoryPermission {
  path: string;
  allow_read: boolean;
  allow_write: boolean;
}

/** file_ops プラグインの config_json 構造 */
interface FileOpsConfig {
  directories: DirectoryPermission[];
}

/** chat_plugin_configs テーブルのレコード */
interface ChatPluginConfig {
  id: string;
  session_id: string;
  plugin_name: string;
  config_json: string;
  updated_at: string;
}

interface FileOpsDirectoryManagerProps {
  sessionId: string;
}

/**
 * file_ops プラグインのディレクトリ権限管理コンポーネント。
 * セッション単位で許可ディレクトリの追加・削除・Read/Write トグルを提供し、
 * 変更を即座にバックエンドへ永続化する。
 */
export function FileOpsDirectoryManager({ sessionId }: FileOpsDirectoryManagerProps) {
  const [directories, setDirectories] = useState<DirectoryPermission[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [newPath, setNewPath] = useState('');

  // バックエンドから設定を読み込む
  const loadConfig = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const config = await invoke<ChatPluginConfig | null>('get_session_plugin_config', {
        sessionId,
        pluginName: 'file_ops',
      });
      if (config) {
        const parsed: FileOpsConfig = JSON.parse(config.config_json);
        setDirectories(parsed.directories);
      } else {
        setDirectories([]);
      }
    } catch (e) {
      setError(String(e));
      setDirectories([]);
    } finally {
      setLoading(false);
    }
  }, [sessionId]);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  // 変更をバックエンドへ保存
  const persistConfig = useCallback(
    async (dirs: DirectoryPermission[]) => {
      const configJson = JSON.stringify({ directories: dirs } satisfies FileOpsConfig);
      try {
        await invoke<ChatPluginConfig>('update_session_plugin_config', {
          sessionId,
          pluginName: 'file_ops',
          configJson,
        });
        setError(null);
      } catch (e) {
        setError(String(e));
      }
    },
    [sessionId]
  );

  // トグル変更（楽観的更新）
  const handleToggle = (index: number, field: 'allow_read' | 'allow_write') => {
    const updated = directories.map((dir, i) =>
      i === index ? { ...dir, [field]: !dir[field] } : dir
    );
    setDirectories(updated);
    persistConfig(updated);
  };

  // ディレクトリ削除（楽観的更新）
  const handleDelete = (index: number) => {
    const updated = directories.filter((_, i) => i !== index);
    setDirectories(updated);
    persistConfig(updated);
  };

  // 新規ディレクトリ追加
  const handleAdd = () => {
    const trimmed = newPath.trim();
    if (!trimmed) return;
    // 重複チェック
    if (directories.some((d) => d.path === trimmed)) {
      setError('同じパスが既に登録されている');
      return;
    }
    const newDir: DirectoryPermission = {
      path: trimmed,
      allow_read: true,
      allow_write: false,
    };
    const updated = [...directories, newDir];
    setDirectories(updated);
    setNewPath('');
    setError(null);
    persistConfig(updated);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleAdd();
    }
  };

  return (
    <div className="space-y-2">
      {/* Section header */}
      <div className="flex items-center gap-1.5 px-1">
        <FolderOpen className="w-3.5 h-3.5 text-muted-foreground" />
        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          ディレクトリ権限
        </span>
      </div>

      {/* Loading state */}
      {loading && (
        <div className="flex items-center justify-center py-3 text-muted-foreground text-xs gap-1.5">
          <Loader2 className="w-3.5 h-3.5 animate-spin" />
          <span>読み込み中...</span>
        </div>
      )}

      {/* Error display */}
      {error && (
        <div className="px-2 py-1.5 text-[10px] text-destructive bg-destructive/10 rounded-md">
          {error}
        </div>
      )}

      {/* Directory list */}
      {!loading && directories.length === 0 && (
        <div className="text-[10px] text-muted-foreground px-2 py-2">
          ディレクトリが未設定。下のフォームから追加できる。
        </div>
      )}

      {!loading &&
        directories.map((dir, index) => (
          <div
            key={`${dir.path}-${index}`}
            className="flex items-center gap-1.5 px-2 py-1.5 rounded-md hover:bg-muted/40 group"
          >
            {/* Path */}
            <div className="flex-1 min-w-0">
              <div className="text-xs font-mono truncate" title={dir.path}>
                {dir.path}
              </div>
            </div>

            {/* Read toggle */}
            <label className="flex items-center gap-0.5 text-[10px] text-muted-foreground cursor-pointer">
              <input
                type="checkbox"
                checked={dir.allow_read}
                onChange={() => handleToggle(index, 'allow_read')}
                className="rounded text-primary focus:ring-primary/50 w-3 h-3"
                aria-label={`${dir.path} の読み取り許可`}
              />
              <span>R</span>
            </label>

            {/* Write toggle */}
            <label className="flex items-center gap-0.5 text-[10px] text-muted-foreground cursor-pointer">
              <input
                type="checkbox"
                checked={dir.allow_write}
                onChange={() => handleToggle(index, 'allow_write')}
                className="rounded text-primary focus:ring-primary/50 w-3 h-3"
                aria-label={`${dir.path} の書き込み許可`}
              />
              <span>W</span>
            </label>

            {/* Delete button */}
            <button
              onClick={() => handleDelete(index)}
              className="p-0.5 rounded hover:bg-destructive/20 text-muted-foreground hover:text-destructive transition-colors opacity-0 group-hover:opacity-100"
              aria-label={`${dir.path} を削除`}
              title="削除"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
        ))}

      {/* Add new directory */}
      {!loading && (
        <div className="flex items-center gap-1 px-2 pt-1">
          <input
            type="text"
            value={newPath}
            onChange={(e) => setNewPath(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="ディレクトリパスを入力..."
            className="flex-1 min-w-0 text-xs px-2 py-1 rounded-md border border-border bg-background focus:outline-none focus:ring-1 focus:ring-primary/50 placeholder:text-muted-foreground/50"
            aria-label="新規ディレクトリパス"
          />
          <button
            onClick={handleAdd}
            disabled={!newPath.trim()}
            className="p-1 rounded-md hover:bg-primary/20 text-muted-foreground hover:text-primary transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            aria-label="ディレクトリを追加"
            title="追加"
          >
            <Plus className="w-3.5 h-3.5" />
          </button>
        </div>
      )}
    </div>
  );
}
