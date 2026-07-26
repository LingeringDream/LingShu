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
      if (input === '/api/v1/settings/enhance' && !init?.method) {
        return jsonResponse({
          enabled_default: false,
          members: [],
          aggregator: null,
          per_model_timeout_ms: 30000,
          max_parallel: 4,
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

  it('saves multi-model enhancement settings from configured providers', async () => {
    const providers = {
      built_in: [],
      saved: [
        {
          id: 'provider-deepseek',
          provider_id: 'deepseek',
          display_name: 'DeepSeek',
          has_api_key: true,
          api_base_url: 'https://api.deepseek.com',
          models: [{ id: 'deepseek-chat', name: 'deepseek-chat', display_name: 'deepseek-chat' }],
          is_active: false,
          created_at: '2026-06-15T00:00:00Z',
        },
        {
          id: 'provider-openai',
          provider_id: 'openai',
          display_name: 'OpenAI',
          has_api_key: true,
          api_base_url: 'https://api.openai.com',
          models: [{ id: 'gpt-4o', name: 'gpt-4o', display_name: 'gpt-4o' }],
          is_active: true,
          created_at: '2026-06-15T00:00:00Z',
        },
      ],
    };

    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/settings/llm' && !init?.method) {
        return jsonResponse({
          provider: 'openai',
          api_key: '',
          api_base_url: 'https://api.openai.com',
          model: 'gpt-4o',
          temperature: 0.7,
          max_tokens: 4096,
          context_messages: 20,
        });
      }
      if (input === '/api/v1/settings/enhance' && !init?.method) {
        return jsonResponse({
          enabled_default: false,
          members: [],
          aggregator: null,
          per_model_timeout_ms: 30000,
          max_parallel: 4,
        });
      }
      if (input === '/api/v1/settings/providers' && !init?.method) {
        return jsonResponse(providers);
      }
      if (input === '/api/v1/settings/enhance' && init?.method === 'PATCH') {
        return jsonResponse(JSON.parse(String(init.body)));
      }
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<ChatSettings collapsed={false} />);

    fireEvent.click(await screen.findByLabelText('聊天默认开启'));
    fireEvent.click(await screen.findByLabelText('DeepSeek · deepseek-chat'));
    fireEvent.change(await screen.findByLabelText('整合模型'), {
      target: { value: 'openai::gpt-4o' },
    });
    fireEvent.click(screen.getByRole('button', { name: '保存增强配置' }));

    await waitFor(() => {
      expect(mockedApiFetch).toHaveBeenCalledWith(
        '/api/v1/settings/enhance',
        expect.objectContaining({
          method: 'PATCH',
          body: JSON.stringify({
            enabled_default: true,
            members: [{ provider_id: 'deepseek', model: 'deepseek-chat' }],
            aggregator: { provider_id: 'openai', model: 'gpt-4o' },
            per_model_timeout_ms: 30000,
            max_parallel: 4,
          }),
        }),
      );
    });
  });

  it('treats missing enhancement settings endpoint as unavailable instead of an error', async () => {
    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/settings/llm' && !init?.method) {
        return jsonResponse({
          provider: 'openai',
          api_key: '',
          api_base_url: 'https://api.openai.com',
          model: 'gpt-4o',
          temperature: 0.7,
          max_tokens: 4096,
          context_messages: 20,
        });
      }
      if (input === '/api/v1/settings/enhance' && !init?.method) {
        return jsonResponse({}, false, 404);
      }
      if (input === '/api/v1/settings/providers' && !init?.method) {
        return jsonResponse({ built_in: [], saved: [] });
      }
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<ChatSettings collapsed={false} />);

    expect(await screen.findByText('当前后端不支持增强配置，请重启或更新灵枢后端后再使用。')).toBeInTheDocument();
    expect(screen.queryByText(/加载增强配置失败/)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '保存增强配置' })).toBeDisabled();
  });
});
