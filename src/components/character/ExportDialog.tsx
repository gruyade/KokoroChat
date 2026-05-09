import { useState } from 'react';
import { Download, X } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { useUIStore } from '../../stores';

interface ExportDialogProps {
  characterId: string;
  characterName: string;
  isOpen: boolean;
  onClose: () => void;
}

export function ExportDialog({ characterId, characterName, isOpen, onClose }: ExportDialogProps) {
  const [includeChats, setIncludeChats] = useState(true);
  const [includeThoughts, setIncludeThoughts] = useState(true);
  const [includeMemories, setIncludeMemories] = useState(true);
  const [exporting, setExporting] = useState(false);

  if (!isOpen) return null;

  const handleExport = async () => {
    const { showToast } = useUIStore.getState();
    if (exporting) return;
    setExporting(true);

    try {
      // 1. バックエンドからエクスポートデータ取得
      const data = await invoke('export_character', {
        characterId,
        options: {
          include_chats: includeChats,
          include_thoughts: includeThoughts,
          include_memories: includeMemories,
        },
      });

      // 2. ファイル保存ダイアログ表示
      const filePath = await save({
        defaultPath: `${characterName}_export.json`,
        filters: [{ name: 'JSON', extensions: ['json'] }],
      });

      if (!filePath) {
        // ユーザーがキャンセルした場合
        setExporting(false);
        return;
      }

      // 3. ファイルに書き込み
      await writeTextFile(filePath, JSON.stringify(data, null, 2));

      showToast('エクスポート完了');
      onClose();
    } catch (e) {
      const { showToast } = useUIStore.getState();
      showToast(`エクスポートに失敗: ${e}`, 'error');
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-md rounded-lg border border-border bg-card p-6 shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Download className="w-5 h-5 text-primary" />
            <h2 className="text-lg font-semibold">キャラクターエクスポート</h2>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
            aria-label="閉じる"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Character name */}
        <p className="text-sm text-muted-foreground mb-4">
          「{characterName}」のデータをエクスポート
        </p>

        {/* Options */}
        <div className="space-y-3 mb-6">
          <label className="flex items-center gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={includeChats}
              onChange={(e) => setIncludeChats(e.target.checked)}
              className="w-4 h-4 rounded border-border accent-primary"
            />
            <span className="text-sm">チャット履歴</span>
          </label>
          <label className="flex items-center gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={includeThoughts}
              onChange={(e) => setIncludeThoughts(e.target.checked)}
              className="w-4 h-4 rounded border-border accent-primary"
            />
            <span className="text-sm">思考</span>
          </label>
          <label className="flex items-center gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={includeMemories}
              onChange={(e) => setIncludeMemories(e.target.checked)}
              className="w-4 h-4 rounded border-border accent-primary"
            />
            <span className="text-sm">記憶</span>
          </label>
        </div>

        {/* Actions */}
        <div className="flex justify-end gap-2">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm rounded-md border border-border hover:bg-muted transition-colors"
          >
            キャンセル
          </button>
          <button
            onClick={handleExport}
            disabled={exporting}
            className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
          >
            {exporting ? (
              <>
                <span className="w-4 h-4 border-2 border-primary-foreground/30 border-t-primary-foreground rounded-full animate-spin" />
                エクスポート中...
              </>
            ) : (
              <>
                <Download className="w-4 h-4" />
                エクスポート
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
