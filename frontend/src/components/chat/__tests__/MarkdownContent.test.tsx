import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { MarkdownContent } from '../MarkdownContent';

describe('MarkdownContent', () => {
  it('renders assistant markdown formatting', () => {
    render(<MarkdownContent content="**bold** text" compact />);

    expect(screen.getByText('bold').tagName).toBe('STRONG');
    expect(screen.getByText('text')).toBeInTheDocument();
  });
});
