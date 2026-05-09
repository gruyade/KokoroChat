# Implementation Plan

- [x] 1. Write bug condition exploration test
  - **Property 1: Bug Condition** - ユーザースクロールアップ中の位置維持
  - **CRITICAL**: This test MUST FAIL on unfixed code - failure confirms the bug exists
  - **DO NOT attempt to fix the test or the code when it fails**
  - **NOTE**: This test encodes the expected behavior - it will validate the fix when it passes after implementation
  - **GOAL**: Surface counterexamples that demonstrate the bug exists
  - **Scoped PBT Approach**: Scope the property to cases where user has scrolled up (distance > 200px) during streaming
  - Test file: `src/components/chat/ChatView.test.tsx`
  - Use `fast-check` to generate random scroll positions where `scrollHeight - scrollTop - clientHeight > 200`
  - Simulate the bug condition: `isProgrammaticScrollRef` is true (from prior `smoothScrollToBottom` call), user scrolls up via wheel event, then `handleScroll` is called
  - Assert: after user wheel-up event during streaming, `isNearBottomRef` should become `false` (expected behavior)
  - Assert: `smoothScrollToBottom` should NOT execute scroll when user is scrolled up
  - On UNFIXED code: test will FAIL because `handleScroll` early-returns when `isProgrammaticScrollRef` is true, so `isNearBottomRef` never updates to false
  - Document counterexamples: e.g., "User at scrollTop=100 with scrollHeight=1000, clientHeight=600 (400px from bottom) — wheel up event ignored, isNearBottomRef remains true, auto-scroll fires"
  - Also test `shouldAutoScroll` in isolation to confirm threshold logic is correct (this should pass — the bug is in the event handling, not the threshold function)
  - Run test on UNFIXED code
  - **EXPECTED OUTCOME**: Test FAILS (this is correct - it proves the bug exists)
  - Mark task complete when test is written, run, and failure is documented
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2_

- [x] 2. Write preservation property tests (BEFORE implementing fix)
  - **Property 2: Preservation** - 底付近での自動スクロール継続
  - **IMPORTANT**: Follow observation-first methodology
  - Test file: `src/components/chat/ChatView.test.tsx`
  - Use `fast-check` to generate random scroll positions where `scrollHeight - scrollTop - clientHeight <= 200` (user near bottom)
  - Observe on UNFIXED code: `shouldAutoScroll` returns `true` for all positions within 200px of bottom
  - Observe on UNFIXED code: `smoothScrollToBottom` executes `scrollTo` when `isNearBottomRef` is true
  - Observe on UNFIXED code: `scrollToBottomInstant` sets `scrollTop = scrollHeight` and `isNearBottomRef = true`
  - Write property-based tests:
    - For all `(scrollHeight, scrollTop, clientHeight)` where `scrollHeight - scrollTop - clientHeight <= 200`: `shouldAutoScroll` returns `true`
    - For all `(scrollHeight, scrollTop, clientHeight)` where `scrollHeight - scrollTop - clientHeight > 200`: `shouldAutoScroll` returns `false`
    - When `isNearBottomRef` is true and streaming content updates, `smoothScrollToBottom` triggers `scrollTo`
    - Session switch triggers `scrollToBottomInstant` regardless of current scroll position
    - When user manually scrolls back to bottom (distance <= 200px), `isNearBottomRef` becomes `true` (auto-scroll re-enabled)
  - Verify all tests PASS on UNFIXED code
  - **EXPECTED OUTCOME**: Tests PASS (this confirms baseline behavior to preserve)
  - Mark task complete when tests are written, run, and passing on unfixed code
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [x] 3. Fix for チャットストリーミング中のユーザースクロール位置強制リセット

  - [x] 3.1 Implement the fix in `src/components/chat/ChatView.tsx`
    - Add `wheel` and `touchmove` event listeners that directly update `isNearBottomRef` to `false` when user scrolls up, bypassing `isProgrammaticScrollRef` guard
    - Modify `handleScroll`: remove or reduce `isProgrammaticScrollRef` early-return scope so that user-initiated scroll events can still update `isNearBottomRef`
    - Add re-check of `isNearBottomRef` inside `smoothScrollToBottom`'s rAF callback (just before `scrollTo`) to catch user scroll-up between schedule and execution
    - Add throttling to `smoothScrollToBottom` calls during streaming to prevent continuous 300ms timeout extension
    - Reduce `isProgrammaticScrollRef` scope: only block scroll-event-based updates, not user-action-event-based updates
    - _Bug_Condition: isBugCondition(input) where userScrolledUp=true AND isStreaming=true AND streamingContentUpdated=true AND isProgrammaticScrollRef blocks handleScroll_
    - _Expected_Behavior: User scroll position preserved when scrolled up during streaming; isNearBottomRef correctly set to false via wheel/touchmove listeners_
    - _Preservation: Near-bottom auto-scroll, session-switch instant scroll, manual scroll-down re-enable all unchanged_
    - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3, 3.4_

  - [x] 3.2 Verify bug condition exploration test now passes
    - **Property 1: Expected Behavior** - ユーザースクロールアップ中の位置維持
    - **IMPORTANT**: Re-run the SAME test from task 1 - do NOT write a new test
    - The test from task 1 encodes the expected behavior (isNearBottomRef becomes false on user scroll-up, auto-scroll suppressed)
    - When this test passes, it confirms the expected behavior is satisfied
    - Run bug condition exploration test from step 1
    - **EXPECTED OUTCOME**: Test PASSES (confirms bug is fixed)
    - _Requirements: 2.1, 2.2, 2.3_

  - [x] 3.3 Verify preservation tests still pass
    - **Property 2: Preservation** - 底付近での自動スクロール継続
    - **IMPORTANT**: Re-run the SAME tests from task 2 - do NOT write new tests
    - Run preservation property tests from step 2
    - **EXPECTED OUTCOME**: Tests PASS (confirms no regressions)
    - Confirm all tests still pass after fix (no regressions)
    - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [x] 4. Checkpoint - Ensure all tests pass
  - Run `vitest run` to execute full test suite
  - Confirm Property 1 (Bug Condition → Expected Behavior) passes
  - Confirm Property 2 (Preservation) passes
  - Confirm no other tests are broken
  - Ask the user if questions arise
