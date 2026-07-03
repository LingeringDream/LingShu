import { render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { PersonalityCenter } from '../PersonalityCenter';
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

describe('PersonalityCenter', () => {
  it('shows the latest personality evolution run', async () => {
    mockedApiFetch.mockImplementation(async (input) => {
      if (input === '/api/v1/personality/snapshots') {
        return jsonResponse([
          {
            id: 'snapshot-1',
            user_id: 'user-1',
            trait_values: {
              directness: 0.5,
              warmth: 0.5,
              proactivity: 0.5,
              risk_tolerance: 0.5,
              verbosity: 0.4,
              formality: 0.5,
              humor: 0.5,
            },
            change_reason: 'auto-evolution: 用户多次反馈回答太长',
            source_memory_ids: [],
            is_active: true,
            created_at: '2026-07-03T00:00:00Z',
          },
        ]);
      }
      if (input === '/api/v1/personality/snapshots/active') {
        return jsonResponse({
          id: 'snapshot-1',
          user_id: 'user-1',
          trait_values: {
            directness: 0.5,
            warmth: 0.5,
            proactivity: 0.5,
            risk_tolerance: 0.5,
            verbosity: 0.4,
            formality: 0.5,
            humor: 0.5,
          },
          change_reason: 'auto-evolution: 用户多次反馈回答太长',
          source_memory_ids: [],
          is_active: true,
          created_at: '2026-07-03T00:00:00Z',
        });
      }
      if (input === '/api/v1/evolution/runs?scope=personality&limit=1') {
        return jsonResponse([
          {
            id: 'run-1',
            user_id: 'user-1',
            scope: 'personality',
            status: 'applied',
            evidence_summary: {
              memory_count: 2,
              style_feedback: {
                window_days: 30,
                reply_thumb_up: 0,
                reply_thumb_down: 1,
                too_long: 3,
                too_short: 0,
                too_formal: 0,
                personality_slider_changed: 0,
              },
            },
            proposed_changes: [],
            applied_changes: [],
            skipped_reason: null,
            created_at: '2026-07-03T00:01:00Z',
          },
        ]);
      }
      if (input === '/api/v1/settings/role-prompt') {
        return jsonResponse({ role_prompt: '' });
      }
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<PersonalityCenter />);

    expect(await screen.findByText('最近一次自动调整')).toBeInTheDocument();
    expect(screen.getByText('已应用')).toBeInTheDocument();
    expect(screen.getByText('反馈样本: 4')).toBeInTheDocument();
  });

  it('does not block the page when evolution history is unavailable', async () => {
    mockedApiFetch.mockImplementation(async (input) => {
      if (input === '/api/v1/personality/snapshots') return jsonResponse([]);
      if (input === '/api/v1/personality/snapshots/active') return jsonResponse({}, false, 404);
      if (input === '/api/v1/evolution/runs?scope=personality&limit=1') {
        return jsonResponse({}, false, 404);
      }
      if (input === '/api/v1/settings/role-prompt') return jsonResponse({ role_prompt: '' });
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<PersonalityCenter />);

    await waitFor(() => {
      expect(screen.getByText(/人格中心/)).toBeInTheDocument();
    });
    expect(screen.queryByText('最近一次自动调整')).not.toBeInTheDocument();
  });
});
