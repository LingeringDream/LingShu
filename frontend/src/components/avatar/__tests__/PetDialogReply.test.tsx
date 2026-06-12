import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

import { PetDialogReply } from '../PetDialogReply';

describe('PetDialogReply', () => {
  it('renders long markdown replies in a scrollable region', () => {
    render(<PetDialogReply content={'**结论**\n\n- 第一条\n- 第二条\n- 第三条'} />);

    const region = screen.getByRole('region', { name: '宠物回复' });
    expect(region).toHaveStyle({ overflowY: 'auto' });
    expect(region).toHaveStyle({ maxHeight: '150px' });
    expect(screen.getByText('结论').tagName).toBe('STRONG');
    expect(screen.getByText('第三条')).toBeInTheDocument();
  });

  it('hides the expand affordance when the reply fits', () => {
    // jsdom reports scrollHeight === clientHeight === 0 (no layout), so a
    // short reply is treated as non-overflowing.
    render(<PetDialogReply content={'短回复'} onExpand={() => {}} />);
    expect(screen.queryByRole('button', { name: /查看完整回复/ })).toBeNull();
  });

  it('shows the expand affordance and calls onExpand when the reply overflows', () => {
    // Force an overflow by stubbing the layout metrics the component reads.
    const sh = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'scrollHeight');
    const ch = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientHeight');
    Object.defineProperty(HTMLElement.prototype, 'scrollHeight', { configurable: true, get: () => 300 });
    Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 150 });
    try {
      const onExpand = vi.fn();
      render(<PetDialogReply content={'很长很长的回复内容'} onExpand={onExpand} />);
      const button = screen.getByRole('button', { name: /查看完整回复/ });
      fireEvent.click(button);
      expect(onExpand).toHaveBeenCalledOnce();
    } finally {
      if (sh) Object.defineProperty(HTMLElement.prototype, 'scrollHeight', sh);
      if (ch) Object.defineProperty(HTMLElement.prototype, 'clientHeight', ch);
    }
  });
});
