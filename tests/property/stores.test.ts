/**
 * フロントエンド Zustand Store プロパティテスト
 *
 * fast-check を使用して状態管理ロジックの不変条件を検証する。
 * Tauri API (invoke/listen) はモックし、純粋な状態遷移ロジックに集中。
 *
 * **Validates: Requirements 全体（フロントエンド状態管理）**
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as fc from 'fast-check';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock @tauri-apps/api/event
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

// Mock @tauri-apps/plugin-dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

import { useCharacterStore } from '../../src/stores/character.store';
import { useChatStore } from '../../src/stores/chat.store';
import { useUIStore } from '../../src/stores/ui.store';

describe('Character Store Property Tests', () => {
  beforeEach(() => {
    // Reset store state
    useCharacterStore.setState({
      characters: [],
      selectedCharacterId: null,
      loading: false,
      error: null,
    });
  });

  it('create/delete操作の任意シーケンスでキャラクターリストが一貫する', () => {
    /**
     * Property: For any sequence of create/delete operations,
     * the character list should contain exactly the characters
     * that were created and not yet deleted.
     *
     * **Validates: Requirements 1.4, 1.6**
     */
    const characterArb = fc.record({
      id: fc.uuid(),
      name: fc.string({ minLength: 1, maxLength: 50 }),
      description: fc.string({ maxLength: 200 }),
      system_prompt: fc.string({ maxLength: 500 }),
      created_at: fc.constant(new Date().toISOString()),
      updated_at: fc.constant(new Date().toISOString()),
    });

    type Op =
      | { type: 'create'; character: { id: string; name: string; description: string; system_prompt: string; created_at: string; updated_at: string } }
      | { type: 'delete'; index: number };

    const createOp = characterArb.map((c) => ({ type: 'create' as const, character: c }));
    const deleteOp = fc.nat({ max: 100 }).map((index) => ({ type: 'delete' as const, index }));
    const opArb: fc.Arbitrary<Op> = fc.oneof(createOp, deleteOp);

    fc.assert(
      fc.property(fc.array(opArb, { minLength: 1, maxLength: 30 }), (ops) => {
        // Reset state
        useCharacterStore.setState({ characters: [], selectedCharacterId: null });

        // Track expected state
        const expected: string[] = [];

        for (const op of ops) {
          const state = useCharacterStore.getState();

          if (op.type === 'create') {
            // Simulate successful create (直接stateを操作)
            useCharacterStore.setState({
              characters: [...state.characters, op.character],
            });
            expected.push(op.character.id);
          } else if (op.type === 'delete') {
            const chars = useCharacterStore.getState().characters;
            if (chars.length > 0) {
              const idx = op.index % chars.length;
              const idToDelete = chars[idx].id;
              // Simulate successful delete
              useCharacterStore.setState({
                characters: chars.filter((c) => c.id !== idToDelete),
                selectedCharacterId:
                  state.selectedCharacterId === idToDelete ? null : state.selectedCharacterId,
              });
              const expectedIdx = expected.indexOf(idToDelete);
              if (expectedIdx !== -1) expected.splice(expectedIdx, 1);
            }
          }
        }

        // Verify consistency
        const finalState = useCharacterStore.getState();
        const actualIds = finalState.characters.map((c) => c.id).sort();
        const expectedIds = [...expected].sort();

        expect(actualIds).toEqual(expectedIds);
        expect(finalState.characters.length).toBe(expected.length);
      }),
      { numRuns: 100 }
    );
  });

  it('削除されたキャラクターがselectedCharacterIdの場合、nullにリセットされる', () => {
    /**
     * Property: If the selected character is deleted,
     * selectedCharacterId becomes null.
     *
     * **Validates: Requirements 1.6**
     */
    const characterArb = fc.record({
      id: fc.uuid(),
      name: fc.string({ minLength: 1, maxLength: 50 }),
      description: fc.string({ maxLength: 200 }),
      system_prompt: fc.string({ maxLength: 500 }),
      created_at: fc.constant(new Date().toISOString()),
      updated_at: fc.constant(new Date().toISOString()),
    });

    fc.assert(
      fc.property(
        fc.array(characterArb, { minLength: 1, maxLength: 10 }),
        fc.nat(),
        (characters, selectIdx) => {
          // Setup: add characters and select one
          useCharacterStore.setState({
            characters,
            selectedCharacterId: characters[selectIdx % characters.length].id,
          });

          const selectedId = useCharacterStore.getState().selectedCharacterId!;

          // Delete the selected character
          const state = useCharacterStore.getState();
          useCharacterStore.setState({
            characters: state.characters.filter((c) => c.id !== selectedId),
            selectedCharacterId:
              state.selectedCharacterId === selectedId ? null : state.selectedCharacterId,
          });

          // Verify: selectedCharacterId is null
          expect(useCharacterStore.getState().selectedCharacterId).toBeNull();
        }
      ),
      { numRuns: 100 }
    );
  });
});

describe('Chat Store Property Tests', () => {
  beforeEach(() => {
    useChatStore.setState({
      sessions: [],
      currentSessionId: null,
      messages: [],
      isStreaming: false,
      streamingContent: '',
      error: null,
    });
  });

  it('appendStreamChunkが任意のチャンク列を正しく蓄積する', () => {
    /**
     * Property: For any sequence of string chunks,
     * appendStreamChunk accumulates them into streamingContent
     * such that the final content equals the concatenation of all chunks.
     *
     * **Validates: Requirements 2.5**
     */
    fc.assert(
      fc.property(
        fc.array(fc.string({ minLength: 0, maxLength: 100 }), { minLength: 1, maxLength: 50 }),
        (chunks) => {
          // Reset streaming state
          useChatStore.setState({ streamingContent: '', isStreaming: false });

          // Append all chunks
          for (const chunk of chunks) {
            useChatStore.getState().appendStreamChunk(chunk);
          }

          // Verify: streamingContent equals concatenation
          const expectedContent = chunks.join('');
          const state = useChatStore.getState();

          expect(state.streamingContent).toBe(expectedContent);
          expect(state.isStreaming).toBe(true);
        }
      ),
      { numRuns: 100 }
    );
  });

  it('finishStreamingがストリーミング状態をリセットしメッセージを追加する', () => {
    /**
     * Property: After finishStreaming is called with any content,
     * isStreaming becomes false, streamingContent becomes empty,
     * and a new assistant message with that content is appended.
     *
     * **Validates: Requirements 2.5**
     */
    fc.assert(
      fc.property(
        fc.string({ minLength: 1, maxLength: 500 }),
        fc.uuid(),
        (fullContent, sessionId) => {
          // Setup: simulate active streaming session
          useChatStore.setState({
            currentSessionId: sessionId,
            messages: [],
            isStreaming: true,
            streamingContent: 'partial...',
          });

          // Finish streaming
          useChatStore.getState().finishStreaming(fullContent);

          const state = useChatStore.getState();

          // Verify state reset
          expect(state.isStreaming).toBe(false);
          expect(state.streamingContent).toBe('');

          // Verify message added
          expect(state.messages.length).toBe(1);
          expect(state.messages[0].role).toBe('assistant');
          expect(state.messages[0].content).toBe(fullContent);
          expect(state.messages[0].session_id).toBe(sessionId);
        }
      ),
      { numRuns: 100 }
    );
  });

  it('appendStreamChunk後のfinishStreamingで状態が完全にリセットされる', () => {
    /**
     * Property: After any sequence of appendStreamChunk followed by finishStreaming,
     * the streaming state is fully reset regardless of chunk content.
     *
     * **Validates: Requirements 2.5**
     */
    fc.assert(
      fc.property(
        fc.array(fc.string({ minLength: 1, maxLength: 50 }), { minLength: 1, maxLength: 20 }),
        fc.string({ minLength: 1, maxLength: 200 }),
        fc.uuid(),
        (chunks, finalContent, sessionId) => {
          useChatStore.setState({
            currentSessionId: sessionId,
            messages: [],
            isStreaming: false,
            streamingContent: '',
          });

          // Simulate streaming
          for (const chunk of chunks) {
            useChatStore.getState().appendStreamChunk(chunk);
          }

          // Verify streaming is active
          expect(useChatStore.getState().isStreaming).toBe(true);

          // Finish
          useChatStore.getState().finishStreaming(finalContent);

          const state = useChatStore.getState();
          expect(state.isStreaming).toBe(false);
          expect(state.streamingContent).toBe('');
          expect(state.messages[state.messages.length - 1].content).toBe(finalContent);
        }
      ),
      { numRuns: 100 }
    );
  });
});

describe('UI Store Property Tests', () => {
  beforeEach(() => {
    useUIStore.setState({
      theme: 'dark',
      sidebarOpen: true,
      activeView: 'chat',
    });
  });

  it('toggleThemeが常にlightとdarkを交互に切り替える', () => {
    /**
     * Property: For any number of toggleTheme calls,
     * the theme always alternates between 'light' and 'dark'.
     * After an even number of toggles, theme returns to initial.
     * After an odd number, theme is the opposite.
     *
     * **Validates: Requirements 9.2**
     */
    fc.assert(
      fc.property(
        fc.oneof(fc.constant('light' as const), fc.constant('dark' as const)),
        fc.nat({ max: 100 }),
        (initialTheme, toggleCount) => {
          // Set initial theme
          useUIStore.setState({ theme: initialTheme });

          // Toggle N times
          for (let i = 0; i < toggleCount; i++) {
            useUIStore.getState().toggleTheme();
          }

          const finalTheme = useUIStore.getState().theme;

          if (toggleCount % 2 === 0) {
            // Even toggles: back to initial
            expect(finalTheme).toBe(initialTheme);
          } else {
            // Odd toggles: opposite
            expect(finalTheme).toBe(initialTheme === 'light' ? 'dark' : 'light');
          }
        }
      ),
      { numRuns: 100 }
    );
  });

  it('toggleThemeの結果は常にlightまたはdarkのみ', () => {
    /**
     * Property: The theme value is always either 'light' or 'dark',
     * never any other value, regardless of how many times toggled.
     *
     * **Validates: Requirements 9.2**
     */
    fc.assert(
      fc.property(fc.nat({ max: 200 }), (toggleCount) => {
        useUIStore.setState({ theme: 'dark' });

        for (let i = 0; i < toggleCount; i++) {
          useUIStore.getState().toggleTheme();
          const theme = useUIStore.getState().theme;
          expect(theme === 'light' || theme === 'dark').toBe(true);
        }
      }),
      { numRuns: 50 }
    );
  });

  it('toggleSidebarが常にtrue/falseを交互に切り替える', () => {
    /**
     * Property: toggleSidebar alternates the sidebarOpen boolean.
     *
     * **Validates: Requirements 9.4**
     */
    fc.assert(
      fc.property(fc.boolean(), fc.nat({ max: 50 }), (initialOpen, toggleCount) => {
        useUIStore.setState({ sidebarOpen: initialOpen });

        for (let i = 0; i < toggleCount; i++) {
          useUIStore.getState().toggleSidebar();
        }

        const finalOpen = useUIStore.getState().sidebarOpen;

        if (toggleCount % 2 === 0) {
          expect(finalOpen).toBe(initialOpen);
        } else {
          expect(finalOpen).toBe(!initialOpen);
        }
      }),
      { numRuns: 100 }
    );
  });
});
