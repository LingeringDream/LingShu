import { beforeEach, describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MessageBubble, type Message } from '../MessageBubble';
import { postSignal } from '../../../lib/api';

vi.mock('../../../lib/api', () => ({
  apiFetch: vi.fn(),
  postSignal: vi.fn().mockResolvedValue(undefined),
}));

const mockedPostSignal = vi.mocked(postSignal);

function makeMessage(overrides: Partial<Message> = {}): Message {
  return {
    id: 'test-1',
    role: 'user',
    content: 'Hello world',
    timestamp: new Date('2026-06-01T12:00:00Z'),
    ...overrides,
  };
}

describe('MessageBubble', () => {
  beforeEach(() => {
    mockedPostSignal.mockClear();
  });

  it('renders user message content', () => {
    render(<MessageBubble message={makeMessage({ role: 'user', content: 'Hello' })} />);
    expect(screen.getByText('Hello')).toBeInTheDocument();
  });

  it('renders assistant message with markdown', () => {
    const msg = makeMessage({ role: 'assistant', content: '**bold** text' });
    render(<MessageBubble message={msg} />);
    // ReactMarkdown renders bold as <strong>
    expect(screen.getByText('bold')).toBeInTheDocument();
    expect(screen.getByText('text')).toBeInTheDocument();
  });

  it('renders empty assistant message without crashing', () => {
    const { container } = render(
      <MessageBubble message={makeMessage({ role: 'assistant', content: '' })} />
    );
    // Should render without crashing — the message div should exist
    expect(container.querySelector('[style*="word-break"]')).toBeTruthy();
  });

  it('renders user message with pre-wrap whitespace', () => {
    render(<MessageBubble message={makeMessage({ role: 'user', content: 'line1\nline2' })} />);
    const span = screen.getByText(/line1/);
    expect(span).toBeInTheDocument();
    // The span should preserve newlines
    expect(span.style.whiteSpace).toBe('pre-wrap');
  });

  it('marks the selected style feedback chip as active', async () => {
    render(
      <MessageBubble
        message={makeMessage({
          role: 'assistant',
          content: 'Short answer',
          dbId: '0d9a76f7-75f7-47a2-8d46-80cfa00f7e40',
        })}
      />,
    );

    fireEvent.click(screen.getByTitle('太短了'));

    await waitFor(() => {
      expect(screen.getByTitle('太短了')).toHaveStyle({ background: 'var(--accent)' });
    });
    expect(screen.getByTitle('太长了')).not.toHaveStyle({ background: 'var(--accent)' });
  });

  it('keeps style signal metadata unchanged and requests regeneration for the selected style', async () => {
    const onStyleRegenerate = vi.fn();

    render(
      <MessageBubble
        message={makeMessage({
          role: 'assistant',
          content: 'Short answer',
          dbId: '0d9a76f7-75f7-47a2-8d46-80cfa00f7e40',
        })}
        onStyleRegenerate={onStyleRegenerate}
      />,
    );

    fireEvent.click(screen.getByTitle('太正式'));

    await waitFor(() => {
      expect(mockedPostSignal).toHaveBeenCalledWith({
        event_type: 'reply_style_tag',
        entity_type: 'message',
        entity_id: '0d9a76f7-75f7-47a2-8d46-80cfa00f7e40',
        metadata: { tag: 'too_formal' },
      });
    });
    expect(onStyleRegenerate).toHaveBeenCalledWith(
      'test-1',
      'too_formal',
    );
  });

  it('shows enhanced collaboration progress and expands a member draft', () => {
    render(
      <MessageBubble
        message={makeMessage({
          role: 'assistant',
          content: '最终答案',
          enhance: {
            phase: 'done',
            aggregatorLabel: 'OpenAI · gpt-4o',
            members: [
              {
                id: '0',
                label: 'DeepSeek · deepseek-chat',
                provider: 'deepseek',
                status: 'done',
                draft: '候选答案',
              },
              {
                id: '1',
                label: 'Ollama · qwen',
                provider: 'ollama',
                status: 'failed',
                error: '超时',
              },
            ],
          },
        })}
      />,
    );

    expect(screen.getByText('由 2 个模型增强·查看过程')).toBeInTheDocument();
    expect(screen.getByText('✦增强')).toBeInTheDocument();

    fireEvent.click(screen.getByText('由 2 个模型增强·查看过程'));

    expect(screen.getByText('正在综合各方观点')).toBeInTheDocument();
    fireEvent.click(screen.getByText('DeepSeek · deepseek-chat'));
    expect(screen.getByText('候选答案')).toBeInTheDocument();
    expect(screen.getByText('超时')).toBeInTheDocument();
  });
});
