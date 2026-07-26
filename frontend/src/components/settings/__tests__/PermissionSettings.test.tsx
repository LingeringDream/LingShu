import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiFetch } from '../../../lib/api';
import { PermissionSettings } from '../PermissionSettings';

vi.mock('../../../lib/api', () => ({
  apiFetch: vi.fn(),
}));

const mockedApiFetch = vi.mocked(apiFetch);

function jsonResponse(body: unknown): Response {
  return {
    ok: true,
    status: 200,
    json: () => Promise.resolve(body),
  } as Response;
}

const defaultPermissions = {
  l0_enabled: true,
  l1_web_search: false,
  l1_calendar: false,
  l1_require_confirmation: true,
  l2_automation: false,
  l2_whitelist_only: true,
  l2_whitelist: [],
  l3_accessibility: false,
  l4_autonomous: false,
};

describe('PermissionSettings', () => {
  beforeEach(() => {
    mockedApiFetch.mockReset();
  });

  it('shows and persists the web-search permission toggle', async () => {
    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/permissions' && !init?.method) {
        return jsonResponse(defaultPermissions);
      }
      if (input === '/api/v1/permissions' && init?.method === 'PATCH') {
        return jsonResponse({ ...defaultPermissions, l1_web_search: true });
      }
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<PermissionSettings />);
    fireEvent.click(screen.getByText('权限分级'));

    const title = await screen.findByText('L1 · 联网搜索');
    const checkbox = title.closest('label')?.querySelector('input');
    expect(checkbox).not.toBeNull();
    expect(checkbox).not.toBeChecked();
    fireEvent.click(checkbox!);

    await waitFor(() => {
      expect(mockedApiFetch).toHaveBeenCalledWith(
        '/api/v1/permissions',
        expect.objectContaining({
          method: 'PATCH',
          body: JSON.stringify({ l1_web_search: true }),
        }),
      );
    });
    expect(checkbox).toBeChecked();
  });
});
