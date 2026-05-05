import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { Attachment } from '../types';

/**
 * ファイル添付操作Hook
 * - Tauriダイアログによるファイル選択
 * - Attachment Processorによるファイル処理
 */
export function useAttachment() {
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [error, setError] = useState<string | null>(null);

  const addAttachment = useCallback(async () => {
    setError(null);
    try {
      const filePath = await open({
        multiple: false,
        filters: [
          {
            name: 'Supported Files',
            extensions: ['txt', 'md', 'csv', 'pdf', 'png', 'jpg', 'jpeg', 'webp'],
          },
        ],
      });

      if (!filePath) return;
      const attachment = await invoke<Attachment>('process_attachment', { filePath });
      setAttachments((prev) => [...prev, attachment]);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const removeAttachment = useCallback((id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id));
  }, []);

  const clearAttachments = useCallback(() => {
    setAttachments([]);
  }, []);

  return { attachments, error, addAttachment, removeAttachment, clearAttachments };
}
