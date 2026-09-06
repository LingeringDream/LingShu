import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TeamWorkspace } from '../TeamWorkspace';
import { useCloudAccountStore } from '../../../stores/cloudAccountStore';
import { cloudAccountClient } from '../../../lib/cloudAccount';

vi.mock('../../../lib/cloudAccount', () => ({ cloudAccountClient: { teamRequest: vi.fn() } }));

describe('team product gate', () => {
  beforeEach(() => {
    vi.mocked(cloudAccountClient.teamRequest).mockResolvedValue(new Response('[]'));
    useCloudAccountStore.setState({ initialized: true, initialize: vi.fn(), account: {
      user: { id: 'test', email: 'test@example.com', display_name: 'Test', email_verified_at: '', created_at: '' },
      device: { id: 'test', name: 'Test', platform: 'web' }, subscription: null,
    } });
  });
  it.each([undefined, false, true])('gates creation but preserves joining when entitled=%s', async enabled => {
    const account = useCloudAccountStore.getState().account!;
    if (enabled !== undefined) useCloudAccountStore.setState({ account: { ...account, entitlements: {
      effective_plan: enabled ? 'team' : 'free', features: [
        { code: 'team_create', name: '创建团队', plans: ['team'], included: enabled, available: true, enabled },
      ],
    } } });
    render(<TeamWorkspace />);
    await waitFor(() => expect(cloudAccountClient.teamRequest).toHaveBeenCalled());
    if (enabled) expect(screen.getByRole('button', { name: '创建团队' })).toBeEnabled();
    else expect(screen.getByRole('button', { name: '创建团队' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '加入团队' })).toBeEnabled();
  });

  it('explains that an invited Team member cannot create another team', async () => {
    const account = useCloudAccountStore.getState().account!;
    useCloudAccountStore.setState({ account: { ...account, entitlements: {
      effective_plan: 'team', access_source: 'team_membership', features: [
        { code: 'team_create', name: '创建团队', plans: ['team'], included: false, available: true, enabled: false },
      ],
    } } });
    render(<TeamWorkspace />);
    await waitFor(() => expect(cloudAccountClient.teamRequest).toHaveBeenCalled());
    expect(screen.getByRole('button', { name: '创建团队' })).toBeDisabled();
    expect(screen.getByText(/当前为受邀团队成员/)).toBeInTheDocument();
  });
});
