import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ChatInput } from '../ChatInput';
import { useChatStore } from '../../../stores/chatStore';
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

describe('ChatInput', () => {
  beforeEach(() => {
    mockedApiFetch.mockReset();
    useChatStore.setState({
      messages: [],
      isLoading: false,
      streamingId: null,
      sessionId: null,
      enhanceEnabled: false,
      enhanceDefaultLoaded: false,
    });
  });

  it('sends saved enhancement settings without opening a model selector', async () => {
    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/settings/enhance' && !init?.method) {
        return jsonResponse({
          enabled_default: false,
          members: [{ provider_id: 'deepseek', model: 'deepseek-chat' }],
          aggregator: { provider_id: 'openai', model: 'gpt-4o' },
          per_model_timeout_ms: 30000,
          max_parallel: 4,
        });
      }
      if (input === '/api/v1/settings/providers' && !init?.method) {
        return jsonResponse({
          built_in: [],
          saved: [
            {
              provider_id: 'deepseek',
              display_name: 'DeepSeek',
              models: [{ id: 'deepseek-chat', name: 'deepseek-chat', display_name: 'deepseek-chat' }],
              is_active: false,
            },
            {
              provider_id: 'openai',
              display_name: 'OpenAI',
              models: [{ id: 'gpt-4o', name: 'gpt-4o', display_name: 'gpt-4o' }],
              is_active: true,
            },
          ],
        });
      }
      throw new Error(`Unexpected request: ${input}`);
    });
    const onSend = vi.fn();

    render(<ChatInput onSend={onSend} />);

    fireEvent.click(screen.getByTitle('开启增强输出'));
    await waitFor(() => {
      expect(mockedApiFetch).toHaveBeenCalledWith('/api/v1/settings/providers');
    });
    expect(screen.queryByLabelText('DeepSeek · deepseek-chat')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('增强整合模型')).not.toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText('输入消息...'), {
      target: { value: '解释 MoA' },
    });
    await waitFor(() => {
      expect(screen.getByRole('button', { name: '发送' })).toBeEnabled();
    });
    fireEvent.click(screen.getByRole('button', { name: '发送' }));

    await waitFor(() => {
      expect(onSend).toHaveBeenCalledWith('解释 MoA', {
        members: [{ provider_id: 'deepseek', model: 'deepseek-chat' }],
        aggregator: { provider_id: 'openai', model: 'gpt-4o' },
        per_model_timeout_ms: 30000,
        max_parallel: 4,
      });
    });
  });

  it('auto-selects configured models when enhancement settings are empty', async () => {
    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/settings/enhance' && !init?.method) {
        return jsonResponse({
          enabled_default: false,
          members: [],
          aggregator: null,
          per_model_timeout_ms: 45000,
          max_parallel: 2,
        });
      }
      if (input === '/api/v1/settings/providers' && !init?.method) {
        return jsonResponse({
          built_in: [],
          saved: [
            {
              provider_id: 'deepseek',
              display_name: 'DeepSeek',
              is_active: true,
              models: [
                { id: 'deepseek-v4-flash', name: 'deepseek-v4-flash', display_name: 'deepseek-v4-flash' },
                { id: 'deepseek-v4-pro', name: 'deepseek-v4-pro', display_name: 'deepseek-v4-pro' },
              ],
            },
          ],
        });
      }
      throw new Error(`Unexpected request: ${input}`);
    });
    const onSend = vi.fn();

    render(<ChatInput onSend={onSend} />);

    fireEvent.click(screen.getByTitle('开启增强输出'));
    await waitFor(() => {
      expect(mockedApiFetch).toHaveBeenCalledWith('/api/v1/settings/providers');
    });
    expect(screen.queryByLabelText('DeepSeek · deepseek-v4-flash')).not.toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText('输入消息...'), {
      target: { value: '解释 MoA' },
    });
    await waitFor(() => {
      expect(screen.getByRole('button', { name: '发送' })).toBeEnabled();
    });
    fireEvent.click(screen.getByRole('button', { name: '发送' }));

    await waitFor(() => {
      expect(onSend).toHaveBeenCalledWith('解释 MoA', {
        members: [
          { provider_id: 'deepseek', model: 'deepseek-v4-flash' },
          { provider_id: 'deepseek', model: 'deepseek-v4-pro' },
        ],
        aggregator: { provider_id: 'deepseek', model: 'deepseek-v4-flash' },
        per_model_timeout_ms: 45000,
        max_parallel: 2,
      });
    });
  });
});
