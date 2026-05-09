import { useEffect, useState } from 'react';
import { Upload, X, FileText, AlertCircle } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { readTextFile } from '@tauri-apps/plugin-fs';
import { useUIStore } from '../../stores';

interface ImportDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onImported: () => void;
}

interface ParsedData {
  version: number;
  exported_at: string;
  character: {
    name: string;
    description: string;
    system_prompt: string;
    tts_config?: unknown;
  };
  chat_sessions?: unknown[];
  thoughts?: unknown[];
  memories?: unknown[];
}

type ImportStep = 'idle' | 'selecting' | 'options' | 'importing';

export function ImportDialog({ isOpen, onClose, onImported }: ImportDialogProps) {
  const [step, setStep] = useState<ImportStep>('idle');
  const [parsedData, setParsedData] = useState<ParsedData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [includeChats, setIncludeChats] = useState(true);
  const [includeThoughts, setIncludeThoughts] = useState(true);
  const [includeMemories, setIncludeMemories] = useState(true);
  const [importing, setImporting] = useState(false);

  // ダイアログが開いたらファイル選択を開始
  useEffect(() => {
    if (isOpen && step === 'idle') {
      handleSelectFile();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen]);

  // ダイアログが閉じたらリセット
  useEffect(() => {
    if (!isOpen) {
      setStep('idle');
      setParsedData(null);
      setError(null);
      setIncludeChats(true);
      setIncludeThoughts(true);
      setIncludeMemories(true);
      setImporting(false);
    }
  }, [isOpen]);

  const handleSelectFile = async () => {
    setStep('selecting');
    setError(null);

    try {
      const filePath = await open({
        filters: [{ name: 'JSON', extensions: ['json'] }],
        multiple: false,
      });

      if (!filePath) {
        // ユーザーがキャンセル
        onClose();
        return;
      }

      // ファイル読み込み
      const content = await readTextFile(filePath as string);

      // JSON解析
      let data: unknown;
      try {
        data = JSON.parse(content);
      } catch {
        setError('ファイル形式が不正: JSONとして解析できない');
        setStep('options');
        return;
      }

      // バリデーション
      const validationError = validateImportData(data);
      if (validationError) {
        setError(validationError);
        setStep('options');
        return;
      }

      setParsedData(data as ParsedData);
      setStep('options');
    } catch (e) {
      setError(`ファイルの読み込みに失敗: ${e}`);
      setStep('options');
    }
  };

  const handleImport = async () => {
    if (!parsedData || importing) return;
    setImporting(true);
    setError(null);

    try {
      await invoke('import_character', {
        data: parsedData,
        options: {
          include_chats: includeChats,
          include_thoughts: includeThoughts,
          include_memories: includeMemories,
        },
      });

      const { showToast } = useUIStore.getState();
      showToast('インポート完了');
      onImported();
      onClose();
    } catch (e) {
      setError(`インポートに失敗: ${e}`);
    } finally {
      setImporting(false);
    }
  };

  if (!isOpen) return null;

  // ファイル選択中はモーダルのみ表示
  if (step === 'selecting') {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
        <div className="w-full max-w-md rounded-lg border border-border bg-card p-6 shadow-xl">
          <div className="flex items-center justify-center gap-2 text-muted-foreground">
            <FileText className="w-5 h-5 animate-pulse" />
            <span className="text-sm">ファイルを選択中...</span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-md rounded-lg border border-border bg-card p-6 shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Upload className="w-5 h-5 text-primary" />
            <h2 className="text-lg font-semibold">キャラクターインポート</h2>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
            aria-label="閉じる"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Error */}
        {error && (
          <div className="mb-4 p-3 rounded-md bg-destructive/10 border border-destructive/20 flex items-start gap-2">
            <AlertCircle className="w-4 h-4 text-destructive shrink-0 mt-0.5" />
            <p className="text-sm text-destructive">{error}</p>
          </div>
        )}

        {/* Content */}
        {parsedData ? (
          <>
            {/* Character name */}
            <p className="text-sm text-muted-foreground mb-4">
              「{parsedData.character.name}」をインポート
            </p>

            {/* Options */}
            <div className="space-y-3 mb-6">
              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={includeChats}
                  onChange={(e) => setIncludeChats(e.target.checked)}
                  disabled={!parsedData.chat_sessions?.length}
                  className="w-4 h-4 rounded border-border accent-primary"
                />
                <span className={`text-sm ${!parsedData.chat_sessions?.length ? 'text-muted-foreground' : ''}`}>
                  チャット履歴
                  {parsedData.chat_sessions?.length
                    ? ` (${parsedData.chat_sessions.length}件)`
                    : ' (データなし)'}
                </span>
              </label>
              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={includeThoughts}
                  onChange={(e) => setIncludeThoughts(e.target.checked)}
                  disabled={!parsedData.thoughts?.length}
                  className="w-4 h-4 rounded border-border accent-primary"
                />
                <span className={`text-sm ${!parsedData.thoughts?.length ? 'text-muted-foreground' : ''}`}>
                  思考
                  {parsedData.thoughts?.length
                    ? ` (${parsedData.thoughts.length}件)`
                    : ' (データなし)'}
                </span>
              </label>
              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={includeMemories}
                  onChange={(e) => setIncludeMemories(e.target.checked)}
                  disabled={!parsedData.memories?.length}
                  className="w-4 h-4 rounded border-border accent-primary"
                />
                <span className={`text-sm ${!parsedData.memories?.length ? 'text-muted-foreground' : ''}`}>
                  記憶
                  {parsedData.memories?.length
                    ? ` (${parsedData.memories.length}件)`
                    : ' (データなし)'}
                </span>
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
                onClick={handleImport}
                disabled={importing}
                className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
              >
                {importing ? (
                  <>
                    <span className="w-4 h-4 border-2 border-primary-foreground/30 border-t-primary-foreground rounded-full animate-spin" />
                    インポート中...
                  </>
                ) : (
                  <>
                    <Upload className="w-4 h-4" />
                    インポート
                  </>
                )}
              </button>
            </div>
          </>
        ) : (
          /* エラーのみ表示（parsedDataがない場合） */
          <div className="flex justify-end gap-2">
            <button
              onClick={onClose}
              className="px-4 py-2 text-sm rounded-md border border-border hover:bg-muted transition-colors"
            >
              閉じる
            </button>
            <button
              onClick={handleSelectFile}
              className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors flex items-center gap-2"
            >
              <FileText className="w-4 h-4" />
              別のファイルを選択
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function validateImportData(data: unknown): string | null {
  if (!data || typeof data !== 'object') {
    return '不正なデータ形式';
  }

  const obj = data as Record<string, unknown>;

  if (obj.version === undefined) {
    return '必須データが不足: version';
  }

  if (typeof obj.version !== 'number' || obj.version !== 1) {
    return `未対応のエクスポート形式（version: ${obj.version}）`;
  }

  if (!obj.character || typeof obj.character !== 'object') {
    return '必須データが不足: character';
  }

  const character = obj.character as Record<string, unknown>;

  if (!character.name || typeof character.name !== 'string') {
    return '必須データが不足: character.name';
  }

  if (character.description === undefined || typeof character.description !== 'string') {
    return '必須データが不足: character.description';
  }

  if (character.system_prompt === undefined || typeof character.system_prompt !== 'string') {
    return '必須データが不足: character.system_prompt';
  }

  return null;
}
