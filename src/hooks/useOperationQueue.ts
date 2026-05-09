import { useCallback, useRef, useState } from 'react';

/**
 * 非同期操作をキューイングして順次実行するフック。
 *
 * UIへの即時反映は呼び出し側で deletedIds 等を使って行う。
 * バックエンド操作はキューで順次処理される。
 *
 * - processing: キューに1件以上のタスクが残っているか
 * - enqueue: 操作をキューに追加
 */
export function useOperationQueue() {
  const [processing, setProcessing] = useState(false);
  const queueRef = useRef<Array<() => Promise<void>>>([]);
  const runningRef = useRef(false);

  const processQueue = useCallback(async () => {
    if (runningRef.current) return;
    runningRef.current = true;
    setProcessing(true);

    while (queueRef.current.length > 0) {
      const task = queueRef.current.shift()!;
      try {
        await task();
      } catch {
        // エラーは各タスク内でハンドリング済み想定
      }
    }

    runningRef.current = false;
    setProcessing(false);
  }, []);

  const enqueue = useCallback(
    (task: () => Promise<void>) => {
      queueRef.current.push(task);
      processQueue();
    },
    [processQueue]
  );

  return { processing, enqueue };
}
