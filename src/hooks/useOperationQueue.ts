import { useCallback, useRef, useState } from 'react';

/**
 * 非同期操作をキューイングして順次実行するフック。
 *
 * UIへの即時反映は行わず、バックグラウンドで順次処理する。
 * コンポーネント再マウント時（画面切り替え後）にデータ再取得すれば
 * 削除済みアイテムは自然に消える。
 *
 * - processing: キューに1件以上のタスクが残っているか（処理中表示用）
 * - enqueue: 操作をキューに追加
 * - onComplete: 全タスク完了時に呼ばれるコールバックを設定
 */
export function useOperationQueue(onAllComplete?: () => void) {
  const [processing, setProcessing] = useState(false);
  const queueRef = useRef<Array<() => Promise<void>>>([]);
  const runningRef = useRef(false);
  const onAllCompleteRef = useRef(onAllComplete);
  onAllCompleteRef.current = onAllComplete;

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
    onAllCompleteRef.current?.();
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
