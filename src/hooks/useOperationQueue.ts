import { useCallback, useRef, useState } from 'react';

/**
 * 非同期操作をキューイングして順次実行するフック。
 * 連続した削除・更新操作がバックエンドと確実に同期するよう保証する。
 *
 * - pendingIds: 現在キューに入っている（処理待ち or 処理中）アイテムIDのSet
 * - processing: キューに1件以上のタスクが残っているか
 * - enqueue: 操作をキューに追加
 */
export function useOperationQueue() {
  const [pendingIds, setPendingIds] = useState<Set<string>>(new Set());
  const queueRef = useRef<Array<{ id: string; task: () => Promise<void> }>>([]);
  const runningRef = useRef(false);

  const processQueue = useCallback(async () => {
    if (runningRef.current) return;
    runningRef.current = true;

    while (queueRef.current.length > 0) {
      const item = queueRef.current.shift()!;
      try {
        await item.task();
      } catch {
        // エラーは各タスク内でハンドリング済み想定
      }
      setPendingIds((prev) => {
        const next = new Set(prev);
        next.delete(item.id);
        return next;
      });
    }

    runningRef.current = false;
  }, []);

  const enqueue = useCallback(
    (id: string, task: () => Promise<void>) => {
      queueRef.current.push({ id, task });
      setPendingIds((prev) => new Set(prev).add(id));
      processQueue();
    },
    [processQueue]
  );

  const processing = pendingIds.size > 0;

  return { pendingIds, processing, enqueue };
}
