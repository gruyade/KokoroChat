/**
 * Chat Store Thinking Content プロパティテスト
 *
 * Feature: thinking-reasoning-support
 * Property 8: Tool break preserves accumulated thinking content
 *
 * tool_break発生時にthinking contentが確定バブルに正しく関連付けられ、
 * バッファがリセットされることを検証する。
 *
 * **Validates: Requirements 8.1, 8.2**
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as fc from 'fast-check';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { useChatStore } from './chat.store';

describe('Chat Store - Property 8: Tool break preserves accumulated thinking content', () => {
  beforeEach(() => {
    useChatStore.setState({
      streamingThinkingContent: '',
      isThinking: false,
      messages: [],
      currentSessionId: 'test-session',
      isStreaming: true,
      streamingContent: '',
      executingToolName: null,
    });
  });

  /**
   * Property 8: Tool break preserves accumulated thinking content
   *
   * For any tool_break event occurring during streaming, the thinking content
   * accumulated up to that point SHALL be associated with the committed pre-tool
   * assistant message, and the thinking buffer SHALL be reset to empty for
   * subsequent streaming.
   *
   * **Validates: Requirements 8.1, 8.2**
   */
  it('tool break preserves thinking content in committed message', () => {
    fc.assert(
      fc.property(
        fc.array(fc.string({ minLength: 1, maxLength: 100 }), { minLength: 1, maxLength: 20 }),
        fc.string({ minLength: 1, maxLength: 200 }).filter((s) => s.trim().length > 0), // text content for commitPreToolContent
        (thinkingChunks, textContent) => {
          // Reset state for each iteration
          useChatStore.setState({
            streamingThinkingContent: '',
            isThinking: false,
            messages: [],
            currentSessionId: 'test-session',
            isStreaming: true,
            streamingContent: '',
            executingToolName: null,
          });

          // Accumulate thinking chunks
          for (const chunk of thinkingChunks) {
            useChatStore.getState().appendThinkingChunk(chunk);
          }

          // Verify thinking is being accumulated
          const expectedThinking = thinkingChunks.join('');
          expect(useChatStore.getState().streamingThinkingContent).toBe(expectedThinking);
          expect(useChatStore.getState().isThinking).toBe(true);

          // Commit pre-tool content (simulates tool_break)
          useChatStore.getState().commitPreToolContent(textContent);

          const state = useChatStore.getState();

          // Committed message has the accumulated thinking_content
          const lastMessage = state.messages[state.messages.length - 1];
          expect(lastMessage.thinking_content).toBe(expectedThinking || null);
          expect(lastMessage.content).toBe(textContent);
          expect(lastMessage.role).toBe('assistant');

          // Buffer is reset after commit
          expect(state.streamingThinkingContent).toBe('');
          expect(state.isThinking).toBe(false);
        }
      ),
      { numRuns: 100 }
    );
  });
});
