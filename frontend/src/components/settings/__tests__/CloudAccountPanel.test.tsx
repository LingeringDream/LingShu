import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { CloudAccountPanel } from '../CloudAccountPanel';
import { useCloudAccountStore } from '../../../stores/cloudAccountStore';

vi.mock('../../../lib/tauri', () => ({
  isTauri: () => true,
  getPlatformCapabilities: async () => ({ platform: 'windows' }),
}));

describe('CloudAccountPanel', () => {
  beforeEach(() => {
    useCloudAccountStore.setState({ account: null, busy: false, initialized: true, error: null, login: vi.fn(), logout: vi.fn(), reload: vi.fn(), browserPending: false, loginWithBrowser: vi.fn(), cancelBrowserLogin: vi.fn() });
  });

  it('starts browser verification without asking for a password', () => {
    render(<CloudAccountPanel />);
    fireEvent.click(screen.getByRole('button', { name: '通过网页登录' }));
    expect(useCloudAccountStore.getState().loginWithBrowser).toHaveBeenCalledWith('我的灵枢电脑');
    expect(useCloudAccountStore.getState().login).not.toHaveBeenCalled();
  });

  it('allows cancellation while waiting for browser verification', () => {
    useCloudAccountStore.setState({ browserPending: true, busy: true });
    render(<CloudAccountPanel />);
    expect(screen.getByRole('status')).toHaveTextContent('软件会自动接收结果');
    fireEvent.click(screen.getByRole('button', { name: '取消网页登录' }));
    expect(useCloudAccountStore.getState().cancelBrowserLogin).toHaveBeenCalled();
  });

  it('submits website credentials with the device name and clears the password input', async () => {
    render(<CloudAccountPanel />);
    fireEvent.change(screen.getByLabelText('邮箱'), { target: { value: 'alice@example.com' } });
    fireEvent.change(screen.getByLabelText('密码'), { target: { value: 'a-private-password' } });
    fireEvent.click(screen.getByRole('button', { name: '登录云账户' }));
    await waitFor(() => expect(useCloudAccountStore.getState().login).toHaveBeenCalledWith('alice@example.com', 'a-private-password', '我的灵枢电脑', 'windows'));
    expect(screen.getByLabelText('密码')).toHaveValue('');
    expect(screen.getByText(/云端同步尚未开放/)).toBeInTheDocument();
  });

  it('shows errors and a recovery action after an offline restart', () => {
    useCloudAccountStore.setState({ error: '无法连接云账户服务' });
    render(<CloudAccountPanel />);
    expect(screen.getByRole('alert')).toHaveTextContent('无法连接');
    fireEvent.click(screen.getByRole('button', { name: '重试恢复登录' }));
    expect(useCloudAccountStore.getState().reload).toHaveBeenCalled();
  });

  it('shows the actual account and subscription and supports logout', () => {
    useCloudAccountStore.setState({ account: {
      user: { id: '1', email: 'alice@example.com', display_name: 'Alice', email_verified_at: '', created_at: '' },
      device: { id: '1', name: '工作电脑', platform: 'windows' },
      entitlements: { effective_plan: 'free', features: [
        { code: 'local_mcp', name: '本地 MCP', plans: ['free'], included: true, available: true, enabled: true },
        { code: 'cloud_sync', name: '云同步', plans: ['pro'], included: false, available: false, enabled: false },
        { code: 'team_create', name: '创建团队', plans: ['team'], included: false, available: true, enabled: false },
      ] },
      subscription: { plan_code: 'free', plan_name: '免费版', status: 'active', current_period_end: null, entitlement_active: true },
    } });
    render(<CloudAccountPanel />);
    expect(screen.getByText('alice@example.com')).toBeInTheDocument();
    expect(screen.getByText('免费版 Free')).toBeInTheDocument();
    expect(screen.getByText('可用')).toBeInTheDocument();
    expect(screen.getByText('尚未开放')).toBeInTheDocument();
    expect(screen.getByText('需要 TEAM')).toBeInTheDocument();
    expect(screen.getByText('工作电脑')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '退出云账户' }));
    expect(useCloudAccountStore.getState().logout).toHaveBeenCalled();
  });

  it('shows an invited free account as Team while retaining member context', () => {
    useCloudAccountStore.setState({ account: {
      user: { id: '1', email: 'member@example.com', display_name: 'Member', email_verified_at: '', created_at: '' },
      device: { id: '1', name: '团队电脑', platform: 'windows' },
      entitlements: { effective_plan: 'team', access_source: 'team_membership', features: [
        { code: 'team_create', name: '创建团队', plans: ['team'], included: false, available: true, enabled: false },
      ] },
      subscription: { plan_code: 'free', plan_name: '免费版', status: 'active', current_period_end: null, entitlement_active: true },
    } });
    render(<CloudAccountPanel />);
    expect(screen.getByText('团队版 Team（受邀成员）')).toBeInTheDocument();
    expect(screen.getByText(/当前生效版本：团队版 Team（受邀成员）/)).toBeInTheDocument();
  });
});
