import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { ProviderSelector } from '../ProviderSelector';
import { apiFetch } from '../../../lib/api';

vi.mock('../../../lib/api', () => ({
  apiFetch: vi.fn(),
}));

const mockedApiFetch = vi.mocked(apiFetch);

function jsonResponse(body: unknown, ok = true, status = 200): Response {
  return {
    ok,
    status,
    json: () => Promise.resolve(body),
  } as Response;
}

describe('ProviderSelector', () => {
  it('notifies the parent when an existing provider is activated', async () => {
    const onProviderChange = vi.fn();
    const provider = {
      id: 'provider-1',
      provider_id: 'ollama',
      display_name: 'Ollama（本地）',
      has_api_key: false,
      api_base_url: 'http://localhost:11434',
      models: [{ id: 'gemma4:12b', name: 'gemma4:12b', display_name: 'gemma4:12b' }],
      is_active: false,
      created_at: '2026-06-15T00:00:00Z',
    };
    const activated = { ...provider, is_active: true };

    mockedApiFetch
      .mockResolvedValueOnce(jsonResponse({ built_in: [], saved: [provider] }))
      .mockResolvedValueOnce(jsonResponse(activated))
      .mockResolvedValueOnce(jsonResponse({ built_in: [], saved: [activated] }));

    render(<ProviderSelector onProviderChange={onProviderChange} />);

    const activate = await screen.findByRole('button', { name: '激活' });
    fireEvent.click(activate);

    await waitFor(() => {
      expect(onProviderChange).toHaveBeenCalledWith('ollama', 'gemma4:12b');
    });
    expect(mockedApiFetch).toHaveBeenNthCalledWith(
      2,
      '/api/v1/settings/providers/provider-1/activate',
      { method: 'POST' },
    );
  });
});
