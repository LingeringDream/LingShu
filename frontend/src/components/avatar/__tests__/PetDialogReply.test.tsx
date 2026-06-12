import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { PetDialogReply } from '../PetDialogReply';

describe('PetDialogReply', () => {
  it('renders long markdown replies in a scrollable region', () => {
    render(<PetDialogReply content={'**结论**\n\n- 第一条\n- 第二条\n- 第三条'} />);

    const region = screen.getByRole('region', { name: '宠物回复' });
    expect(region).toHaveStyle({ overflowY: 'auto' });
    expect(region).toHaveStyle({ maxHeight: '104px' });
    expect(screen.getByText('结论').tagName).toBe('STRONG');
    expect(screen.getByText('第三条')).toBeInTheDocument();
  });
});
