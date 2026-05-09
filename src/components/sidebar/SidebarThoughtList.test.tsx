import { render, waitFor, act, cleanup, within } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as fc from 'fast-check';
import '@testing-library/jest-dom';

// Mock @tauri-apps/api/core
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// Mock @tauri-apps/api/event
const mockListen = vi.fn();
vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => mockListen(...args),
}));

// Mock useCharacterStore
vi.mock('../../stores', () => ({
  useCharacterStore: () => ({ selectedCharacterId: 'test-char-id' }),
}));

import { SidebarThoughtList } from './SidebarThoughtList';
import type { Thought } from '../../types';

/**
 * Arbitrary generator for Thought data
 */
const thoughtArbitrary = fc.record({
  id: fc.uuid(),
  character_id: fc.constant('test-char-id'),
  content: fc.stringMatching(/^[a-zA-Z0-9]{3,30}$/),
  context: fc.option(fc.stringMatching(/^[a-zA-Z0-9]{3,20}$/), { nil: undefined }),
  created_at: fc.date({ min: new Date('2020-01-01'), max: new Date('2025-12-31') }).map((d) => d.toISOString()),
});

const thoughtsArbitrary = fc.array(thoughtArbitrary, { minLength: 1, maxLength: 5 });

describe('SidebarThoughtList - Bug Condition Exploration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListen.mockResolvedValue(() => {});
  });

  afterEach(() => {
    cleanup();
  });

  /**
   * Property 1: Bug Condition - 思考カードに削除ボタンが存在しない
   *
   * **Validates: Requirements 1.1, 1.2, 2.1, 2.2**
   *
   * For any rendered thought card, a button with title "削除" should exist.
   * This test is EXPECTED TO FAIL on unfixed code, proving the bug exists.
   */
  it('property: every rendered thought card should have a delete button with title "削除"', async () => {
    await fc.assert(
      fc.asyncProperty(thoughtsArbitrary, async (thoughts) => {
        cleanup();
        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === 'get_thoughts') return Promise.resolve(thoughts);
          return Promise.resolve();
        });

        const { container, unmount } = render(<SidebarThoughtList />);
        const view = within(container);

        // Wait for thoughts to load
        await waitFor(() => {
          expect(view.getByText(thoughts[0].content)).toBeInTheDocument();
        });

        // Assert: for each thought, a delete button with title "削除" exists
        const deleteButtons = view.queryAllByTitle('削除');
        expect(deleteButtons.length).toBe(thoughts.length);

        unmount();
      }),
      { numRuns: 5 }
    );
  }, 30000);

  it('property: clicking delete button enqueues delete_thought invocation', async () => {
    await fc.assert(
      fc.asyncProperty(thoughtsArbitrary, async (thoughts) => {
        cleanup();
        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === 'get_thoughts') return Promise.resolve(thoughts);
          if (cmd === 'delete_thought') return Promise.resolve();
          return Promise.resolve();
        });

        const { container, unmount } = render(<SidebarThoughtList />);
        const view = within(container);

        // Wait for thoughts to load
        await waitFor(() => {
          expect(view.getByText(thoughts[0].content)).toBeInTheDocument();
        });

        // Find delete buttons and click the first one
        const deleteButtons = view.queryAllByTitle('削除');
        expect(deleteButtons.length).toBeGreaterThan(0);

        if (deleteButtons.length > 0) {
          await act(async () => {
            deleteButtons[0].click();
          });
          // キューイング方式: confirmなしで即座にdelete_thoughtが呼ばれる
          await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('delete_thought', { id: thoughts[0].id });
          });
        }

        unmount();
      }),
      { numRuns: 5 }
    );
  }, 30000);
});

/**
 * Helper: Generate an array of thoughts with unique IDs and unique content.
 * Uses index-based generation to guarantee uniqueness even during shrinking.
 */
function makeUniqueThoughts(count: number, seed: number, withContext: boolean = false): Thought[] {
  return Array.from({ length: count }, (_, i) => ({
    id: `thought-${seed}-${i}`,
    character_id: 'test-char-id',
    content: `content_${seed}_${i}`,
    context: withContext ? `context_${seed}_${i}` : undefined,
    created_at: new Date(2020 + (i % 5), i % 12, (i % 28) + 1).toISOString(),
  }));
}

describe('SidebarThoughtList - Preservation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListen.mockResolvedValue(() => {});
  });

  afterEach(() => {
    cleanup();
  });

  /**
   * Preservation Property: 全思考のcontentテキストがDOMにレンダリングされる
   *
   * **Validates: Requirements 3.1, 3.3**
   *
   * For all generated thought arrays (length 1-20): each thought's content text is rendered in the DOM.
   */
  it('property: all thought content texts are rendered in the DOM', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.integer({ min: 1, max: 20 }),
        fc.integer({ min: 0, max: 9999 }),
        async (count, seed) => {
          cleanup();
          const thoughts = makeUniqueThoughts(count, seed);

          mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_thoughts') return Promise.resolve(thoughts);
            return Promise.resolve();
          });

          const { container, unmount } = render(<SidebarThoughtList />);
          const view = within(container);

          // Wait for first thought to appear
          await waitFor(() => {
            expect(view.getByText(thoughts[0].content)).toBeInTheDocument();
          });

          // Assert: every thought's content is rendered
          for (const thought of thoughts) {
            expect(view.getByText(thought.content)).toBeInTheDocument();
          }

          unmount();
        }
      ),
      { numRuns: 10 }
    );
  }, 30000);

  /**
   * Preservation Property: レイアウト順序は content → context → date (上から下)
   *
   * **Validates: Requirements 3.3**
   *
   * For all generated thought arrays: layout order within each card is content → context → date (top to bottom).
   */
  it('property: layout order is content → context → date within each card', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.integer({ min: 1, max: 5 }),
        fc.integer({ min: 0, max: 9999 }),
        async (count, seed) => {
          cleanup();
          const thoughts = makeUniqueThoughts(count, seed, true);

          mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_thoughts') return Promise.resolve(thoughts);
            return Promise.resolve();
          });

          const { container, unmount } = render(<SidebarThoughtList />);
          const view = within(container);

          await waitFor(() => {
            expect(view.getByText(thoughts[0].content)).toBeInTheDocument();
          });

          // For each thought card, verify DOM order: content appears before context, context before date
          for (const thought of thoughts) {
            const contentEl = view.getByText(thought.content);
            view.getByText(thought.context!);

            // Get the parent card element
            const card = contentEl.closest('.rounded-lg');
            expect(card).not.toBeNull();

            // Get all paragraph/div elements within the card to check order
            const allTextElements = card!.querySelectorAll('p, div.text-xs');

            // Convert to array and find indices
            const elements = Array.from(allTextElements);
            const contentIndex = elements.findIndex((el) => el.textContent === thought.content);
            const contextIndex = elements.findIndex((el) => el.textContent === thought.context);
            const dateIndex = elements.findIndex((el) =>
              el.classList.contains('text-muted-foreground/70')
            );

            // content should come before context, context before date
            expect(contentIndex).toBeLessThan(contextIndex);
            expect(contextIndex).toBeLessThan(dateIndex);
          }

          unmount();
        }
      ),
      { numRuns: 10 }
    );
  }, 30000);

  /**
   * Preservation Property: 20件のリストに新しい思考を追加しても20件を維持
   *
   * **Validates: Requirements 3.2**
   *
   * For all generated thought arrays with length 20: adding a new thought via event keeps list at 20 items.
   */
  it('property: adding a thought via event to a full list (20 items) keeps list at 20', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.integer({ min: 0, max: 9999 }),
        async (seed) => {
          cleanup();
          const thoughts = makeUniqueThoughts(20, seed);
          const newThought: Thought = {
            id: `new-thought-${seed}`,
            character_id: 'test-char-id',
            content: `new_content_${seed}`,
            context: undefined,
            created_at: new Date().toISOString(),
          };

          let listenCallback: ((event: { payload: { character_id: string; thought: Thought } }) => void) | null = null;

          mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_thoughts') return Promise.resolve(thoughts);
            return Promise.resolve();
          });

          mockListen.mockImplementation((_eventName: string, callback: (...args: unknown[]) => void) => {
            listenCallback = callback as typeof listenCallback;
            return Promise.resolve(() => {});
          });

          const { container, unmount } = render(<SidebarThoughtList />);
          const view = within(container);

          // Wait for initial thoughts to load
          await waitFor(() => {
            expect(view.getByText(thoughts[0].content)).toBeInTheDocument();
          });

          // Count initial thought cards
          const initialCards = container.querySelectorAll('.rounded-lg');
          expect(initialCards.length).toBe(20);

          // Simulate thought:generated event
          expect(listenCallback).not.toBeNull();
          await act(async () => {
            listenCallback!({
              payload: {
                character_id: 'test-char-id',
                thought: newThought,
              },
            });
          });

          // After adding, list should still be 20 items
          await waitFor(() => {
            const updatedCards = container.querySelectorAll('.rounded-lg');
            expect(updatedCards.length).toBe(20);
          });

          // New thought should be at the top
          expect(view.getByText(newThought.content)).toBeInTheDocument();

          unmount();
        }
      ),
      { numRuns: 5 }
    );
  }, 60000);

  /**
   * Preservation Property: contextを持つ思考はitalicスタイルでレンダリングされる
   *
   * **Validates: Requirements 3.3**
   *
   * For generated thoughts with context: context text is rendered with italic styling.
   */
  it('property: thoughts with context render context text with italic styling', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.integer({ min: 1, max: 10 }),
        fc.integer({ min: 0, max: 9999 }),
        async (count, seed) => {
          cleanup();
          const thoughts = makeUniqueThoughts(count, seed, true);

          mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_thoughts') return Promise.resolve(thoughts);
            return Promise.resolve();
          });

          const { container, unmount } = render(<SidebarThoughtList />);
          const view = within(container);

          await waitFor(() => {
            expect(view.getByText(thoughts[0].content)).toBeInTheDocument();
          });

          // Assert: each thought's context is rendered with italic class
          for (const thought of thoughts) {
            const contextEl = view.getByText(thought.context!);
            expect(contextEl).toBeInTheDocument();
            expect(contextEl.classList.contains('italic')).toBe(true);
          }

          unmount();
        }
      ),
      { numRuns: 10 }
    );
  }, 30000);
});
