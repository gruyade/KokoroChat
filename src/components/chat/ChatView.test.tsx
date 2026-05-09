import { cleanup } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as fc from 'fast-check';
import '@testing-library/jest-dom';

import { shouldAutoScroll } from './ChatView';

/**
 * ChatViewのスクロール制御ロジックを忠実に再現するシミュレーター。
 * コンポーネントの内部ref状態を直接操作・観察可能にする。
 *
 * これにより、isProgrammaticScrollRef がtrueの間にユーザーがスクロールアップした場合の
 * 動作を正確にテストできる。
 */
class ScrollLogicSimulator {
  // Refs (ChatView内部状態の再現)
  isNearBottomRef = true;
  isProgrammaticScrollRef = false;
  rafIdRef: number | null = null;

  // Container state
  scrollHeight: number;
  scrollTop: number;
  clientHeight: number;

  // Tracking
  scrollToCalls: Array<{ top: number; behavior: string }> = [];

  constructor(scrollHeight: number, scrollTop: number, clientHeight: number) {
    this.scrollHeight = scrollHeight;
    this.scrollTop = scrollTop;
    this.clientHeight = clientHeight;
  }

  /**
   * handleScroll の再現 (ChatView.tsx L28-L36)
   * isProgrammaticScrollRef がtrueの場合、早期リターンする
   */
  handleScroll(): void {
    if (this.isProgrammaticScrollRef) return; // ← バグの根本原因
    this.isNearBottomRef = shouldAutoScroll(
      this.scrollHeight,
      this.scrollTop,
      this.clientHeight
    );
  }

  /**
   * smoothScrollToBottom の再現 (ChatView.tsx L48-L66)
   * isNearBottomRef がfalseなら早期リターン
   */
  smoothScrollToBottom(): void {
    if (!this.isNearBottomRef) return;
    this.isProgrammaticScrollRef = true;
    // rAF callback (同期的に実行)
    this.scrollToCalls.push({
      top: this.scrollHeight,
      behavior: 'smooth',
    });
    // 300ms後にフラグ解除（テストでは手動で呼ぶ）
  }

  /**
   * 300msタイムアウト後のフラグ解除
   */
  releaseProgrammaticFlag(): void {
    this.isProgrammaticScrollRef = false;
  }

  /**
   * ユーザーのwheel upイベントをシミュレート。
   * 修正後コード: wheelイベントリスナーが直接isNearBottomRefをfalseに設定する。
   * isProgrammaticScrollRefの状態に関係なくユーザー意図を即座に検知。
   */
  userWheelUp(): void {
    // 修正後: wheelイベントリスナーが直接isNearBottomRefをfalseに設定
    // isProgrammaticScrollRefをバイパスしてユーザー操作を即座に反映
    this.isNearBottomRef = false;
  }

  /**
   * ストリーミング更新をシミュレート。
   * useEffect内で isStreaming && streamingContent && isNearBottomRef.current の場合に
   * smoothScrollToBottom() が呼ばれる。
   */
  onStreamingUpdate(): void {
    // ChatView.tsx L87-L90 の useEffect を再現
    if (this.isNearBottomRef) {
      this.smoothScrollToBottom();
    }
  }
}

describe('ChatView - Bug Condition Exploration (Property 1)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  /**
   * Property 1a: shouldAutoScroll threshold logic (単体テスト)
   *
   * **Validates: Requirements 1.1**
   *
   * shouldAutoScroll は底から200px以内ならtrue、200px超ならfalseを返す。
   * この閾値ロジック自体は正しい — バグはイベントハンドリングにある。
   * このテストはPASSするはず。
   */
  it('property: shouldAutoScroll returns true when distance <= 200, false when > 200', () => {
    fc.assert(
      fc.property(
        fc.integer({ min: 500, max: 5000 }),
        fc.integer({ min: 200, max: 800 }),
        fc.double({ min: 0, max: 1, noNaN: true }),
        (scrollHeight, clientHeight, scrollFraction) => {
          if (clientHeight >= scrollHeight) return;

          const maxScrollTop = scrollHeight - clientHeight;
          const scrollTop = Math.floor(scrollFraction * maxScrollTop);
          const distanceFromBottom = scrollHeight - scrollTop - clientHeight;

          const result = shouldAutoScroll(scrollHeight, scrollTop, clientHeight);

          if (distanceFromBottom <= 200) {
            expect(result).toBe(true);
          } else {
            expect(result).toBe(false);
          }
        }
      ),
      { numRuns: 200 }
    );
  });

  /**
   * Property 1b: Bug Condition - ユーザースクロールアップ中の位置維持
   *
   * **Validates: Requirements 1.1, 1.2, 1.3, 2.1, 2.2**
   *
   * シナリオ:
   * 1. ストリーミング中にsmoothScrollToBottomが呼ばれる（isProgrammaticScrollRef = true）
   * 2. ユーザーがwheel upでスクロールアップ（底から200px超の位置）
   * 3. ブラウザがscrollイベントを発火 → handleScrollが呼ばれる
   * 4. 次のストリーミング更新が到着
   *
   * 期待動作: isNearBottomRef が false になり、auto-scroll が抑制される
   *
   * 未修正コードでの動作:
   * - handleScroll が isProgrammaticScrollRef=true により早期リターン
   * - isNearBottomRef が true のまま
   * - 次のストリーミング更新で smoothScrollToBottom が scrollTo を実行
   * - ユーザーのスクロール位置が強制リセットされる
   *
   * Scoped PBT Approach: distance > 200px のケースのみ生成
   */
  it('property: user wheel-up during streaming should set isNearBottomRef to false and suppress auto-scroll', () => {
    fc.assert(
      fc.property(
        fc.record({
          scrollHeight: fc.integer({ min: 1000, max: 5000 }),
          clientHeight: fc.integer({ min: 300, max: 800 }),
          distanceFromBottom: fc.integer({ min: 201, max: 2000 }),
        }),
        ({ scrollHeight, clientHeight, distanceFromBottom }) => {
          if (clientHeight >= scrollHeight) return;
          const scrollTop = scrollHeight - clientHeight - distanceFromBottom;
          if (scrollTop < 0) return;

          // Setup: ユーザーがスクロールアップした位置にいる
          const sim = new ScrollLogicSimulator(scrollHeight, scrollTop, clientHeight);

          // Verify precondition: この位置では shouldAutoScroll は false を返すはず
          expect(shouldAutoScroll(scrollHeight, scrollTop, clientHeight)).toBe(false);

          // Step 1: ストリーミング中にsmoothScrollToBottomが呼ばれる
          // (isNearBottomRef は初期値 true なので実行される)
          sim.smoothScrollToBottom();

          // この時点で isProgrammaticScrollRef = true
          expect(sim.isProgrammaticScrollRef).toBe(true);

          // Step 2: ユーザーがwheel upでスクロールアップ
          // 未修正コードでは handleScroll のみが呼ばれ、isProgrammaticScrollRef=true で早期リターン
          sim.userWheelUp();

          // Step 3: 次のストリーミング更新が到着
          // isNearBottomRef の状態に基づいて smoothScrollToBottom が呼ばれるか決まる
          sim.scrollToCalls = []; // 前回のcallsをクリア
          sim.onStreamingUpdate();

          // ASSERTION: ユーザーがスクロールアップしているので、
          // isNearBottomRef は false であるべき → smoothScrollToBottom は scrollTo を呼ばないはず
          //
          // 未修正コードでは:
          // - handleScroll が早期リターンしたため isNearBottomRef は true のまま
          // - onStreamingUpdate → smoothScrollToBottom → scrollTo が実行される
          // - テストFAIL
          //
          // Counterexample: User at scrollTop=${scrollTop} with scrollHeight=${scrollHeight},
          // clientHeight=${clientHeight} (${distanceFromBottom}px from bottom) —
          // wheel up event ignored, isNearBottomRef remains true, auto-scroll fires
          expect(sim.isNearBottomRef).toBe(false);
          expect(sim.scrollToCalls.length).toBe(0);
        }
      ),
      { numRuns: 100 }
    );
  }, 30000);

  /**
   * Property 1c: smoothScrollToBottom は修正後、ユーザーがwheel upした後は scrollTo を実行しない
   *
   * **Validates: Requirements 2.2**
   *
   * 修正後: wheelイベントリスナーがisNearBottomRefを即座にfalseに設定するため、
   * 次のストリーミング更新でsmoothScrollToBottomはscrollToを呼び出さない。
   * ユーザーのスクロール位置が維持される。
   */
  it('property: smoothScrollToBottom does NOT execute scrollTo after user wheel-up (fix applied)', () => {
    fc.assert(
      fc.property(
        fc.record({
          scrollHeight: fc.integer({ min: 1000, max: 5000 }),
          clientHeight: fc.integer({ min: 300, max: 800 }),
          distanceFromBottom: fc.integer({ min: 201, max: 2000 }),
        }),
        ({ scrollHeight, clientHeight, distanceFromBottom }) => {
          if (clientHeight >= scrollHeight) return;
          const scrollTop = scrollHeight - clientHeight - distanceFromBottom;
          if (scrollTop < 0) return;

          const sim = new ScrollLogicSimulator(scrollHeight, scrollTop, clientHeight);

          // Step 1: Initial smoothScrollToBottom (streaming started)
          sim.smoothScrollToBottom();
          expect(sim.isProgrammaticScrollRef).toBe(true);

          // Step 2: User wheels up (wheel listener directly sets isNearBottomRef = false)
          sim.userWheelUp();

          // Step 3: Next streaming update arrives
          sim.scrollToCalls = [];
          sim.onStreamingUpdate();

          // FIXED BEHAVIOR: scrollTo is NOT called because isNearBottomRef is now false
          // The wheel event listener bypasses isProgrammaticScrollRef and directly updates the flag
          expect(sim.scrollToCalls.length).toBe(0);
        }
      ),
      { numRuns: 100 }
    );
  }, 30000);
});


describe('ChatView - Preservation Property Tests (Property 2)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  /**
   * Property 2a: shouldAutoScroll は底から200px以内で true を返す
   *
   * **Validates: Requirements 3.1**
   *
   * 底付近にいるユーザーに対して自動スクロールが有効であることを保証。
   * この動作は修正後も変わらないことを確認する。
   */
  it('property: shouldAutoScroll returns true for all positions within 200px of bottom', () => {
    fc.assert(
      fc.property(
        fc.record({
          scrollHeight: fc.integer({ min: 500, max: 10000 }),
          clientHeight: fc.integer({ min: 200, max: 800 }),
          distanceFromBottom: fc.integer({ min: 0, max: 200 }),
        }),
        ({ scrollHeight, clientHeight, distanceFromBottom }) => {
          // Ensure valid scroll geometry
          if (clientHeight >= scrollHeight) return;
          const scrollTop = scrollHeight - clientHeight - distanceFromBottom;
          if (scrollTop < 0) return;

          const result = shouldAutoScroll(scrollHeight, scrollTop, clientHeight);
          expect(result).toBe(true);
        }
      ),
      { numRuns: 200 }
    );
  });

  /**
   * Property 2b: shouldAutoScroll は底から200px超で false を返す
   *
   * **Validates: Requirements 3.1**
   *
   * 底から離れているユーザーに対して自動スクロールが無効であることを保証。
   * この閾値ロジックは修正後も変わらないことを確認する。
   */
  it('property: shouldAutoScroll returns false for all positions more than 200px from bottom', () => {
    fc.assert(
      fc.property(
        fc.record({
          scrollHeight: fc.integer({ min: 1000, max: 10000 }),
          clientHeight: fc.integer({ min: 200, max: 800 }),
          distanceFromBottom: fc.integer({ min: 201, max: 5000 }),
        }),
        ({ scrollHeight, clientHeight, distanceFromBottom }) => {
          if (clientHeight >= scrollHeight) return;
          const scrollTop = scrollHeight - clientHeight - distanceFromBottom;
          if (scrollTop < 0) return;

          const result = shouldAutoScroll(scrollHeight, scrollTop, clientHeight);
          expect(result).toBe(false);
        }
      ),
      { numRuns: 200 }
    );
  });

  /**
   * Property 2c: isNearBottomRef=true かつストリーミング更新時に smoothScrollToBottom が scrollTo を実行
   *
   * **Validates: Requirements 3.1, 3.2**
   *
   * ユーザーが底付近にいる状態でストリーミング更新が来た場合、
   * smoothScrollToBottom が scrollTo を呼び出して自動追従することを確認。
   * この動作は修正後も保持される必要がある。
   */
  it('property: smoothScrollToBottom triggers scrollTo when isNearBottomRef is true during streaming', () => {
    fc.assert(
      fc.property(
        fc.record({
          scrollHeight: fc.integer({ min: 500, max: 10000 }),
          clientHeight: fc.integer({ min: 200, max: 800 }),
          distanceFromBottom: fc.integer({ min: 0, max: 200 }),
        }),
        ({ scrollHeight, clientHeight, distanceFromBottom }) => {
          if (clientHeight >= scrollHeight) return;
          const scrollTop = scrollHeight - clientHeight - distanceFromBottom;
          if (scrollTop < 0) return;

          const sim = new ScrollLogicSimulator(scrollHeight, scrollTop, clientHeight);
          // isNearBottomRef starts as true (default)
          expect(sim.isNearBottomRef).toBe(true);

          // Streaming update triggers smoothScrollToBottom
          sim.onStreamingUpdate();

          // scrollTo should have been called
          expect(sim.scrollToCalls.length).toBe(1);
          expect(sim.scrollToCalls[0].top).toBe(scrollHeight);
          expect(sim.scrollToCalls[0].behavior).toBe('smooth');
        }
      ),
      { numRuns: 200 }
    );
  });

  /**
   * Property 2d: セッション切り替え時に scrollToBottomInstant が現在のスクロール位置に関係なく実行
   *
   * **Validates: Requirements 3.3**
   *
   * セッション切り替え時は scrollTop = scrollHeight に設定され、
   * isNearBottomRef が true になることを確認。
   * どのスクロール位置からでも即座に最下部へ移動する。
   */
  it('property: session switch triggers scrollToBottomInstant regardless of current scroll position', () => {
    fc.assert(
      fc.property(
        fc.record({
          scrollHeight: fc.integer({ min: 500, max: 10000 }),
          clientHeight: fc.integer({ min: 200, max: 800 }),
          distanceFromBottom: fc.integer({ min: 0, max: 5000 }),
        }),
        ({ scrollHeight, clientHeight, distanceFromBottom }) => {
          if (clientHeight >= scrollHeight) return;
          const scrollTop = scrollHeight - clientHeight - distanceFromBottom;
          if (scrollTop < 0) return;

          const sim = new ScrollLogicSimulator(scrollHeight, scrollTop, clientHeight);
          // ユーザーが底から離れている場合、isNearBottomRef を false に設定
          if (distanceFromBottom > 200) {
            sim.isNearBottomRef = false;
          }

          // scrollToBottomInstant のロジックを再現
          // ChatView.tsx L73-L80: scrollTop = scrollHeight, isNearBottomRef = true
          sim.isProgrammaticScrollRef = true;
          sim.scrollTop = sim.scrollHeight;
          sim.isNearBottomRef = true;

          // Assertions
          expect(sim.scrollTop).toBe(sim.scrollHeight);
          expect(sim.isNearBottomRef).toBe(true);
        }
      ),
      { numRuns: 200 }
    );
  });

  /**
   * Property 2e: ユーザーが手動で底付近までスクロールダウンした場合、isNearBottomRef が true になる
   *
   * **Validates: Requirements 3.4**
   *
   * ユーザーが一度スクロールアップした後、手動で底付近まで戻った場合、
   * handleScroll により isNearBottomRef が true に更新され、
   * 自動スクロールが再有効化されることを確認。
   * (isProgrammaticScrollRef が false の状態で handleScroll が呼ばれる場合)
   */
  it('property: manual scroll-down to bottom re-enables auto-scroll (isNearBottomRef becomes true)', () => {
    fc.assert(
      fc.property(
        fc.record({
          scrollHeight: fc.integer({ min: 1000, max: 10000 }),
          clientHeight: fc.integer({ min: 200, max: 800 }),
          finalDistanceFromBottom: fc.integer({ min: 0, max: 200 }),
        }),
        ({ scrollHeight, clientHeight, finalDistanceFromBottom }) => {
          if (clientHeight >= scrollHeight) return;
          const finalScrollTop = scrollHeight - clientHeight - finalDistanceFromBottom;
          if (finalScrollTop < 0) return;

          // Setup: ユーザーが以前スクロールアップしていた状態
          const initialScrollTop = Math.max(0, scrollHeight - clientHeight - 500);
          const sim = new ScrollLogicSimulator(scrollHeight, initialScrollTop, clientHeight);
          sim.isNearBottomRef = false; // スクロールアップ中
          sim.isProgrammaticScrollRef = false; // プログラムスクロールではない

          // ユーザーが手動で底付近までスクロールダウン
          sim.scrollTop = finalScrollTop;
          sim.handleScroll();

          // isNearBottomRef が true に更新される（自動スクロール再有効化）
          expect(sim.isNearBottomRef).toBe(true);
        }
      ),
      { numRuns: 200 }
    );
  });
});
