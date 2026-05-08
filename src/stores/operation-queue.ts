import { create } from 'zustand';

interface QueueTask {
  id: string;
  execute: () => Promise<void>;
  label?: string;
}

interface OperationQueueState {
  pendingCount: number;
  processing: boolean;
  currentTaskLabel: string | null;
  enqueue: (task: () => Promise<void>, label?: string) => void;
}

export const useOperationQueue = create<OperationQueueState>((set) => {
  const queue: QueueTask[] = [];
  let running = false;

  const processQueue = async () => {
    if (running) return;
    running = true;
    set({ processing: true });

    while (queue.length > 0) {
      const task = queue.shift()!;
      set({ currentTaskLabel: task.label ?? null, pendingCount: queue.length });
      try {
        await task.execute();
      } catch (e) {
        console.error('[OperationQueue] Task failed:', task.label, e);
      }
    }

    running = false;
    set({ processing: false, currentTaskLabel: null, pendingCount: 0 });
  };

  return {
    pendingCount: 0,
    processing: false,
    currentTaskLabel: null,
    enqueue: (execute, label) => {
      queue.push({ id: crypto.randomUUID(), execute, label });
      set({ pendingCount: queue.length });
      processQueue();
    },
  };
});
