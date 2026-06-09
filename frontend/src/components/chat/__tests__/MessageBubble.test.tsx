import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MessageBubble, type Message } from '../MessageBubble';

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
});
