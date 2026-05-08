import { describe, it, expect, beforeEach, vi } from 'vitest';
import * as fc from 'fast-check';

/**
 * オペレーションキューのプロパティテスト
 *
 * Property 3: キュー順序保証と障害耐性
 * Property 5: キュー状態の正確性
 *
 * Feature: app-enhancements-v2
 */

// テスト間でストアを再生成するため動的インポートを使用
async function createFreshStore() {
  // vi.resetModules() + dynamic import でモジュールスコープのクロージャをリセット
  vi.resetModules();
  const mod = await import('./operation-queue');
  return mod.useOperationQueue;
}

describe('OperationQueue - Property 3: キュー順序保証と障害耐性', () => {
  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  /**
   * Property 3: キュー順序保証と障害耐性
   *
   * For any タスク列（一部が例外をスローするタスクを含む）に対して、
   * オペレーションキューは失敗しなかったタスクを追加順序通りに実行し、
   * 失敗タスクの後も後続タスクの実行を継続する。
   *
   * **Validates: Requirements 4.3, 4.4**
   */
  it('property: 成功タスクは追加順序通りに実行され、失敗タスクの後も後続タスクが継続する', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.array(
          fc.record({
            shouldFail: fc.boolean(),
            index: fc.nat(),
          }),
          { minLength: 1, maxLength: 20 }
        ),
        async (taskDefs) => {
          const useOperationQueue = await createFreshStore();

          const executionOrder: number[] = [];

          // 全タスクの完了を追跡するPromise
          let allDone: () => void;
          const allDonePromise = new Promise<void>((r) => (allDone = r));
          let completedCount = 0;

          // 全タスクをキューに追加
          for (let i = 0; i < taskDefs.length; i++) {
            const idx = i;
            const def = taskDefs[i];

            useOperationQueue.getState().enqueue(async () => {
              executionOrder.push(idx);
              completedCount++;
              if (completedCount === taskDefs.length) {
                allDone();
              }
              if (def.shouldFail) {
                throw new Error(`Task ${idx} failed`);
              }
            }, `task-${idx}`);
          }

          // 全タスクの完了を待つ
          await allDonePromise;
          // processQueue の while ループ完了を待つ
          await new Promise((resolve) => setTimeout(resolve, 0));

          // 検証1: 全タスク（成功・失敗問わず）が追加順序通りに実行された
          const expectedOrder = taskDefs.map((_, i) => i);
          expect(executionOrder).toEqual(expectedOrder);

          // 検証2: 失敗タスクの後も後続タスクが実行された
          // （全タスクが実行されていることで証明）
          expect(executionOrder.length).toBe(taskDefs.length);
        }
      ),
      { numRuns: 100 }
    );
  }, 30000);
});

describe('OperationQueue - Property 5: キュー状態の正確性', () => {
  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  /**
   * Property 5: キュー状態の正確性
   *
   * For any タスク追加・完了のシーケンスに対して、
   * `pendingCount`は未実行タスク数と一致し、
   * `processing`はキューにタスクが存在する間trueである。
   *
   * **Validates: Requirements 4.5**
   *
   * 実装の動作:
   * - enqueue時: pendingCount = queue.length（push後）
   * - processQueue内: queue.shift()後に pendingCount = queue.length を設定してからexecute
   * - 最初のenqueueでprocessQueueが即座に開始されるため、最初のタスクは
   *   他のタスクがenqueueされる前にshiftされる
   * - テストでは全タスクenqueue後の安定状態で検証するため、
   *   最初のタスクをゲートとして使い、全enqueue完了後に処理を開始させる
   */
  it('property: pendingCountは未実行タスク数と一致し、processingはキュー処理中trueである', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.array(
          fc.record({
            shouldFail: fc.boolean(),
          }),
          { minLength: 1, maxLength: 15 }
        ),
        async (taskDefs) => {
          const useOperationQueue = await createFreshStore();

          const taskResolvers: Array<() => void> = [];
          const taskStartedPromises: Array<Promise<void>> = [];
          const pendingSnapshots: Array<{ taskIndex: number; pendingCount: number; processing: boolean }> = [];

          // ゲートタスク: 全タスクenqueue完了後に解放し、キュー処理を開始させる
          let gateResolve!: () => void;
          const gatePromise = new Promise<void>((r) => (gateResolve = r));
          let gateStarted!: () => void;
          const gateStartedPromise = new Promise<void>((r) => (gateStarted = r));

          useOperationQueue.getState().enqueue(async () => {
            gateStarted();
            await gatePromise;
          }, 'gate');

          // ゲートタスクの開始を待つ（processQueueがゲートのawaitで停止）
          await gateStartedPromise;

          // ゲートタスクがawaitしている間に全タスクをenqueue
          for (let i = 0; i < taskDefs.length; i++) {
            let startResolve!: () => void;
            taskStartedPromises.push(new Promise<void>((r) => (startResolve = r)));

            let completeResolve!: () => void;
            const completePromise = new Promise<void>((r) => (completeResolve = r));
            taskResolvers.push(completeResolve);

            const idx = i;
            const def = taskDefs[i];

            useOperationQueue.getState().enqueue(async () => {
              startResolve();
              // タスク実行中のスナップショット取得
              pendingSnapshots.push({
                taskIndex: idx,
                pendingCount: useOperationQueue.getState().pendingCount,
                processing: useOperationQueue.getState().processing,
              });
              await completePromise;
              if (def.shouldFail) {
                throw new Error('fail');
              }
            }, `task-${i}`);
          }

          // 全タスクenqueue完了。ゲートを解放して処理開始
          // この時点でqueue = [t0, t1, ..., tn]（ゲートは既にshift済み）
          expect(useOperationQueue.getState().processing).toBe(true);

          // ゲート解放
          gateResolve();

          // 各タスクを順番に完了させる
          for (let i = 0; i < taskDefs.length; i++) {
            await taskStartedPromises[i];

            // タスク実行中: processingはtrue
            expect(useOperationQueue.getState().processing).toBe(true);

            taskResolvers[i]();

            if (i < taskDefs.length - 1) {
              // 次のタスクの開始を待つ
              await taskStartedPromises[i + 1];
            } else {
              // 最後のタスク完了後、processQueueのwhileループ終了を待つ
              await new Promise((resolve) => setTimeout(resolve, 0));
            }
          }

          // 全タスク完了後: processing=false, pendingCount=0
          const finalState = useOperationQueue.getState();
          expect(finalState.processing).toBe(false);
          expect(finalState.pendingCount).toBe(0);

          // 各タスク実行中のスナップショット検証
          for (const snap of pendingSnapshots) {
            // タスク実行中はprocessingがtrue
            expect(snap.processing).toBe(true);
            // pendingCountは残りのキュー内タスク数（現在実行中のタスクは含まない）
            // ゲートタスク後にenqueueされたタスクが taskDefs.length 個
            // taskIndex番目のタスク実行時、残りは taskDefs.length - taskIndex - 1 個
            const expectedPending = taskDefs.length - snap.taskIndex - 1;
            expect(snap.pendingCount).toBe(expectedPending);
          }
        }
      ),
      { numRuns: 100 }
    );
  }, 60000);
});
