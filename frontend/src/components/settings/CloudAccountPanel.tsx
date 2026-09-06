import { useState, type FormEvent } from 'react';
import { cloudErrorMessage, openCloudAccount } from '../../lib/cloudAccount';
import { getPlatformCapabilities, isTauri } from '../../lib/tauri';
import { useCloudAccountStore } from '../../stores/cloudAccountStore';
import './CloudAccountPanel.css';

export function CloudAccountPanel() {
  const { account, busy, initialized, error, login, logout, reload, browserPending, loginWithBrowser, cancelBrowserLogin } = useCloudAccountStore();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [deviceName, setDeviceName] = useState('我的灵枢电脑');
  const [linkError, setLinkError] = useState<string | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const submittedPassword = password;
    setPassword('');
    const capabilities = await getPlatformCapabilities();
    await login(email, submittedPassword, deviceName, capabilities.platform);
  }

  async function open(path: '/account' | '/billing' | '/devices') {
    setLinkError(null);
    try { await openCloudAccount(path); }
    catch (failure) { setLinkError(cloudErrorMessage(failure)); }
  }

  return (
    <section className="dashboard-card page-tool-card narrow-tool-card cloud-account-panel" aria-labelledby="cloud-account-title">
      <div>
        <p className="eyebrow">LingShu Account</p>
        <h2 id="cloud-account-title">云账户</h2>
        <p className="cloud-account-muted">通过网页验证登录，或使用邮箱和密码，管理你的账户、订阅与设备。</p>
      </div>
      {(error || linkError) && <p className="cloud-account-error" role="alert">{error || linkError}</p>}
      {!initialized ? <p role="status">正在恢复云账户…</p> : account ? (
        <>
          <div className="cloud-account-profile">
            <span className="cloud-account-avatar" aria-hidden="true">{account.user.display_name.slice(0, 1)}</span>
            <div><h3>{account.user.display_name}</h3><p>{account.user.email}</p><span className="cloud-account-verified">邮箱已验证</span></div>
          </div>
          <dl className="cloud-account-details">
            <div><dt>当前订阅</dt><dd>{account.subscription?.plan_name || '暂未获取'}{account.subscription && !account.subscription.entitlement_active ? '（权益未生效）' : ''}</dd></div>
            <div><dt>当前设备</dt><dd>{account.device.name}</dd></div>
            <div><dt>设备平台</dt><dd>{account.device.platform}</dd></div>
            {account.subscription?.current_period_end && <div><dt>订阅到期</dt><dd>{new Date(account.subscription.current_period_end).toLocaleDateString('zh-CN')}</dd></div>}
          </dl>
          {account.entitlements && <div className="cloud-account-note" aria-label="版本权益">
            <strong>当前生效版本：{account.entitlements.effective_plan.toUpperCase()}</strong>
            <dl className="cloud-account-details">{account.entitlements.features.map(feature => <div key={feature.code}>
              <dt>{feature.name}</dt><dd>{!feature.available ? '尚未开放' : feature.enabled ? '可用' : `需要 ${feature.plans.map(plan => plan.toUpperCase()).join(' / ')}`}</dd>
            </div>)}</dl>
            <p>受邀加入团队不要求个人购买团队版，团队内操作仍受成员角色限制。</p>
          </div>}
          {account.subscriptionError && <p className="cloud-account-muted" role="status">订阅信息暂未更新：{account.subscriptionError}</p>}
          <div className="cloud-account-actions">
            <button className="cloud-account-primary" type="button" onClick={() => void open('/account')}>打开网页账户中心</button>
            <button type="button" onClick={() => void open('/billing')}>订阅与账单</button>
            <button type="button" onClick={() => void open('/devices')}>管理设备</button>
          </div>
          <div className="cloud-account-actions">
            <button type="button" disabled={busy} onClick={() => void reload()}>{busy ? '正在处理…' : '刷新账户信息'}</button>
            <button type="button" disabled={busy} onClick={() => void logout()}>退出云账户</button>
          </div>
          <p className="cloud-account-muted">网页与软件共用账户，各自管理登录会话。网页中可以查看或撤销这台设备。</p>
        </>
      ) : (
        <>
          {isTauri() && <div className="cloud-account-note">
            <strong>使用浏览器登录</strong>
            <p>打开账户网页后，已登录的账号会自动用于软件；尚未登录时，在网页完成登录即可。</p>
            <div className="cloud-account-actions">
              <button className="cloud-account-primary" type="button" disabled={busy} onClick={() => void loginWithBrowser(deviceName)}>{browserPending ? '等待网页验证…' : '通过网页登录'}</button>
              {browserPending && <button type="button" onClick={cancelBrowserLogin}>取消网页登录</button>}
            </div>
            {browserPending && <p role="status">请在打开的浏览器中完成登录，软件会自动接收结果。此请求 5 分钟内有效。</p>}
          </div>}
          <form className="cloud-account-form" onSubmit={(event) => void submit(event)}>
            <label htmlFor="cloud-email">邮箱<input id="cloud-email" type="email" autoComplete="username" required maxLength={254} value={email} onChange={(event) => setEmail(event.target.value)} disabled={busy} /></label>
            <label htmlFor="cloud-password">密码<input id="cloud-password" type="password" autoComplete="current-password" required maxLength={128} value={password} onChange={(event) => setPassword(event.target.value)} disabled={busy} /></label>
            <label htmlFor="cloud-device">设备名称<input id="cloud-device" autoComplete="off" required maxLength={100} value={deviceName} onChange={(event) => setDeviceName(event.target.value)} disabled={busy} /><span>这个名称会显示在网页的设备列表中。</span></label>
            <div className="cloud-account-actions">
              <button className="cloud-account-primary" type="submit" disabled={busy}>{busy ? '正在连接…' : '登录云账户'}</button>
              <button type="button" onClick={() => void open('/account')}>前往网页注册</button>
              {error && <button type="button" disabled={busy} onClick={() => void reload()}>重试恢复登录</button>}
            </div>
          </form>
          <p className="cloud-account-muted">新账号需先在网页注册并完成邮箱验证，再回到软件登录。{isTauri() ? '登录状态将安全保存，重启软件后自动恢复。' : '浏览器开发模式仅在当前页面保留登录状态。'}</p>
        </>
      )}
      <div className="cloud-account-note"><strong>本地数据由你掌握</strong><p>云账户用于身份、订阅和设备管理。聊天与记忆仍保存在本机，云端同步尚未开放；登录或退出不会迁移、删除本地数据。</p></div>
    </section>
  );
}
