import { render, cleanup, fireEvent } from '@testing-library/react';
import { describe, it, expect, afterEach } from 'vitest';
import '@testing-library/jest-dom';

import { ThinkingSection } from './ThinkingSection';

describe('ThinkingSection - Unit Tests', () => {
  afterEach(() => {
    cleanup();
  });

  /**
   * デフォルト折り畳み状態の確認
   * isStreaming=false, defaultExpanded=false の場合、コンテンツは非表示
   *
   * **Validates: Requirements 5.1, 5.2**
   */
  it('should be collapsed by default when not streaming and not defaultExpanded', () => {
    const { queryByText, getByRole } = render(
      <ThinkingSection thinkingContent="テスト思考内容" />
    );

    // トグルボタンは存在する
    const toggleButton = getByRole('button', { name: '思考プロセスの表示切り替え' });
    expect(toggleButton).toBeInTheDocument();
    expect(toggleButton).toHaveAttribute('aria-expanded', 'false');

    // コンテンツは非表示
    expect(queryByText('テスト思考内容')).not.toBeInTheDocument();
  });

  /**
   * トグル動作の確認
   * クリックで展開→再クリックで折り畳み
   *
   * **Validates: Requirements 5.3**
   */
  it('should toggle content visibility when clicking the toggle button', () => {
    const { queryByText, getByRole } = render(
      <ThinkingSection thinkingContent="展開テスト内容" />
    );

    const toggleButton = getByRole('button', { name: '思考プロセスの表示切り替え' });

    // 初期状態: 折り畳み
    expect(queryByText('展開テスト内容')).not.toBeInTheDocument();

    // クリックで展開
    fireEvent.click(toggleButton);
    expect(queryByText('展開テスト内容')).toBeInTheDocument();
    expect(toggleButton).toHaveAttribute('aria-expanded', 'true');

    // 再クリックで折り畳み
    fireEvent.click(toggleButton);
    expect(queryByText('展開テスト内容')).not.toBeInTheDocument();
    expect(toggleButton).toHaveAttribute('aria-expanded', 'false');
  });

  /**
   * redacted表示の確認 — isRedacted prop
   * isRedacted=trueの場合、「思考内容は非表示です」プレースホルダー表示
   *
   * **Validates: Requirements 6.3**
   */
  it('should display redacted placeholder when isRedacted is true', () => {
    const { getByText, getByRole } = render(
      <ThinkingSection
        thinkingContent="some content"
        isRedacted={true}
        defaultExpanded={true}
      />
    );

    const toggleButton = getByRole('button', { name: '思考プロセスの表示切り替え' });
    expect(toggleButton).toHaveAttribute('aria-expanded', 'true');

    expect(getByText('思考内容は非表示です')).toBeInTheDocument();
  });

  /**
   * redacted表示の確認 — [REDACTED_THINKING] マーカー検出
   * コンテンツに[REDACTED_THINKING]が含まれる場合もプレースホルダー表示
   *
   * **Validates: Requirements 6.3**
   */
  it('should display redacted placeholder when content contains [REDACTED_THINKING] marker', () => {
    const { getByText, queryByText } = render(
      <ThinkingSection
        thinkingContent="前のthinking[REDACTED_THINKING]後のthinking"
        defaultExpanded={true}
      />
    );

    expect(getByText('思考内容は非表示です')).toBeInTheDocument();
    // 元のコンテンツテキストは表示されない
    expect(queryByText('前のthinking')).not.toBeInTheDocument();
  });

  /**
   * thinking_content空時に非表示の確認
   * 親コンポーネントがthinkingContent空の場合にレンダリングしない責務を持つが、
   * コンポーネント自体に空文字列を渡した場合の動作確認
   *
   * **Validates: Requirements 5.6**
   */
  it('should render with empty content (parent responsibility to not render)', () => {
    const { container, getByRole } = render(
      <ThinkingSection thinkingContent="" defaultExpanded={true} />
    );

    // コンポーネント自体はレンダリングされるが、展開しても意味のあるコンテンツはない
    const toggleButton = getByRole('button', { name: '思考プロセスの表示切り替え' });
    expect(toggleButton).toBeInTheDocument();

    // 展開状態でもコンテンツ領域のテキストは空
    const contentArea = container.querySelector('.whitespace-pre-wrap');
    expect(contentArea).toBeInTheDocument();
    expect(contentArea?.textContent).toBe('');
  });

  /**
   * ストリーミング中はデフォルト展開
   * isStreaming=trueの場合、コンテンツは展開状態で表示
   *
   * **Validates: Requirements 5.1**
   */
  it('should be expanded by default when isStreaming is true', () => {
    const { getByText, getByRole } = render(
      <ThinkingSection thinkingContent="ストリーミング中の思考" isStreaming={true} />
    );

    const toggleButton = getByRole('button', { name: '思考プロセスの表示切り替え' });
    expect(toggleButton).toHaveAttribute('aria-expanded', 'true');

    // コンテンツが表示される
    expect(getByText('ストリーミング中の思考')).toBeInTheDocument();

    // 「思考中...」インジケーターも表示
    expect(getByText('思考中...')).toBeInTheDocument();
  });

  /**
   * 展開時にthinkingコンテンツテキストが表示される
   *
   * **Validates: Requirements 5.1, 5.2**
   */
  it('should show thinking content text when expanded', () => {
    const { getByText } = render(
      <ThinkingSection
        thinkingContent="これはモデルの思考プロセスです"
        defaultExpanded={true}
      />
    );

    expect(getByText('これはモデルの思考プロセスです')).toBeInTheDocument();
  });
});
