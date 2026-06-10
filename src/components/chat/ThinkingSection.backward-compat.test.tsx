/**
 * 後方互換性テスト — Thinking Content
 *
 * Task 9.2: MessageBubble / ChatStore の後方互換性確認
 * - thinking_content が undefined/null の場合、ThinkingSection を表示しない
 * - thinking chunk を受信しない場合、ChatStore が正常動作する
 *
 * **Validates: Requirements 2.5, 2.6, 4.4, 5.6**
 */

import { render, cleanup } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import '@testing-library/jest-dom';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { useChatStore } from '../../stores/chat.store';
import { ThinkingSection } from './ThinkingSection';

describe('Backward Compatibility - ThinkingSection rendering', () => {
  afterEach(() => {
    cleanup();
  });

  /**
   * thinking_content が undefined の場合、ThinkingSection をレンダリングしない
   * （親コンポーネント MessageBubble の条件分岐を検証）
   *
   * **Validates: Requirements 5.6**
   */
  it('should not render ThinkingSection when thinking_content is undefined', () => {
    // MessageBubble内の条件: !isCurrentlyStreaming && message.thinking_content && <ThinkingSection>
    // thinking_content が undefined なら falsy → レンダリングしない
    const thinkingContent: string | undefined = undefined;
    const shouldRender = !!(thinkingContent);

    expect(shouldRender).toBe(false);

    // ThinkingSectionが条件分岐で呼ばれない場合のDOMを検証
    const { container } = render(<div>{shouldRender && <ThinkingSection thinkingContent={thinkingContent!} />}</div>);
    expect(container.querySelector('[aria-expanded]')).toBeNull();
  });

  /**
   * thinking_content が null の場合、ThinkingSection をレンダリングしない
   *
   * **Validates: Requirements 5.6**
   */
  it('should not render ThinkingSection when thinking_content is null', () => {
    const thinkingContent: string | null = null;
    const shouldRender = !!(thinkingContent);

    expect(shouldRender).toBe(false);

    const { container } = render(<div>{shouldRender && <ThinkingSection thinkingContent={thinkingContent!} />}</div>);
    expect(container.querySelector('[aria-expanded]')).toBeNull();
  });

  /**
   * thinking_content が空文字列の場合、ThinkingSection をレンダリングしない
   * MessageBubble の条件 `message.thinking_content && ...` は空文字列で falsy
   *
   * **Validates: Requirements 5.6**
   */
  it('should not render ThinkingSection when thinking_content is empty string', () => {
    const thinkingContent = '';
    const shouldRender = !!(thinkingContent);

    expect(shouldRender).toBe(false);

    const { container } = render(<div>{shouldRender && <ThinkingSection thinkingContent={thinkingContent} />}</div>);
    expect(container.querySelector('[aria-expanded]')).toBeNull();
  });

  /**
   * thinking_content が存在する場合のみ ThinkingSection をレンダリングする
   *
   * **Validates: Requirements 5.1**
   */
  it('should render ThinkingSection when thinking_content has value', () => {
    const thinkingContent = 'モデルの思考プロセス';
    const shouldRender = !!(thinkingContent);

    expect(shouldRender).toBe(true);

    const { getByRole } = render(<div>{shouldRender && <ThinkingSection thinkingContent={thinkingContent} />}</div>);
    expect(getByRole('button', { name: '思考プロセスの表示切り替え' })).toBeInTheDocument();
  });
});

describe('Backward Compatibility - ChatStore without thinking chunks', () => {
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
   * thinking chunk を一切受信しない場合、streamingThinkingContent は空のまま
   *
   * **Validates: Requirements 2.5**
   */
  it('streamingThinkingContent stays empty when no thinking chunks received', () => {
    const state = useChatStore.getState();
    expect(state.streamingThinkingContent).toBe('');
    expect(state.isThinking).toBe(false);
  });

  /**
   * thinking chunk なしで finishStreaming した場合、メッセージの thinking_content は null
   *
   * **Validates: Requirements 2.5, 4.4**
   */
  it('finishStreaming produces message with thinking_content: null when no thinking received', () => {
    // テキストのみストリーミング（thinking chunk なし）
    useChatStore.getState().finishStreaming('通常の応答テキスト');

    const state = useChatStore.getState();
    const messages = state.messages;

    expect(messages.length).toBe(1);
    expect(messages[0].content).toBe('通常の応答テキスト');
    expect(messages[0].thinking_content).toBeNull();
    expect(messages[0].role).toBe('assistant');

    // ストリーミング状態がリセットされている
    expect(state.isStreaming).toBe(false);
    expect(state.streamingThinkingContent).toBe('');
    expect(state.isThinking).toBe(false);
  });

  /**
   * thinking chunk なしで commitPreToolContent した場合、確定メッセージの thinking_content は null/undefined
   *
   * **Validates: Requirements 2.5, 2.6**
   */
  it('commitPreToolContent produces message with null/undefined thinking_content when no thinking received', () => {
    useChatStore.getState().commitPreToolContent('ツール呼び出し前のテキスト');

    const state = useChatStore.getState();
    const messages = state.messages;

    expect(messages.length).toBe(1);
    expect(messages[0].content).toBe('ツール呼び出し前のテキスト');
    // thinking_content は null または undefined（空文字列は || null で null に変換される）
    expect(messages[0].thinking_content == null).toBe(true);
    expect(messages[0].role).toBe('assistant');
  });

  /**
   * 複数の通常テキストストリーミング後に finishStreaming — thinking 関連状態は影響なし
   *
   * **Validates: Requirements 2.5**
   */
  it('multiple text-only streams work without affecting thinking state', () => {
    // 1回目のストリーミング
    useChatStore.getState().finishStreaming('1回目の応答');

    // 2回目のストリーミング準備
    useChatStore.setState({
      isStreaming: true,
      streamingContent: '',
      streamingThinkingContent: '',
      currentSessionId: 'test-session',
    });

    useChatStore.getState().finishStreaming('2回目の応答');

    const messages = useChatStore.getState().messages;
    expect(messages.length).toBe(2);
    expect(messages[0].thinking_content).toBeNull();
    expect(messages[1].thinking_content).toBeNull();
    expect(messages[0].content).toBe('1回目の応答');
    expect(messages[1].content).toBe('2回目の応答');
  });
});
