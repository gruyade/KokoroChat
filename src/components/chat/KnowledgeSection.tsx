import { useEffect, useState, useCallback } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { Trash2, Download, Upload, Loader2 } from 'lucide-react';
import { useKnowledgeStore } from '../../stores/knowledge.store';
import { useUIStore } from '../../stores/ui.store';
import type { InjectionMode, KnowledgeEntryMeta } from '../../types';

/** 512KB = 524288 bytes */
const MAX_FILE_SIZE = 524288;

interface KnowledgeSectionProps {
  sessionId: string;
}

/** バイト数を人間可読フォーマットに変換 */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} bytes`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1048576).toFixed(1)} MB`;
}

/**
 * ToolManagementPane 内のナレッジ管理セクション。
 * DropZone によるファイル追加、一覧表示、有効/無効切替、
 * 注入モード変更、削除、エクスポートを提供する。
 */
export function KnowledgeSection({ sessionId }: KnowledgeSectionProps) {
  const {
    entries,
    loading,
    fetchEntries,
    addKnowledge,
    removeKnowledge,
    toggleKnowledge,
    setInjectionMode,
    exportKnowledge,
  } = useKnowledgeStore();
  const showToast = useUIStore((s) => s.showToast);

  const [dragOver, setDragOver] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  // マウント時およびsessionId変更時にエントリ取得
  useEffect(() => {
    fetchEntries(sessionId);
  }, [sessionId, fetchEntries]);

  // created_at 昇順でソート
  const sortedEntries = [...entries].sort(
    (a, b) => a.created_at.localeCompare(b.created_at)
  );

  // --- DropZone ハンドラ ---
  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragOver(false);
  }, []);

  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setDragOver(false);

      const files = Array.from(e.dataTransfer.files);
      for (const file of files) {
        // サイズチェック
        if (file.size > MAX_FILE_SIZE) {
          showToast(`${file.name}: ファイルサイズが上限(512KB)を超えている`, 'error');
          continue;
        }

        try {
          const content = await readFileAsUTF8(file);
          await addKnowledge(sessionId, file.name, content);
        } catch (err) {
          showToast(
            `${file.name}: ${err instanceof Error ? err.message : 'ファイルの読み取りに失敗'}`,
            'error'
          );
        }
      }
    },
    [sessionId, addKnowledge, showToast]
  );

  // --- 操作ハンドラ ---
  const handleToggle = async (entry: KnowledgeEntryMeta) => {
    try {
      await toggleKnowledge(sessionId, entry.file_name, !entry.enabled);
    } catch {
      showToast('有効/無効の切り替えに失敗', 'error');
    }
  };

  const handleInjectionModeChange = async (
    entry: KnowledgeEntryMeta,
    mode: InjectionMode
  ) => {
    try {
      await setInjectionMode(sessionId, entry.file_name, mode);
    } catch {
      showToast('注入モードの変更に失敗', 'error');
    }
  };

  const handleDeleteConfirm = async () => {
    if (!deleteTarget) return;
    try {
      await removeKnowledge(sessionId, deleteTarget);
    } catch {
      showToast('削除に失敗', 'error');
    }
    setDeleteTarget(null);
  };

  const handleExport = async (entry: KnowledgeEntryMeta) => {
    try {
      const content = await exportKnowledge(sessionId, entry.file_name);
      const filePath = await save({ defaultPath: entry.file_name });
      if (!filePath) return; // キャンセル時は何もしない
      await writeTextFile(filePath, content);
      showToast(`${entry.file_name} をエクスポート`, 'success');
    } catch (err) {
      showToast(
        `エクスポート失敗: ${err instanceof Error ? err.message : String(err)}`,
        'error'
      );
    }
  };

  return (
    <div className="space-y-2">
      {/* DropZone — data-drop-target="knowledge" でネイティブドロップ判定対象 */}
      <div
        data-drop-target="knowledge"
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        className={`flex items-center justify-center gap-1.5 px-3 py-3 border-2 border-dashed rounded-md transition-colors cursor-default ${
          dragOver
            ? 'border-primary bg-primary/10'
            : 'border-border/50 hover:border-muted-foreground/50'
        }`}
      >
        <Upload className="w-3.5 h-3.5 text-muted-foreground" />
        <span className="text-[10px] text-muted-foreground">
          ファイルをドロップして追加
        </span>
      </div>

      {/* Loading */}
      {loading && entries.length === 0 && (
        <div className="flex items-center justify-center py-3 text-muted-foreground text-xs gap-1.5">
          <Loader2 className="w-3.5 h-3.5 animate-spin" />
          <span>読み込み中...</span>
        </div>
      )}

      {/* 空状態プレースホルダ */}
      {!loading && entries.length === 0 && (
        <div className="text-[10px] text-muted-foreground px-2 py-2 text-center">
          ナレッジファイルがありません
        </div>
      )}

      {/* エントリ一覧 */}
      {sortedEntries.map((entry) => (
        <div
          key={entry.id}
          className={`flex items-center gap-1.5 px-2 py-1.5 rounded-md hover:bg-muted/40 group transition-opacity ${
            !entry.enabled ? 'opacity-50' : ''
          }`}
        >
          {/* ファイル名 + サイズ */}
          <div className="flex-1 min-w-0">
            <div className="text-xs font-mono truncate" title={entry.file_name}>
              {entry.file_name}
            </div>
            <div className="text-[10px] text-muted-foreground">
              {formatSize(entry.size_bytes)}
            </div>
          </div>

          {/* Enabled toggle */}
          <label className="flex items-center cursor-pointer" title={entry.enabled ? '無効にする' : '有効にする'}>
            <input
              type="checkbox"
              checked={entry.enabled}
              onChange={() => handleToggle(entry)}
              className="rounded text-primary focus:ring-primary/50 w-3 h-3"
              aria-label={`${entry.file_name} を${entry.enabled ? '無効' : '有効'}にする`}
            />
          </label>

          {/* Injection mode select */}
          <select
            value={entry.injection_mode}
            onChange={(e) =>
              handleInjectionModeChange(entry, e.target.value as InjectionMode)
            }
            className="text-[10px] bg-background border border-border rounded px-1 py-0.5 focus:outline-none focus:ring-1 focus:ring-primary/50"
            aria-label={`${entry.file_name} の注入モード`}
          >
            <option value="system_prompt">system_prompt</option>
            <option value="tool_reference">tool_reference</option>
          </select>

          {/* Export button */}
          <button
            onClick={() => handleExport(entry)}
            className="p-0.5 rounded hover:bg-muted/60 text-muted-foreground hover:text-foreground transition-colors opacity-0 group-hover:opacity-100"
            aria-label={`${entry.file_name} をエクスポート`}
            title="エクスポート"
          >
            <Download className="w-3 h-3" />
          </button>

          {/* Delete button */}
          <button
            onClick={() => setDeleteTarget(entry.file_name)}
            className="p-0.5 rounded hover:bg-destructive/20 text-muted-foreground hover:text-destructive transition-colors opacity-0 group-hover:opacity-100"
            aria-label={`${entry.file_name} を削除`}
            title="削除"
          >
            <Trash2 className="w-3 h-3" />
          </button>
        </div>
      ))}

      {/* 削除確認ダイアログ */}
      {deleteTarget && (
        <div className="px-2 py-2 border border-destructive/50 bg-destructive/10 rounded-md space-y-2">
          <p className="text-xs text-destructive">
            「{deleteTarget}」を削除する？
          </p>
          <div className="flex items-center gap-2">
            <button
              onClick={handleDeleteConfirm}
              className="text-[10px] px-2 py-0.5 rounded bg-destructive text-destructive-foreground hover:bg-destructive/90 transition-colors"
            >
              削除
            </button>
            <button
              onClick={() => setDeleteTarget(null)}
              className="text-[10px] px-2 py-0.5 rounded border border-border hover:bg-muted/50 transition-colors"
            >
              キャンセル
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/** ファイルをUTF-8テキストとして読み取る */
function readFileAsUTF8(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        resolve(reader.result);
      } else {
        reject(new Error('UTF-8テキストとして読み取れない'));
      }
    };
    reader.onerror = () => {
      reject(new Error('ファイルの読み取りに失敗'));
    };
    reader.readAsText(file, 'UTF-8');
  });
}
