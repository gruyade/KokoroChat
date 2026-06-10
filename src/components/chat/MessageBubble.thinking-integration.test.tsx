/**
 * Integration Test: MessageBubble + ThinkingSection 統合テスト
 *
 * Task 9.1: 全層の結合テスト — フロントエンド側
 * - thinking_content 付きメッセージで ThinkingSection が表示されることを確認
 * - thinking_content が null の場合、ThinkingSection が表示されないことを確認
 * - ストリーミング中の streamingThinkingContent が ThinkingSection に反映されることを確認
 *
 * **Validates: Requirements 1.5, 2.2, 4.2, 4.3, 5.1**
 */

import { render, cleanup, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import '@testing-library/jest-dom';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock useAudio hook
vi.mock('../../hooks/useAudio', () => ({
  useAudioStore: {
    getState: () => ({ playAudioFn: null }),
  },
}));

import { useChatStore } from '../../stores/chat.store';
import { useCharacterStore } from '../../stores/character.store';
import { useConfigStore } from '../../stores/config.store';
import { MessageBubble } from './MessageBubble';
import type { ChatMessageRecord } from '../../types';

describe('Integration: MessageBubble renders ThinkingSection for thinking_content', () => {
  beforeEach(() => {
    // Store state: ストリーミングなし
    useChatStore.setState({
      streamingThinkingContent: '',
      isThinking: false,
      messages: [],
      currentSessionId: 'test-session',
      isStreaming: false,
      streamingContent: '',
      executingToolName: null,
      editingMessageId: null,
    });

    useCharacterStore.setState({
      selectedCharacterId: 'char-001',
      characters: [
        {
          id: 'char-001',
          name: 'Test Character',
          description: 'Test',
          system_prompt: 'You are a test character.',
          avatar_path: undefined,
          tts_config: undefined,
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
      ],
    });

    useConfigStore.setState({
      config: {
        models: {},
        spontaneous: { enabled: false, min_interval_seconds: 60, probability: 0.3 },
        thought: { enabled: false, interval_minutes: 5, auto_delete_threshold_minutes: 1440 },
        memory: { compression_threshold: 50 },
        tts: {
          enabled: false,
          voicepeak_path: null,
          timeout_seconds: 60,
          max_chunk_size: 140,
          irodori_base_url: null,
          irodori_caption_base_url: null,
          irodori_reference_audio_base_url: null,
        },
        ui: { theme: 'dark', language: 'ja', send_key: 'Enter' },
        plugins: { enabled_plugins: [], plugin_settings: {} },
        attachment: { max_file_size_bytes: 10485760, allowed_extensions: [] },
      } as never,
    });
  });

  afterEach(() => {
    cleanup();
  });

  /**
   * thinking_content 付きアシスタントメッセージの場合、ThinkingSection が表示される
   *
   * DB保存後の get_messages で返されるレコードに thinking_content が含まれている場合、
   * MessageBubble 内部で ThinkingSection コンポーネントがレンダリングされることを確認
   *
   * **Validates: Requirements 4.3, 5.1**
   */
  it('renders ThinkingSection when message has thinking_content', () => {
    const message: ChatMessageRecord = {
      id: 'msg-001',
      session_id: 'test-session',
      role: 'assistant',
      content: 'Here is my response about quantum physics.',
      thinking_content: 'Let me think step by step about quantum mechanics...',
      created_at: '2024-01-01T10:00:00Z',
    };

    const { getByRole } = render(<MessageBubble message={message} />);

    // ThinkingSection のトグルボタンが存在することを確認
    const toggleButton = getByRole('button', { name: '思考プロセスの表示切り替え' });
    expect(toggleButton).toBeInTheDocument();
    expect(toggleButton).toHaveAttribute('aria-expanded', 'false');
  });

  /**
   * ThinkingSection を展開すると thinking_content のテキストが表示される
   *
   * **Validates: Requirements 5.1, 5.3**
   */
  it('shows thinking_content text when ThinkingSection is expanded', () => {
    const message: ChatMessageRecord = {
      id: 'msg-002',
      session_id: 'test-session',
      role: 'assistant',
      content: 'The answer is 42.',
      thinking_content: 'I need to calculate the meaning of life...',
      created_at: '2024-01-01T10:01:00Z',
    };

    const { getByRole, getByText } = render(<MessageBubble message={message} />);

    // トグルで展開
    const toggleButton = getByRole('button', { name: '思考プロセスの表示切り替え' });
    fireEvent.click(toggleButton);

    // thinking_content が表示される
    expect(getByText('I need to calculate the meaning of life...')).toBeInTheDocument();
  });

  /**
   * thinking_content が null の場合、ThinkingSection は表示されない
   *
   * **Validates: Requirements 5.6**
   */
  it('does not render ThinkingSection when thinking_content is null', () => {
    const message: ChatMessageRecord = {
      id: 'msg-003',
      session_id: 'test-session',
      role: 'assistant',
      content: 'A simple response without thinking.',
      thinking_content: null,
      created_at: '2024-01-01T10:02:00Z',
    };

    const { queryByRole } = render(<MessageBubble message={message} />);

    // ThinkingSection のトグルボタンが存在しない
    expect(queryByRole('button', { name: '思考プロセスの表示切り替え' })).not.toBeInTheDocument();
  });

  /**
   * thinking_content が undefined の場合も ThinkingSection は表示されない
   *
   * **Validates: Requirements 5.6**
   */
  it('does not render ThinkingSection when thinking_content is undefined', () => {
    const message: ChatMessageRecord = {
      id: 'msg-004',
      session_id: 'test-session',
      role: 'assistant',
      content: 'Response with no thinking field.',
      created_at: '2024-01-01T10:03:00Z',
    };

    const { queryByRole } = render(<MessageBubble message={message} />);

    expect(queryByRole('button', { name: '思考プロセスの表示切り替え' })).not.toBeInTheDocument();
  });

  /**
   * ストリーミング中のメッセージで streamingThinkingContent が ThinkingSection に反映される
   *
   * **Validates: Requirements 2.2, 5.1**
   */
  it('renders ThinkingSection with streamingThinkingContent during streaming', () => {
    const message: ChatMessageRecord = {
      id: 'msg-005',
      session_id: 'test-session',
      role: 'assistant',
      content: '',
      thinking_content: null,
      created_at: '2024-01-01T10:04:00Z',
    };

    // ストリーミング中の状態を設定
    useChatStore.setState({
      isStreaming: true,
      streamingThinkingContent: 'Streaming thinking content in progress...',
      messages: [message],
    });

    const { getByText } = render(<MessageBubble message={message} />);

    // ストリーミング中は ThinkingSection が展開状態で表示
    expect(getByText('Streaming thinking content in progress...')).toBeInTheDocument();
    expect(getByText('思考中...')).toBeInTheDocument();
  });

  /**
   * user ロールのメッセージには ThinkingSection が表示されない
   * （thinking_content はアシスタントメッセージ専用）
   *
   * **Validates: Requirements 5.1**
   */
  it('does not render ThinkingSection for user messages', () => {
    const message: ChatMessageRecord = {
      id: 'msg-006',
      session_id: 'test-session',
      role: 'user',
      content: 'User message',
      created_at: '2024-01-01T10:05:00Z',
    };

    const { queryByRole } = render(<MessageBubble message={message} />);

    // ユーザーメッセージにはThinkingSectionのトグルボタンが存在しない
    expect(queryByRole('button', { name: '思考プロセスの表示切り替え' })).not.toBeInTheDocument();
  });
});
