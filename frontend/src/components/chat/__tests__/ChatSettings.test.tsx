import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { ChatSettings } from '../ChatSettings';
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

describe('ChatSettings', () => {
  it('reloads model settings after activating a provider', async () => {
    let llmFetchCount = 0;
    let providersFetchCount = 0;
    const inactiveProvider = {
      id: 'provider-1',
      provider_id: 'ollama',
      display_name: 'Ollama（本地）',
      has_api_key: false,
      api_base_url: 'http://localhost:11434',
      models: [{ id: 'gemma4:12b', name: 'gemma4:12b', display_name: 'gemma4:12b' }],
      is_active: false,
      created_at: '2026-06-15T00:00:00Z',
    };
    const activeProvider = { ...inactiveProvider, is_active: true };

    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/settings/llm' && !init?.method) {
        llmFetchCount += 1;
        return llmFetchCount === 1
          ? jsonResponse({
            provider: 'openai',
            api_key: '',
            api_base_url: 'https://api.deepseek.com',
            model: 'deepseek-v4-flash',
            temperature: 0.7,
            max_tokens: 10000,
            context_messages: 20,
          })
          : jsonResponse({
            provider: 'ollama',
            api_key: '',
            api_base_url: 'http://localhost:11434',
            model: 'gemma4:12b',
            temperature: 0.7,
            max_tokens: 4096,
            context_messages: 20,
          });
      }
      if (input === '/api/v1/settings/providers' && !init?.method) {
        providersFetchCount += 1;
        return providersFetchCount === 1
          ? jsonResponse({ built_in: [], saved: [inactiveProvider] })
          : jsonResponse({ built_in: [], saved: [activeProvider] });
      }
      if (input === '/api/v1/settings/providers/provider-1/activate' && init?.method === 'POST') {
        return jsonResponse(activeProvider);
      }
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<ChatSettings collapsed={false} />);

    await screen.findByText('deepseek-v4-flash');
    fireEvent.click(await screen.findByRole('button', { name: '激活' }));

    await waitFor(() => {
      expect(screen.getByText('ollama')).toBeInTheDocument();
      expect(screen.getAllByText('gemma4:12b').length).toBeGreaterThan(0);
    });
    expect(llmFetchCount).toBe(2);
  });
});
