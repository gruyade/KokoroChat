/**
 * Chat Store Thinking Content 蓄積プロパティテスト
 *
 * Feature: thinking-reasoning-support
 * Property 4: Thinking content accumulation preserves concatenation
 *
 * ランダムなデルタ列を生成し、最終accumulated valueが連結と一致することを検証。
 *
 * **Validates: Requirements 3.2, 3.3**
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as fc from 'fast-check';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { useChatStore } from './chat.store';

describe('Property 4: Thinking content accumulation preserves concatenation', () => {
  beforeEach(() => {
    useChatStore.setState({
      streamingThinkingContent: '',
      isThinking: false,
      messages: [],
      currentSessionId: 'test-session',
      isStreaming: true,
      isAbortable: false,
      streamingContent: '',
      error: null,
    });
  });

  /**
   * Property 4a: streamingThinkingContentはすべてのデルタの連結と等しい
   *
   * **Validates: Requirements 3.2**
   */
  it('accumulated thinking content equals concatenation of all deltas', () => {
    fc.assert(
      fc.property(
        fc.array(fc.string({ minLength: 1, maxLength: 100 }), { minLength: 1, maxLength: 50 }),
        (chunks) => {
          // Reset
          useChatStore.setState({
            streamingThinkingContent: '',
            isThinking: false,
          });

          // Append each chunk
          for (const chunk of chunks) {
            useChatStore.getState().appendThinkingChunk(chunk);
          }

          // Verify accumulated equals concatenation
          const expected = chunks.join('');
          const actual = useChatStore.getState().streamingThinkingContent;
          expect(actual).toBe(expected);
        }
      ),
      { numRuns: 100 }
    );
  });

  /**
   * Property 4b: finishStreaming後のcommitted messageのthinking_contentが連結と等しい
   *
   * **Validates: Requirements 3.3**
   */
  it('committed message thinking_content equals concatenation after finishStreaming', () => {
    fc.assert(
      fc.property(
        fc.array(fc.string({ minLength: 1, maxLength: 100 }), { minLength: 1, maxLength: 50 }),
        fc.string({ minLength: 1, maxLength: 200 }),
        (chunks, fullContent) => {
          // Reset
          useChatStore.setState({
            streamingThinkingContent: '',
            isThinking: false,
            messages: [],
            currentSessionId: 'test-session',
            isStreaming: true,
            streamingContent: '',
          });

          // Append each thinking chunk
          for (const chunk of chunks) {
            useChatStore.getState().appendThinkingChunk(chunk);
          }

          // Finish streaming — commits the message
          useChatStore.getState().finishStreaming(fullContent);

          // Verify committed message
          const messages = useChatStore.getState().messages;
          const lastMessage = messages[messages.length - 1];
          const expected = chunks.join('');

          expect(lastMessage).toBeDefined();
          expect(lastMessage.thinking_content).toBe(expected);
          expect(lastMessage.content).toBe(fullContent);
          expect(lastMessage.role).toBe('assistant');
        }
      ),
      { numRuns: 100 }
    );
  });

  /**
   * Property 4c: thinking chunkが空の場合、thinking_contentはnull
   *
   * **Validates: Requirements 3.3**
   */
  it('committed message thinking_content is null when no thinking chunks received', () => {
    fc.assert(
      fc.property(
        fc.string({ minLength: 1, maxLength: 200 }),
        (fullContent) => {
          // Reset — no thinking chunks appended
          useChatStore.setState({
            streamingThinkingContent: '',
            isThinking: false,
            messages: [],
            currentSessionId: 'test-session',
            isStreaming: true,
            streamingContent: '',
          });

          // Finish streaming without any thinking chunks
          useChatStore.getState().finishStreaming(fullContent);

          // Verify committed message has null thinking_content
          const messages = useChatStore.getState().messages;
          const lastMessage = messages[messages.length - 1];

          expect(lastMessage).toBeDefined();
          expect(lastMessage.thinking_content).toBeNull();
          expect(lastMessage.content).toBe(fullContent);
        }
      ),
      { numRuns: 100 }
    );
  });
});
