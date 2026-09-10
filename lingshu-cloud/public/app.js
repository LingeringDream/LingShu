const API = "/api/v1";
const SESSION_KEYS = {
  access: "lingshu.cloud.access_token",
  refresh: "lingshu.cloud.refresh_token",
};
const DEVICE_KEY = "lingshu.cloud.device_id";
const ACCOUNT_PAGES = {
  account: { path: "/account", title: "账户概览" },
  billing: { path: "/billing", title: "订阅与账单" },
  devices: { path: "/devices", title: "设备与会话" },
  docs: { path: "/docs", title: "使用文档" },
  performance: { path: "/performance", title: "性能评测" },
  download: { path: "/download", title: "下载中心" },
  team: { path: "/team", title: "团队工作区" },
};

const TEAM_ROLES = {
  owner: "所有者",
  admin: "管理员",
  editor: "编辑者",
  viewer: "只读成员",
};
const TEAM_ASSIGNABLE_ROLES = ["admin", "editor", "viewer"];
const TEAM_INVITABLE_ROLES = ["editor", "viewer"];

const ERROR_MESSAGES = {
  INVALID_EMAIL: "请输入有效的邮箱地址。",
  INVALID_PASSWORD: "密码格式不正确。注册密码至少需要 12 个字符。",
  INVALID_DISPLAY_NAME: "请输入显示名称。",
  INVALID_CREDENTIALS: "邮箱或密码不正确。",
  LOGIN_RATE_LIMITED: "登录尝试过于频繁，请稍后再试。",
  EMAIL_NOT_CONFIGURED: "邮件服务尚未配置，暂时无法发送验证邮件。",
  EMAIL_DELIVERY_FAILED: "验证邮件发送失败，请稍后再试。",
  INVALID_TOKEN: "验证链接无效或已过期。",
  INVALID_REFRESH_TOKEN: "登录状态已失效，请重新登录。",
  REFRESH_TOKEN_REUSE: "检测到会话异常，请重新登录。",
  REFRESH_TOKEN_EXPIRED: "登录状态已过期，请重新登录。",
  SESSION_REVOKED: "当前会话已失效，请重新登录。",
  DEVICE_NOT_FOUND: "设备不存在或已经退出。",
  BILLING_NOT_CONFIGURED: "订阅付款尚未开放。",
  BILLING_PROVIDER_UNAVAILABLE: "暂时无法连接账单服务，请稍后再试。",
  BILLING_PROVIDER_ERROR: "账单服务未能完成请求，请稍后再试。",
  BILLING_CUSTOMER_NOT_FOUND: "尚未建立账单资料，请先选择订阅方案。",
  SUBSCRIPTION_ALREADY_EXISTS: "你已有订阅，请通过账单门户管理。",
  INVALID_BILLING_INTERVAL: "请选择有效的订阅周期。",
};

const state = {
  accessToken: readSession(SESSION_KEYS.access),
  refreshToken: readSession(SESSION_KEYS.refresh),
  account: null,
  billing: null,
  devices: [],
  downloads: null,
  pendingDevice: null,
  refreshPromise: null,
  toastTimer: null,
  teams: [],
  teamSelected: "",
  teamMembers: [],
  teamMemories: [],
  teamKeys: [],
  teamInvites: [],
  teamAudit: [],
  teamBusy: false,
  teamLoadEpoch: 0,
  pendingInvites: [],
  pendingInvitesDismissed: false,
};

const elements = {
  authView: document.querySelector("#auth-view"),
  accountView: document.querySelector("#account-view"),
  accountTitle: document.querySelector("#account-title"),
  loginTab: document.querySelector("#login-tab"),
  registerTab: document.querySelector("#register-tab"),
  loginForm: document.querySelector("#login-form"),
  registerForm: document.querySelector("#register-form"),
  registrationSent: document.querySelector("#registration-sent"),
  registrationEmail: document.querySelector("#registration-email"),
  loginEmail: document.querySelector("#login-email"),
  loginPassword: document.querySelector("#login-password"),
  registerName: document.querySelector("#register-name"),
  registerEmail: document.querySelector("#register-email"),
  registerPassword: document.querySelector("#register-password"),
  loginError: document.querySelector("#login-error"),
  registerError: document.querySelector("#register-error"),
  notice: document.querySelector("#auth-notice"),
  noticeTitle: document.querySelector("#auth-notice-title"),
  noticeText: document.querySelector("#auth-notice-text"),
  profileName: document.querySelector("#profile-name"),
  profileEmail: document.querySelector("#profile-email"),
  currentPlanName: document.querySelector("#current-plan-name"),
  currentPlanDetail: document.querySelector("#current-plan-detail"),
  subscriptionStatus: document.querySelector("#subscription-status"),
  billingAvailability: document.querySelector("#billing-availability"),
  subscribeMonthly: document.querySelector("#subscribe-monthly"),
  subscribeMonth3: document.querySelector("#subscribe-month3"),
  subscribeHalfyear: document.querySelector("#subscribe-halfyear"),
  subscribeYearly: document.querySelector("#subscribe-yearly"),
  manageBilling: document.querySelector("#manage-billing"),
  currentDevice: document.querySelector("#current-device"),
  deviceList: document.querySelector("#device-list"),
  emptyDevices: document.querySelector("#empty-devices"),
  deviceCount: document.querySelector("#device-count"),
  downloadStatus: document.querySelector("#download-status"),
  revokeDialog: document.querySelector("#revoke-dialog"),
  revokeDeviceName: document.querySelector("#revoke-device-name"),
  confirmRevoke: document.querySelector("#confirm-revoke"),
  toast: document.querySelector("#toast"),
  sidebarTeam: document.querySelector("#sidebar-link-team"),
  teamSection: document.querySelector("#team-section"),
  teamCurrentUser: document.querySelector("#team-current-user"),
  teamNotice: document.querySelector("#team-notice"),
  teamSetup: document.querySelector("#team-setup"),
  teamCreateForm: document.querySelector("#team-create-form"),
  teamJoinForm: document.querySelector("#team-join-form"),
  teamName: document.querySelector("#team-name"),
  teamJoinToken: document.querySelector("#team-join-token"),
  teamEmptyHint: document.querySelector("#team-empty-hint"),
  teamSwitchRow: document.querySelector("#team-switch-row"),
  teamSwitch: document.querySelector("#team-switch"),
  teamContent: document.querySelector("#team-content"),
  teamMemberCount: document.querySelector("#team-member-count"),
  teamMembers: document.querySelector("#team-members"),
  teamInviteForm: document.querySelector("#team-invite-form"),
  teamInviteEmail: document.querySelector("#team-invite-email"),
  teamInviteRole: document.querySelector("#team-invite-role"),
  teamInvites: document.querySelector("#team-invites"),
  teamKeysCard: document.querySelector("#team-keys-card"),
  teamKeyForm: document.querySelector("#team-key-form"),
  teamKeyAgent: document.querySelector("#team-key-agent"),
  teamKeyPermission: document.querySelector("#team-key-permission"),
  teamKeyDays: document.querySelector("#team-key-days"),
  teamKeys: document.querySelector("#team-keys"),
  teamApiExample: document.querySelector("#team-api-example"),
  teamMemoryWriteForm: document.querySelector("#team-memory-write-form"),
  teamMemoryContent: document.querySelector("#team-memory-content"),
  teamMemoryScope: document.querySelector("#team-memory-scope"),
  teamMemorySearchForm: document.querySelector("#team-memory-search-form"),
  teamMemoryQuery: document.querySelector("#team-memory-query"),
  teamMemoryRefresh: document.querySelector("#team-memory-refresh"),
  teamGrantRow: document.querySelector("#team-grant-row"),
  teamGrantTarget: document.querySelector("#team-grant-target"),
  teamGrantAgent: document.querySelector("#team-grant-agent"),
  teamMemoriesEmpty: document.querySelector("#team-memories-empty"),
  teamMemories: document.querySelector("#team-memories"),
  teamAuditCard: document.querySelector("#team-audit-card"),
  teamAudit: document.querySelector("#team-audit"),
  teamSecret: document.querySelector("#team-secret"),
  teamSecretStrong: document.querySelector("#team-secret strong"),
  teamSecretCode: document.querySelector("#team-secret code"),
  teamSecretDismiss: document.querySelector("#team-secret-dismiss"),
  pendingInvitesBanner: document.querySelector("#pending-invites-banner"),
  pendingInvitesSummary: document.querySelector("#pending-invites-summary"),
  pendingInvitesList: document.querySelector("#pending-invites-list"),
  pendingInvitesClose: document.querySelector("#pending-invites-close"),
};

class ApiError extends Error {
  constructor(status, code, message) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

function readSession(key) {
  try {
    return sessionStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeSession(key, value) {
  try {
    if (value) sessionStorage.setItem(key, value);
    else sessionStorage.removeItem(key);
  } catch {
    // A privacy-restricted browser can still use the current in-memory session.
  }
}

function readDeviceId() {
  try {
    const value = localStorage.getItem(DEVICE_KEY) || "";
    return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value)
      ? value
      : null;
  } catch {
    return null;
  }
}

function writeDeviceId(value) {
  try {
    localStorage.setItem(DEVICE_KEY, value);
  } catch {
    // Device persistence is optional; authentication remains functional.
  }
}

function clearSession() {
  state.accessToken = null;
  state.refreshToken = null;
  state.account = null;
  state.billing = null;
  state.devices = [];
  state.downloads = null;
  state.teams = [];
  state.teamSelected = "";
  state.teamMembers = [];
  state.teamMemories = [];
  state.teamKeys = [];
  state.teamInvites = [];
  state.teamAudit = [];
  state.pendingInvites = [];
  state.pendingInvitesDismissed = false;
  writeSession(SESSION_KEYS.access, null);
  writeSession(SESSION_KEYS.refresh, null);
  applyTeamEntryVisibility();
}

function saveTokens(payload) {
  state.accessToken = payload.access_token;
  state.refreshToken = payload.refresh_token;
  writeSession(SESSION_KEYS.access, state.accessToken);
  writeSession(SESSION_KEYS.refresh, state.refreshToken);
  if (payload.device?.id) writeDeviceId(payload.device.id);
}

function platformInfo() {
  const source = `${navigator.userAgentData?.platform || ""} ${navigator.platform || ""} ${navigator.userAgent || ""}`.toLowerCase();
  if (source.includes("win")) return { code: "windows", name: "Windows 浏览器" };
  if (source.includes("iphone") || source.includes("ipad")) return { code: "ios", name: "iPhone / iPad" };
  if (source.includes("mac")) return { code: "macos", name: "Mac 浏览器" };
  if (source.includes("android")) return { code: "android", name: "Android 浏览器" };
  if (source.includes("linux")) return { code: "linux", name: "Linux 浏览器" };
  return { code: "web", name: "网页浏览器" };
}

async function parseResponse(response) {
  if (response.status === 204) return null;
  const contentType = response.headers.get("content-type") || "";
  const payload = contentType.includes("application/json")
    ? await response.json().catch(() => ({}))
    : {};
  if (!response.ok) {
    const code = payload?.error?.code || "REQUEST_FAILED";
    const fallback = response.status >= 500 ? "服务暂时不可用，请稍后再试。" : "请求未能完成。";
    throw new ApiError(response.status, code, ERROR_MESSAGES[code] || payload?.error?.message || fallback);
  }
  return payload;
}

async function request(path, options = {}) {
  const headers = new Headers(options.headers || {});
  if (options.body && !headers.has("Content-Type")) headers.set("Content-Type", "application/json");
  const response = await fetch(path, { ...options, headers });
  return parseResponse(response);
}

async function refreshSession() {
  if (state.refreshPromise) return state.refreshPromise;

  state.refreshPromise = (async () => {
    try {
      // The HttpOnly first-party session is shared by browser tabs. Minting a
      // short access token does not rotate it or race another tab's refresh.
      try {
        const shared = await request(`${API}/auth/browser-session`, { method: "POST", body: "{}" });
        saveTokens(shared);
        return true;
      } catch (error) {
        if (!(error instanceof ApiError) || error.status !== 401) throw error;
      }
      // Migrate a previously signed-in tab which predates browser cookies.
      if (!state.refreshToken) return false;
      const payload = await request(`${API}/auth/refresh`, {
        method: "POST",
        body: JSON.stringify({ refresh_token: state.refreshToken }),
      });
      saveTokens(payload);
      return true;
    } catch {
      clearSession();
      return false;
    } finally {
      state.refreshPromise = null;
    }
  })();
  return state.refreshPromise;
}

async function authorizedRequest(path, options = {}, canRetry = true) {
  if (!state.accessToken && !(await refreshSession())) {
    throw new ApiError(401, "UNAUTHORIZED", "请先登录。 ");
  }
  const tokenUsed = state.accessToken;
  const headers = new Headers(options.headers || {});
  headers.set("Authorization", `Bearer ${tokenUsed}`);
  if (options.body && !headers.has("Content-Type")) headers.set("Content-Type", "application/json");
  const response = await fetch(path, { ...options, headers });
  if (response.status === 401 && canRetry) {
    const tokenWasAlreadyRotated = tokenUsed !== state.accessToken;
    if (tokenWasAlreadyRotated || (await refreshSession())) {
      return authorizedRequest(path, options, false);
    }
  }
  return parseResponse(response);
}

function setButtonLoading(button, loading) {
  button.disabled = loading;
  button.classList.toggle("is-loading", loading);
  button.setAttribute("aria-busy", String(loading));
}

function showFieldError(element, message) {
  element.textContent = message;
  element.hidden = !message;
}

function showNotice(title, text, type = "success") {
  elements.noticeTitle.textContent = title;
  elements.noticeText.textContent = text;
  elements.notice.classList.toggle("is-error", type === "error");
  elements.notice.hidden = false;
}

function hideNotice() {
  elements.notice.hidden = true;
}

function showToast(message, type = "success") {
  window.clearTimeout(state.toastTimer);
  elements.toast.querySelector("p").textContent = message;
  elements.toast.classList.toggle("is-error", type === "error");
  elements.toast.hidden = false;
  state.toastTimer = window.setTimeout(() => {
    elements.toast.hidden = true;
  }, 4200);
}

function setAuthMode(mode, { focus = true, keepNotice = false } = {}) {
  const isLogin = mode === "login";
  elements.loginTab.classList.toggle("is-active", isLogin);
  elements.registerTab.classList.toggle("is-active", !isLogin);
  elements.loginTab.setAttribute("aria-selected", String(isLogin));
  elements.registerTab.setAttribute("aria-selected", String(!isLogin));
  elements.loginForm.hidden = !isLogin;
  elements.registerForm.hidden = isLogin;
  elements.registrationSent.hidden = true;
  if (!keepNotice) hideNotice();
  showFieldError(elements.loginError, "");
  showFieldError(elements.registerError, "");
  if (focus) {
    window.requestAnimationFrame(() => {
      (isLogin ? elements.loginEmail : elements.registerName).focus();
    });
  }
}

function showRegistrationSent(email) {
  elements.loginForm.hidden = true;
  elements.registerForm.hidden = true;
  elements.registrationEmail.textContent = email;
  elements.registrationSent.hidden = false;
}

function finishBoot() {
  document.documentElement.classList.remove("is-booting");
}

function showAuth() {
  elements.accountView.hidden = true;
  elements.authView.hidden = false;
  document.title = "登录 · 灵枢账户";
  finishBoot();
}

function accountPageFromPath(pathname) {
  const path = pathname.replace(/\/$/, "") || "/";
  const entry = Object.entries(ACCOUNT_PAGES).find(([, page]) => page.path === path);
  if (entry) return entry[0];
  // Deep-link target for the team-invite email lands at /team/invite/<uuid>.
  // Surface the team page so the "Pending invites" banner is in view; auto-
  // accept logic lives in `loadAccount`.
  if (path === "/team" || path.startsWith("/team/invite/")) return "team";
  return "account";
}

function pendingInviteFromLocation() {
  const match = location.pathname.match(/^\/team\/invite\/([0-9a-f-]{36})$/i);
  return match ? match[1] : null;
}

function requestedAccountPage() {
  return accountPageFromPath(location.pathname);
}

function renderAccountPage(pageName = requestedAccountPage()) {
  const page = ACCOUNT_PAGES[pageName] || ACCOUNT_PAGES.account;
  document.querySelectorAll("[data-page-panel]").forEach((panel) => {
    panel.hidden = panel.dataset.pagePanel !== pageName;
  });
  document.querySelectorAll(".sidebar-link[data-page]").forEach((link) => {
    const active = link.dataset.page === pageName;
    link.classList.toggle("is-active", active);
    if (active) link.setAttribute("aria-current", "page");
    else link.removeAttribute("aria-current");
  });
  elements.accountTitle.textContent = "账户中心";
  document.title = `${page.title} · 灵枢`;
}

function showAccount() {
  elements.authView.hidden = true;
  elements.accountView.hidden = false;
  const pageName = requestedAccountPage();
  if (location.pathname === "/") {
    history.replaceState({}, "", `${ACCOUNT_PAGES.account.path}${location.search}${location.hash}`);
  }
  renderAccountPage(pageName);
  window.scrollTo({ top: 0 });
  finishBoot();
  showTeamRequiredNotice();
}

// The worker redirects /team here when the visitor has no team access. The flag
// is consumed once so a later reload does not repeat the message.
function showTeamRequiredNotice() {
  const query = new URLSearchParams(location.search);
  if (query.get("team") !== "required") return;
  query.delete("team");
  const search = query.toString();
  history.replaceState({}, "", `${location.pathname}${search ? `?${search}` : ""}${location.hash}`);
  showToast("团队功能需要团队版订阅，当前账户尚未开通。", "error");
}

function deviceIcon(platform) {
  const normalized = String(platform || "").toLowerCase();
  const mobile = normalized.includes("ios") || normalized.includes("android") || normalized.includes("phone");
  const span = document.createElement("span");
  span.className = "device-icon";
  span.setAttribute("aria-hidden", "true");
  span.innerHTML = mobile
    ? '<svg viewBox="0 0 24 24"><rect x="7" y="2" width="10" height="20" rx="2"/><path d="M10 5h4M11 19h2"/></svg>'
    : '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="13" rx="2"/><path d="M8 21h8M10 17l-1 4M14 17l1 4"/></svg>';
  return span;
}

function friendlyPlatform(platform) {
  const normalized = String(platform || "").toLowerCase();
  if (normalized.includes("win")) return "Windows";
  if (normalized.includes("mac")) return "macOS";
  if (normalized.includes("ios")) return "iOS";
  if (normalized.includes("android")) return "Android";
  if (normalized.includes("linux")) return "Linux";
  return "网页";
}

function formatActivity(value) {
  const time = Date.parse(value);
  if (!Number.isFinite(time)) return "未知";
  const elapsed = Date.now() - time;
  if (elapsed < 60_000) return "刚刚";
  if (elapsed < 3_600_000) return `${Math.max(1, Math.floor(elapsed / 60_000))} 分钟前`;
  if (elapsed < 86_400_000) return `${Math.max(1, Math.floor(elapsed / 3_600_000))} 小时前`;
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(time));
}

function renderCurrentDevice(device) {
  elements.currentDevice.replaceChildren();
  elements.currentDevice.append(deviceIcon(device.platform));
  const copy = document.createElement("div");
  const title = document.createElement("strong");
  title.textContent = `${device.name} · ${friendlyPlatform(device.platform)}`;
  const activity = document.createElement("small");
  activity.textContent = `最近活动：${formatActivity(device.last_seen_at)}`;
  copy.append(title, activity);
  const marker = document.createElement("span");
  marker.className = "current-label";
  marker.textContent = "此设备";
  elements.currentDevice.append(copy, marker);
}

function openRevokeDialog(device) {
  state.pendingDevice = device;
  elements.revokeDeviceName.textContent = `${device.name} · ${friendlyPlatform(device.platform)}`;
  elements.revokeDialog.showModal();
}

function renderDeviceRow(device) {
  const row = document.createElement("div");
  row.className = "device-row";
  row.setAttribute("role", "row");

  const main = document.createElement("div");
  main.className = "device-main";
  main.setAttribute("role", "cell");
  main.append(deviceIcon(device.platform));

  const copy = document.createElement("span");
  copy.className = "device-copy";
  const title = document.createElement("strong");
  title.textContent = `${device.name} · ${friendlyPlatform(device.platform)}`;
  const sessions = document.createElement("small");
  sessions.textContent = device.active_session_count > 0
    ? `${device.active_session_count} 个活动会话`
    : "没有活动会话";
  copy.append(title, sessions);
  main.append(copy);

  const activity = document.createElement("span");
  activity.className = "device-time";
  activity.setAttribute("role", "cell");
  activity.textContent = formatActivity(device.last_seen_at);

  const revoke = document.createElement("button");
  revoke.className = "revoke-button";
  revoke.type = "button";
  revoke.textContent = "退出此设备";
  revoke.setAttribute("role", "cell");
  revoke.addEventListener("click", () => openRevokeDialog(device));
  row.append(main, activity, revoke);
  return row;
}

function renderAccount() {
  const { user, device } = state.account;
  elements.profileName.textContent = user.display_name;
  elements.profileEmail.textContent = user.email;
  renderCurrentDevice(device);

  const others = state.devices.filter((entry) => entry.id !== device.id);
  elements.deviceList.replaceChildren(...others.map(renderDeviceRow));
  elements.emptyDevices.hidden = others.length !== 0;
  elements.deviceCount.textContent = `${others.length} 台设备`;
}

function formatBillingDate(value) {
  const time = Date.parse(value);
  if (!Number.isFinite(time)) return null;
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "long",
    day: "numeric",
  }).format(new Date(time));
}

function renderBilling() {
  const data = state.billing;
  if (!data) return;
  const subscription = data.subscription || {};
  const entitlements = data.entitlements || {};
  const billing = data.billing || {};
  const statusLabels = {
    active: "已生效",
    trialing: "试用中",
    past_due: "待付款",
    incomplete: "待完成",
    incomplete_expired: "未完成",
    canceled: "已取消",
    unpaid: "未付款",
    paused: "已暂停",
  };
  const effectivePlanNames = { free: "免费版", pro: "专业版 Pro", team: "团队版" };
  elements.currentPlanName.textContent = effectivePlanNames[entitlements.effective_plan]
    || subscription.plan_name
    || "免费版";
  const isInvitedTeamMember = entitlements.access_source === "team_membership";
  elements.subscriptionStatus.textContent = isInvitedTeamMember
    ? "受邀成员"
    : statusLabels[subscription.status] || "状态未知";
  elements.subscriptionStatus.dataset.status = isInvitedTeamMember
    ? "active"
    : subscription.status || "unknown";

  const periodEnd = formatBillingDate(subscription.current_period_end);
  if (isInvitedTeamMember) {
    elements.currentPlanDetail.textContent = "已加入团队版工作区；创建新的团队需要单独开通团队版订阅";
  } else if (subscription.plan_code === "team") {
    elements.currentPlanDetail.textContent = "团队版测试权益已生效，可使用团队工作区、成员权限、共享记忆与审计治理";
  } else if (subscription.cancel_at_period_end && periodEnd) {
    elements.currentPlanDetail.textContent = `已取消自动续费，可使用至 ${periodEnd}`;
  } else if (subscription.billing_interval && periodEnd) {
    const intervalLabels = {
      month: "1 个月时长卡",
      month3: "3 个月时长卡",
      halfyear: "6 个月时长卡",
      year: "1 年时长卡",
    };
    const interval = intervalLabels[subscription.billing_interval] || `${subscription.billing_interval} 时长卡`;
    elements.currentPlanDetail.textContent = `${interval}方案 · 当前周期至 ${periodEnd}`;
  } else {
    elements.currentPlanDetail.textContent = "本地核心功能永久免费，订阅只扩展云端能力";
  }

  const developer = (data.plans || []).find((plan) => plan.code === "developer");
  const hasManagedSubscription = ["developer", "team"].includes(subscription.plan_code) &&
    ["active", "trialing", "past_due", "incomplete"].includes(subscription.status);
  const teamCard = document.querySelector('[data-plan-card="team"]');
  if (teamCard) {
    const teamState = teamCard.querySelector(".plan-state");
    const teamPrice = teamCard.querySelector(".plan-price");
    const activeTeam = subscription.plan_code === "team" && subscription.entitlement_active;
    teamCard.classList.toggle("is-planned", !activeTeam);
    if (teamState) {
      teamState.textContent = activeTeam ? "当前生效" : "测试中";
      teamState.classList.toggle("is-live", activeTeam);
    }
    if (teamPrice) teamPrice.textContent = activeTeam ? "测试权益已生效" : "测试权益定向开放，暂不公开购买";
  }

  // interval → 按钮元素 映射（与后端 DEVELOPER_OPTIONS 对齐）
  const optionButtons = {
    month: elements.subscribeMonthly,
    month3: elements.subscribeMonth3,
    halfyear: elements.subscribeHalfyear,
    year: elements.subscribeYearly,
  };
  const fallbackLabels = {
    month: "购买 1 个月",
    month3: "购买 3 个月",
    halfyear: "购买 6 个月",
    year: "购买 1 年",
  };
  for (const [interval, button] of Object.entries(optionButtons)) {
    if (!button) continue;
    const option = developer?.options?.find((o) => o.interval === interval);
    if (!option) {
      button.hidden = true;
      continue;
    }
    button.hidden = false;
    button.textContent = option.label || fallbackLabels[interval];
    button.disabled = !option.available || hasManagedSubscription;
  }
  elements.manageBilling.hidden = !billing.customer_portal_available;

  if (subscription.plan_code === "team" && subscription.entitlement_active) {
    elements.billingAvailability.textContent = "团队版测试权益已生效；正式团队计费尚未开放，如需企业方案可联系商务邮箱。";
  } else if (hasManagedSubscription) {
    elements.billingAvailability.textContent = "已有付费权益，请使用“管理账单”查看订单与到期时间。";
  } else if (billing.configured) {
    elements.billingAvailability.textContent = "权益按购买时长生效，当前不自动续费，到期前会提醒。";
  } else {
    elements.billingAvailability.textContent = "支付通道尚未启用，当前不会产生任何费用。";
  }
  applyTeamEntryVisibility();
}

function applyTeamEntryVisibility() {
  if (!elements.sidebarTeam) return;
  const subscription = state.billing?.subscription;
  const ownsTeamPlan = Boolean(
    subscription &&
      subscription.plan_code === "team" &&
      subscription.entitlement_active,
  );
  // Members of any team workspace see the sidebar entry even when their own
  // plan is Free/Developer — the server-side /team gate keys off the same
  // membership check (see canAccessTeam in lingshu-cloud/src/index.ts), so the
  // link would otherwise be hidden on exactly the page it leads to.
  const isTeamMember = Array.isArray(state.teams) && state.teams.length > 0;
  elements.sidebarTeam.hidden = !(ownsTeamPlan || isTeamMember);
}

function showTeamNotice(message, kind = "error") {
  const node = elements.teamNotice;
  if (!node) return;
  if (!message) { node.hidden = true; node.textContent = ""; node.classList.remove("is-success"); return; }
  node.textContent = message;
  node.classList.toggle("is-success", kind === "success");
  node.hidden = false;
}

function setTeamBusy(busy) {
  state.teamBusy = busy;
  if (elements.teamSetup) elements.teamSetup.disabled = busy;
  if (elements.teamContent) elements.teamContent.disabled = busy;
  if (elements.teamSwitch) elements.teamSwitch.disabled = busy;
}

async function loadTeams() {
  const currentIdentity = state.account?.user?.id;
  state.teamLoadEpoch += 1;
  const epoch = state.teamLoadEpoch;
  state.teams = [];
  state.teamSelected = "";
  state.teamMembers = [];
  state.teamMemories = [];
  state.teamKeys = [];
  state.teamInvites = [];
  state.teamAudit = [];
  showTeamNotice("");
  hideTeamSecret();
  renderTeamSkeleton(currentIdentity);
  if (!currentIdentity) return;
  try {
    const payload = await authorizedRequest(`${API}/teams`);
    if (epoch !== state.teamLoadEpoch) return;
    state.teams = Array.isArray(payload?.teams) ? payload.teams : [];
    state.pendingInvites = Array.isArray(payload?.pending_invites_for_me)
      ? payload.pending_invites_for_me
      : [];
    state.teamSelected = state.teams[0]?.id ?? "";
    renderPendingInvites();
    renderTeamSkeleton(currentIdentity);
    applyTeamEntryVisibility();
    if (state.teamSelected) await loadTeamDetail();
  } catch (error) {
    if (epoch !== state.teamLoadEpoch) return;
    showTeamNotice(error instanceof ApiError ? error.message : "无法加载团队列表，请稍后重试。");
    renderTeamSkeleton(currentIdentity);
  }
}

async function loadPendingInvitesOnly() {
  // Lightweight fetch so the invitation banner shows on every dashboard page
  // without paying for the full team-detail cascade. Falls back silently on
  // auth errors — the regular loadTeams path will report them later.
  if (!state.account?.user?.id) {
    state.pendingInvites = [];
    renderPendingInvites();
    applyTeamEntryVisibility();
    return;
  }
  try {
    const payload = await authorizedRequest(`${API}/teams`);
    state.pendingInvites = Array.isArray(payload?.pending_invites_for_me)
      ? payload.pending_invites_for_me
      : [];
    state.teams = Array.isArray(payload?.teams) ? payload.teams : state.teams;
  } catch (error) {
    if (!(error instanceof ApiError)) return;
    // 401/403 surface through clearSession + showAuth, so the banner just stays empty.
    if (error.status !== 401 && error.status !== 403) return;
  }
  renderPendingInvites();
  applyTeamEntryVisibility();
}

function renderPendingInvites() {
  const banner = elements.pendingInvitesBanner;
  if (!banner) return;
  const invites = state.pendingInvites || [];
  const visible = state.pendingInvitesDismissed ? [] : invites;
  if (visible.length === 0) {
    banner.hidden = true;
    return;
  }
  banner.hidden = false;
  if (elements.pendingInvitesSummary) {
    const count = visible.length;
    elements.pendingInvitesSummary.textContent = count === 1
      ? "你有 1 个待处理的团队邀请。登录时已发送邮件给邀请方留的邮箱。"
      : `你有 ${count} 个待处理的团队邀请。邀请已同时通过邮件发送。`;
  }
  if (elements.pendingInvitesList) {
    elements.pendingInvitesList.replaceChildren();
    for (const invite of visible) {
      elements.pendingInvitesList.append(buildPendingInviteRow(invite));
    }
  }
}

function buildPendingInviteRow(invite) {
  const row = document.createElement("li");
  const info = document.createElement("div");
  info.className = "pending-invites-info";
  const title = document.createElement("strong");
  title.textContent = invite.team_name || "未命名团队";
  const meta = document.createElement("small");
  const roleLabel = TEAM_ROLES[invite.role] || invite.role;
  const expiresLabel = formatExpireLabel(invite.expires_at);
  meta.textContent = `角色：${roleLabel} · ${expiresLabel}`;
  info.append(title, meta);

  const actions = document.createElement("div");
  actions.className = "pending-invites-actions";
  const accept = document.createElement("button");
  accept.className = "primary-button compact-button";
  accept.type = "button";
  accept.textContent = "接受";
  accept.addEventListener("click", () => {
    void acceptInvite(invite.invite_id);
  });
  const dismiss = document.createElement("button");
  dismiss.className = "secondary-button compact-button";
  dismiss.type = "button";
  dismiss.textContent = "忽略";
  dismiss.addEventListener("click", () => {
    void dismissInvite(invite.invite_id);
  });
  actions.append(accept, dismiss);

  row.append(info, actions);
  return row;
}

function formatExpireLabel(expiresAt) {
  if (!expiresAt) return "7 天内有效";
  const target = Date.parse(expiresAt);
  if (!Number.isFinite(target)) return "7 天内有效";
  const ms = target - Date.now();
  if (ms <= 0) return "已过期";
  const days = Math.max(1, Math.ceil(ms / 86_400_000));
  return `还剩 ${days} 天有效`;
}

async function acceptInvite(inviteId) {
  if (!inviteId) return;
  try {
    const team = await teamRequest(`${API}/teams/invites/${encodeURIComponent(inviteId)}/accept`, undefined, "POST");
    state.pendingInvites = state.pendingInvites.filter((entry) => entry.invite_id !== inviteId);
    if (team && team.id) {
      state.teams = [...state.teams.filter((t) => t.id !== team.id), team];
      state.teamSelected = team.id;
    }
    renderPendingInvites();
    applyTeamEntryVisibility();
    showToast("已加入团队。");
    if (location.pathname !== "/team") {
      history.pushState({}, "", "/team");
      await loadAccount({ preserveView: true });
    }
  } catch (error) {
    showToast(error instanceof ApiError ? error.message : "接受邀请失败，请稍后重试。", "error");
    if (error instanceof ApiError && error.code === "INVITE_INVALID") {
      state.pendingInvites = state.pendingInvites.filter((entry) => entry.invite_id !== inviteId);
      renderPendingInvites();
    }
  }
}

async function dismissInvite(inviteId) {
  if (!inviteId) return;
  // Treat dismiss as user-driven hide: remove locally for this session; the
  // invite stays usable until it expires or the inviter revokes it.
  state.pendingInvites = state.pendingInvites.filter((entry) => entry.invite_id !== inviteId);
  renderPendingInvites();
}

async function loadTeamDetail() {
  if (!state.teamSelected) { renderTeamDetail(); return; }
  const epoch = ++state.teamLoadEpoch;
  const team = currentTeam();
  const isAdmin = isAdminRole(team?.role);
  const base = `${API}/teams/${state.teamSelected}`;
  try {
    const [members, memories] = await Promise.all([
      authorizedRequest(`${base}/members`),
      authorizedRequest(`${base}/memories`),
    ]);
    if (epoch !== state.teamLoadEpoch) return;
    state.teamMembers = Array.isArray(members) ? members : [];
    state.teamMemories = Array.isArray(memories) ? memories : [];
    state.teamKeys = [];
    state.teamInvites = [];
    state.teamAudit = [];
    renderTeamDetail();
    if (isAdmin) {
      const [keys, invites, audit] = await Promise.all([
        authorizedRequest(`${base}/keys`).catch(() => []),
        authorizedRequest(`${base}/invites`).catch(() => []),
        authorizedRequest(`${base}/audit`).catch(() => []),
      ]);
      if (epoch !== state.teamLoadEpoch) return;
      state.teamKeys = Array.isArray(keys) ? keys : [];
      state.teamInvites = Array.isArray(invites) ? invites : [];
      state.teamAudit = Array.isArray(audit) ? audit : [];
      renderTeamDetail();
    }
  } catch (error) {
    if (epoch !== state.teamLoadEpoch) return;
    showTeamNotice(error instanceof ApiError ? error.message : "无法加载团队详情，请稍后重试。");
  }
}

function currentTeam() {
  return state.teams.find((t) => t.id === state.teamSelected) || null;
}

function isAdminRole(role) {
  return role === "owner" || role === "admin";
}

function renderTeamSkeleton(currentIdentity) {
  const email = state.account?.user?.email;
  if (email && elements.teamCurrentUser) {
    elements.teamCurrentUser.textContent = email;
    elements.teamCurrentUser.hidden = false;
  } else if (elements.teamCurrentUser) {
    elements.teamCurrentUser.hidden = true;
  }
  if (!elements.teamSetup) return;
  elements.teamSetup.disabled = !currentIdentity || state.teamBusy;
}

function renderTeamDetail() {
  const team = currentTeam();
  const identity = state.account?.user?.id;
  const admin = isAdminRole(team?.role);
  if (!elements.teamSection) return;
  if (!team) {
    elements.teamEmptyHint.hidden = false;
    elements.teamSwitchRow.hidden = true;
    elements.teamContent.hidden = true;
    return;
  }
  elements.teamEmptyHint.hidden = true;
  elements.teamSwitchRow.hidden = false;
  elements.teamSwitch.replaceChildren();
  for (const t of state.teams) {
    const option = document.createElement("option");
    option.value = t.id;
    option.textContent = `${t.name} · ${TEAM_ROLES[t.role] ?? t.role}`;
    if (t.id === state.teamSelected) option.selected = true;
    elements.teamSwitch.append(option);
  }
  elements.teamContent.hidden = false;
  elements.teamContent.disabled = state.teamBusy;

  elements.teamMemberCount.textContent = String(state.teamMembers.length);
  renderMemberRows(team, identity, admin);
  elements.teamInviteForm.hidden = !admin;
  if (admin) {
    const owner = team.role === "owner";
    elements.teamInviteRole.replaceChildren();
    const roles = owner ? ["editor", "viewer", "admin"] : TEAM_INVITABLE_ROLES;
    for (const r of roles) {
      const option = document.createElement("option");
      option.value = r;
      option.textContent = TEAM_ROLES[r] ?? r;
      if (r === "editor") option.selected = true;
      elements.teamInviteRole.append(option);
    }
  }
  renderInviteRows();
  elements.teamKeysCard.hidden = !admin;
  if (admin) renderKeyRows();
  if (elements.teamApiExample && team.id) {
    elements.teamApiExample.textContent = `POST /api/v1/teams/${team.id}/search\nAuthorization: Bearer <Agent 凭证>\nContent-Type: application/json\n\n{"query":"项目约定","mode":"adaptive","top_k":5}`;
  }
  elements.teamMemoryWriteForm.hidden = team.role === "viewer";
  renderMemoryRows(team, identity);
  if (team.role !== "viewer" && admin) {
    elements.teamGrantRow.hidden = false;
    elements.teamGrantTarget.replaceChildren();
    const blank = document.createElement("option");
    blank.value = "";
    blank.textContent = "选择成员或输入 Agent 标识";
    elements.teamGrantTarget.append(blank);
    for (const m of state.teamMembers) {
      if (m.cloud_user_id === identity) continue;
      const option = document.createElement("option");
      option.value = `member-${m.cloud_user_id}`;
      option.textContent = m.email;
      elements.teamGrantTarget.append(option);
    }
  } else {
    elements.teamGrantRow.hidden = true;
  }
  elements.teamAuditCard.hidden = !admin;
  if (admin) renderAuditRows();
}

function renderMemberRows(team, identity, admin) {
  elements.teamMembers.replaceChildren();
  if (state.teamMembers.length === 0) {
    const empty = document.createElement("p");
    empty.className = "team-empty-hint";
    empty.textContent = "暂无成员。";
    elements.teamMembers.append(empty);
    return;
  }
  for (const m of state.teamMembers) {
    const row = document.createElement("div");
    row.className = "team-row";
    const left = document.createElement("div");
    const name = document.createElement("strong");
    name.textContent = m.email;
    const meta = document.createElement("span");
    meta.className = "team-row-role";
    meta.dataset.role = m.role;
    meta.textContent = TEAM_ROLES[m.role] ?? m.role;
    left.append(name, meta);
    row.append(left);

    const owner = team.role === "owner";
    if (owner && m.role !== "owner") {
      const select = document.createElement("select");
      select.setAttribute("aria-label", `调整 ${m.email} 的角色`);
      for (const r of TEAM_ASSIGNABLE_ROLES) {
        const option = document.createElement("option");
        option.value = r;
        option.textContent = TEAM_ROLES[r] ?? r;
        if (r === m.role) option.selected = true;
        select.append(option);
      }
      select.addEventListener("change", () => {
        const next = select.value;
        void act(async () => {
          await teamRequest(`${API}/teams/${state.teamSelected}/members/${m.cloud_user_id}`, { role: next }, "PATCH");
          const idx = state.teamMembers.findIndex((x) => x.cloud_user_id === m.cloud_user_id);
          if (idx >= 0) state.teamMembers[idx] = { ...state.teamMembers[idx], role: next };
          renderTeamDetail();
          showTeamNotice("已更新成员角色。", "success");
        });
      });
      row.append(select);
    } else {
      row.append(document.createElement("span"));
    }

    const canRemove = admin && m.role !== "owner" && (team.role === "owner" || m.role !== "admin");
    const remove = document.createElement("button");
    remove.type = "button";
    remove.textContent = "移除";
    remove.disabled = !canRemove;
    if (canRemove) {
      remove.addEventListener("click", () => {
        void act(async () => {
          await teamRequest(`${API}/teams/${state.teamSelected}/members/${m.cloud_user_id}`, undefined, "DELETE");
          await loadTeamDetail();
          showTeamNotice("成员已移除，后续请求将被拒绝。", "success");
        });
      });
    }
    row.append(remove);
    elements.teamMembers.append(row);
  }
}

function renderInviteRows() {
  elements.teamInvites.replaceChildren();
  if (state.teamInvites.length === 0) return;
  for (const invite of state.teamInvites) {
    const row = document.createElement("div");
    row.className = "team-row";
    const left = document.createElement("div");
    const email = document.createElement("strong");
    email.textContent = invite.email;
    const meta = document.createElement("small");
    meta.textContent = `${TEAM_ROLES[invite.role] ?? invite.role} · ${formatInviteStatus(invite)}`;
    left.append(email, meta);
    row.append(left);
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = "撤销邀请";
    button.disabled = Boolean(invite.used_at) || Boolean(invite.revoked_at);
    if (!button.disabled) {
      button.addEventListener("click", () => {
        void act(async () => {
          await teamRequest(`${API}/teams/${state.teamSelected}/invites/${invite.id}`, undefined, "DELETE");
          await loadTeamDetail();
          showTeamNotice("邀请已撤销。", "success");
        });
      });
    }
    row.append(document.createElement("span"));
    row.append(button);
    elements.teamInvites.append(row);
  }
}

function renderKeyRows() {
  elements.teamKeys.replaceChildren();
  if (state.teamKeys.length === 0) {
    const empty = document.createElement("p");
    empty.className = "team-empty-hint";
    empty.textContent = "尚未签发任何凭证。";
    elements.teamKeys.append(empty);
    return;
  }
  for (const key of state.teamKeys) {
    const row = document.createElement("div");
    row.className = "team-row";
    const left = document.createElement("div");
    const agent = document.createElement("strong");
    agent.textContent = key.agent_key;
    const meta = document.createElement("small");
    meta.textContent = `${key.permission === "read" ? "只读" : "读写"} · ${formatKeyStatus(key)}`;
    left.append(agent, meta);
    row.append(left);
    row.append(document.createElement("span"));
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = "撤销凭证";
    button.disabled = Boolean(key.revoked_at);
    if (!button.disabled) {
      button.addEventListener("click", () => {
        void act(async () => {
          await teamRequest(`${API}/teams/${state.teamSelected}/keys/${key.id}`, undefined, "DELETE");
          hideTeamSecret();
          await loadTeamDetail();
          showTeamNotice("凭证已撤销。", "success");
        });
      });
    }
    row.append(button);
    elements.teamKeys.append(row);
  }
}

function renderMemoryRows(team, identity) {
  const empty = state.teamMemories.length === 0;
  elements.teamMemoriesEmpty.hidden = !empty;
  elements.teamMemories.replaceChildren();
  if (empty) return;
  for (const memory of state.teamMemories) {
    const card = document.createElement("div");
    card.className = "team-memory";
    const meta = document.createElement("small");
    const scope = memory.memory_scope === "global" ? "团队公共" : memory.memory_scope === "private" ? "私有" : "定向共享";
    meta.textContent = `${scope} · ${memory.memory_layer}`;
    const body = document.createElement("p");
    body.textContent = memory.content;
    card.append(meta, body);
    if (team.role !== "viewer" && memory.owner_agent_id === `member-${identity}`) {
      const actions = document.createElement("div");
      actions.className = "team-memory-actions";
      const grant = document.createElement("button");
      grant.type = "button";
      grant.textContent = "授权给目标";
      const revoke = document.createElement("button");
      revoke.type = "button";
      revoke.textContent = "撤销目标授权";
      const targetValue = () => elements.teamGrantTarget?.value || elements.teamGrantAgent?.value.trim() || "";
      const refresh = () => { grant.disabled = !targetValue(); revoke.disabled = !targetValue(); };
      refresh();
      elements.teamGrantTarget?.addEventListener("change", refresh);
      elements.teamGrantAgent?.addEventListener("input", refresh);
      grant.addEventListener("click", () => {
        const target = targetValue();
        if (!target) return;
        void act(async () => {
          await teamRequest(`${API}/teams/${state.teamSelected}/memories/${memory.id}/grants`, { owner_agent_id: "ignored", grantee_agent_id: target, can_read: true });
          await loadTeamDetail();
          showTeamNotice("已改为定向共享，并授予目标读取权限。", "success");
        });
      });
      revoke.addEventListener("click", () => {
        const target = targetValue();
        if (!target) return;
        void act(async () => {
          await teamRequest(`${API}/teams/${state.teamSelected}/memories/${memory.id}/grants`, { owner_agent_id: "ignored", grantee_agent_id: target, can_read: false });
          await loadTeamDetail();
          showTeamNotice("已撤销目标读取授权。", "success");
        });
      });
      actions.append(grant, revoke);
      card.append(actions);
    }
    elements.teamMemories.append(card);
  }
}

function renderAuditRows() {
  elements.teamAudit.replaceChildren();
  if (state.teamAudit.length === 0) {
    const empty = document.createElement("p");
    empty.className = "team-empty-hint";
    empty.textContent = "暂无审计记录。";
    elements.teamAudit.append(empty);
    return;
  }
  const emails = new Map(state.teamMembers.map((m) => [m.cloud_user_id, m.email]));
  for (const entry of state.teamAudit) {
    const row = document.createElement("div");
    row.className = "team-audit-row";
    const time = document.createElement("time");
    time.textContent = formatDateTime(entry.created_at);
    const action = document.createElement("strong");
    action.textContent = entry.action;
    const target = document.createElement("code");
    target.textContent = entry.target || "—";
    const actor = document.createElement("small");
    actor.textContent = `操作身份：${emails.get(entry.actor) ?? entry.actor}`;
    row.append(time, action, target, actor);
    elements.teamAudit.append(row);
  }
}

function showTeamSecret(label, token) {
  if (!elements.teamSecret) return;
  elements.teamSecretStrong.textContent = label;
  elements.teamSecretCode.textContent = token;
  elements.teamSecret.hidden = false;
}

function hideTeamSecret() {
  if (!elements.teamSecret) return;
  elements.teamSecret.hidden = true;
  elements.teamSecretStrong.textContent = "";
  elements.teamSecretCode.textContent = "";
}

function formatInviteStatus(invite) {
  if (invite.used_at) return "已接受";
  if (invite.revoked_at) return "已撤销";
  if (Date.parse(invite.expires_at) < Date.now()) return "已过期";
  return "待接受";
}

function formatKeyStatus(key) {
  if (key.revoked_at) return "已撤销";
  if (Date.parse(key.expires_at) < Date.now()) return "已过期";
  return `有效至 ${new Date(key.expires_at).toLocaleDateString("zh-CN")}`;
}

function formatDateTime(value) {
  const time = Date.parse(value);
  if (!Number.isFinite(time)) return value || "—";
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric", month: "2-digit", day: "2-digit",
    hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false,
  }).format(new Date(time));
}

async function teamRequest(path, body, method = body === undefined ? "GET" : "POST") {
  const options = { method };
  if (body !== undefined) options.body = JSON.stringify(body);
  return authorizedRequest(path, options);
}

async function act(operation) {
  if (state.teamBusy) return;
  setTeamBusy(true);
  showTeamNotice("");
  try { await operation(); }
  catch (error) { showTeamNotice(error instanceof Error ? error.message : "操作失败。"); }
  finally { setTeamBusy(false); }
}

function bindTeamForms() {
  if (!elements.teamSection) return;
  elements.teamCreateForm?.addEventListener("submit", (event) => {
    event.preventDefault();
    const name = elements.teamName.value.trim();
    if (!name) return;
    void act(async () => {
      const created = await teamRequest(`${API}/teams`, { name });
      state.teams = [...state.teams, created];
      state.teamSelected = created.id;
      elements.teamName.value = "";
      await loadTeamDetail();
      showTeamNotice("团队已创建。", "success");
    });
  });
  elements.teamJoinForm?.addEventListener("submit", (event) => {
    event.preventDefault();
    const token = elements.teamJoinToken.value.trim();
    if (!token) return;
    void act(async () => {
      const joined = await teamRequest(`${API}/teams/join`, { token });
      state.teams = [...state.teams.filter((t) => t.id !== joined.id), joined];
      state.teamSelected = joined.id;
      elements.teamJoinToken.value = "";
      await loadTeamDetail();
      showTeamNotice("已加入团队。", "success");
    });
  });
  elements.teamSwitch?.addEventListener("change", () => {
    state.teamSelected = elements.teamSwitch.value;
    void loadTeamDetail();
  });
  elements.teamInviteForm?.addEventListener("submit", (event) => {
    event.preventDefault();
    const email = elements.teamInviteEmail.value.trim();
    if (!email) return;
    void act(async () => {
      const payload = await teamRequest(`${API}/teams/${state.teamSelected}/invites`, {
        email,
        role: elements.teamInviteRole.value,
      });
      showTeamSecret(`邀请 ${email}（7 天内有效）`, payload.token);
      elements.teamInviteEmail.value = "";
      await loadTeamDetail();
    });
  });
  elements.teamKeyForm?.addEventListener("submit", (event) => {
    event.preventDefault();
    const agent = elements.teamKeyAgent.value.trim();
    if (!agent) return;
    const permission = elements.teamKeyPermission.value;
    const days = Number(elements.teamKeyDays.value);
    void act(async () => {
      const payload = await teamRequest(`${API}/teams/${state.teamSelected}/keys`, {
        agent_key: agent,
        name: agent,
        permission,
        expires_in_days: days,
      });
      showTeamSecret(`Agent ${agent} 凭证`, payload.token);
      elements.teamKeyAgent.value = "";
      await loadTeamDetail();
    });
  });
  elements.teamMemoryWriteForm?.addEventListener("submit", (event) => {
    event.preventDefault();
    const content = elements.teamMemoryContent.value;
    if (!content) return;
    void act(async () => {
      await teamRequest(`${API}/teams/${state.teamSelected}/memories`, {
        content,
        memory_layer: "episode",
        memory_scope: elements.teamMemoryScope.value,
      });
      elements.teamMemoryContent.value = "";
      await loadTeamDetail();
      showTeamNotice("记忆已写入团队空间。", "success");
    });
  });
  elements.teamMemorySearchForm?.addEventListener("submit", (event) => {
    event.preventDefault();
    const query = elements.teamMemoryQuery.value.trim();
    if (!query) return;
    void act(async () => {
      const payload = await teamRequest(`${API}/teams/${state.teamSelected}/search`, {
        query, mode: "adaptive", top_k: 20,
      });
      state.teamMemories = Array.isArray(payload?.results) ? payload.results : [];
      renderTeamDetail();
      showTeamNotice(`检索到 ${state.teamMemories.length} 条记忆。`, "success");
    });
  });
  elements.teamMemoryRefresh?.addEventListener("click", () => {
    void loadTeamDetail();
  });
  elements.teamSecretDismiss?.addEventListener("click", hideTeamSecret);
}

function validDriveUrl(value) {
  if (typeof value !== "string" || value.trim() === "") return null;
  try {
    const url = new URL(value);
    return url.protocol === "https:" ? url.href : null;
  } catch {
    return null;
  }
}

function renderDownloads() {
  const manifest = state.downloads || {};
  const downloads = manifest.downloads || {};
  let availableCount = 0;
  document.querySelectorAll("[data-download-platform]").forEach((button) => {
    const entry = downloads[button.dataset.downloadPlatform] || {};
    const url = validDriveUrl(entry.url);
    button.disabled = !url;
    button.textContent = url ? "打开网盘" : "网盘下载";
    button.onclick = url ? () => window.open(url, "_blank", "noopener,noreferrer") : null;
    if (url) availableCount += 1;
  });
  document.querySelectorAll("[data-sha-platform]").forEach((sha) => {
    const entry = downloads[sha.dataset.shaPlatform] || {};
    if (typeof entry.sha256 === "string" && /^[0-9a-f]{64}$/i.test(entry.sha256)) {
      sha.textContent = `SHA-256：${entry.sha256}`;
      sha.hidden = false;
    } else {
      sha.hidden = true;
    }
  });
  document.querySelectorAll("[data-access-code-platform]").forEach((codeElement) => {
    const entry = downloads[codeElement.dataset.accessCodePlatform] || {};
    const code = (typeof entry.access_code === "string" && entry.access_code.trim())
      ? entry.access_code.trim()
      : (typeof manifest.access_code === "string" ? manifest.access_code.trim() : "");
    if (validDriveUrl(entry.url) && code) {
      codeElement.textContent = `网盘提取码：${code}`;
      codeElement.hidden = false;
    } else {
      codeElement.hidden = true;
    }
  });

  const title = elements.downloadStatus.querySelector("strong");
  const text = elements.downloadStatus.querySelector("span");
  if (availableCount > 0) {
    title.textContent = `${manifest.provider_name || "网盘"}下载已开放`;
    text.textContent = "请使用各安装包卡片中显示的提取码。";
    elements.downloadStatus.classList.add("is-ready");
  } else {
    title.textContent = "网盘链接整理中";
    text.textContent = "安装包统一通过网盘分发，网盘地址配置后即可下载。";
    elements.downloadStatus.classList.remove("is-ready");
  }
}

async function loadDownloads() {
  try {
    const response = await fetch("/downloads.json", { cache: "no-store" });
    state.downloads = response.ok ? await response.json() : null;
  } catch {
    state.downloads = null;
  }
  renderDownloads();
}

async function loadAccount({ preserveView = false } = {}) {
  const pageName = requestedAccountPage();
  if (preserveView && state.account) {
    renderAccountPage(pageName);
    window.scrollTo({ top: 0 });
  }
  // Always pull subscription so the sidebar team entry reflects the visitor's
  // plan from the very first page they land on (not only after opening the
  // billing tab). Failures fall back to the previous value (or stay null on
  // first load) and never block the rest of the dashboard.
  const [account, devices, billing] = await Promise.all([
    authorizedRequest(`${API}/account`),
    pageName === "devices" ? authorizedRequest(`${API}/account/devices`) : Promise.resolve(null),
    authorizedRequest(`${API}/account/subscription`).catch(() => null),
  ]);
  state.account = account;
  if (devices) state.devices = devices.devices || [];
  if (billing) state.billing = billing;
  state.pendingInvitesDismissed = false;
  renderAccount();
  if (billing) renderBilling();
  applyTeamEntryVisibility();
  if (pageName === "download") await loadDownloads();
  if (pageName === "team") {
    await loadTeams();
  } else {
    // Lightweight banner fetch so the "待处理的团队邀请" strip shows on every
    // dashboard page (not only `/team`). Falls back silently if the user is
    // logged out or the session is mid-refresh.
    void loadPendingInvitesOnly();
  }
  if (pageName === requestedAccountPage()) showAccount();
  // Email deep-link: if the URL is `/team/invite/<uuid>` and the matching row
  // is among the currently-pending invites, attempt the server-side accept
  // immediately so the visitor lands on a successful join.
  maybeAutoAcceptDeepLink();
}

async function maybeAutoAcceptDeepLink() {
  if (!state.account?.user?.id) return;
  const inviteId = pendingInviteFromLocation();
  if (!inviteId) return;
  // Wait one microtask cycle for the lightweight banner fetch to populate
  // pendingInvites (they often arrive in the same request but we want the
  // accept call to be made with the user's intent once we know the invite is
  // really theirs).
  for (let attempt = 0; attempt < 8; attempt += 1) {
    const match = state.pendingInvites.find((row) => row.invite_id === inviteId);
    if (match) {
      history.replaceState({}, "", "/team");
      await acceptInvite(inviteId);
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 120));
  }
  // No matching invite was found (already accepted, expired, or revoked).
  // Surface this to the user instead of leaving them stranded on `/team`.
  history.replaceState({}, "", "/team");
  showToast("这条邀请已经处理过或已失效，请在侧栏「团队」页面查看当前的邀请列表。", "error");
}

async function navigateAccount(url, { replace = false } = {}) {
  const pageName = accountPageFromPath(url.pathname);
  const route = ACCOUNT_PAGES[pageName];
  if (!route || route.path !== url.pathname.replace(/\/$/, "")) {
    location.assign(url.href);
    return;
  }
  if (!state.account) {
    location.assign(url.href);
    return;
  }

  const nextPath = `${route.path}${url.search}${url.hash}`;
  history[replace ? "replaceState" : "pushState"]({}, "", nextPath);
  renderAccountPage(pageName);
  window.scrollTo({ top: 0 });
  try {
    await loadAccount({ preserveView: true });
  } catch {
    clearSession();
    history.replaceState({}, "", "/");
    showAuth();
    setAuthMode("login", { focus: false });
    showNotice("登录状态已失效", "请重新登录后继续。", "error");
  }
}

async function startCheckout(interval, button) {
  setButtonLoading(button, true);
  try {
    const payload = await authorizedRequest(`${API}/billing/checkout`, {
      method: "POST",
      body: JSON.stringify({ interval }),
    });
    location.assign(payload.url);
  } catch (error) {
    showToast(error instanceof ApiError ? error.message : "无法创建订阅，请稍后重试。", "error");
    setButtonLoading(button, false);
  }
}

async function openBillingPortal() {
  setButtonLoading(elements.manageBilling, true);
  try {
    const payload = await authorizedRequest(`${API}/billing/portal`, {
      method: "POST",
      body: "{}",
    });
    location.assign(payload.url);
  } catch (error) {
    showToast(error instanceof ApiError ? error.message : "无法打开账单门户，请稍后重试。", "error");
    setButtonLoading(elements.manageBilling, false);
  }
}

async function handleLogin(event) {
  event.preventDefault();
  showFieldError(elements.loginError, "");
  if (!elements.loginForm.reportValidity()) return;
  const button = elements.loginForm.querySelector("button[type='submit']");
  setButtonLoading(button, true);
  const platform = platformInfo();
  try {
    const payload = await request(`${API}/auth/login`, {
      method: "POST",
      body: JSON.stringify({
        email: elements.loginEmail.value.trim(),
        password: elements.loginPassword.value,
        device_id: readDeviceId() || undefined,
        device_name: platform.name,
        platform: platform.code,
      }),
    });
    saveTokens(payload);
    elements.loginPassword.value = "";
    if (isDesktopLogin()) { await finishDesktopLogin(); return; }
    await loadAccount();
    showToast("登录成功，欢迎回来。");
  } catch (error) {
    showFieldError(elements.loginError, error instanceof ApiError ? error.message : "无法连接服务，请检查网络后重试。");
  } finally {
    setButtonLoading(button, false);
  }
}

async function handleRegister(event) {
  event.preventDefault();
  showFieldError(elements.registerError, "");
  if (!elements.registerForm.reportValidity()) return;
  const button = elements.registerForm.querySelector("button[type='submit']");
  setButtonLoading(button, true);
  const email = elements.registerEmail.value.trim();
  try {
    await request(`${API}/auth/register`, {
      method: "POST",
      body: JSON.stringify({
        display_name: elements.registerName.value.trim(),
        email,
        password: elements.registerPassword.value,
      }),
    });
    elements.loginEmail.value = email;
    elements.registerPassword.value = "";
    showRegistrationSent(email);
  } catch (error) {
    showFieldError(elements.registerError, error instanceof ApiError ? error.message : "无法连接服务，请检查网络后重试。");
  } finally {
    setButtonLoading(button, false);
  }
}

async function handleLogout() {
  const button = document.querySelector("#logout-button");
  button.disabled = true;
  try {
    if (state.refreshToken) {
      await request(`${API}/auth/logout`, {
        method: "POST",
        body: JSON.stringify({ refresh_token: state.refreshToken }),
      });
    } else if (state.accessToken) {
      await authorizedRequest(`${API}/auth/logout`, { method: "POST", body: "{}" }, false);
    }
  } catch {
    // Local sign-out must succeed even when the remote session already expired.
  } finally {
    clearSession();
    button.disabled = false;
    history.replaceState({}, "", "/");
    showAuth();
    setAuthMode("login", { focus: false });
    showNotice("已退出登录", "此浏览器中的云账户会话已清除。", "success");
  }
}

async function handleRevoke() {
  if (!state.pendingDevice) return;
  elements.confirmRevoke.disabled = true;
  try {
    await authorizedRequest(`${API}/account/devices/${encodeURIComponent(state.pendingDevice.id)}`, {
      method: "DELETE",
    });
    const name = state.pendingDevice.name;
    state.pendingDevice = null;
    elements.revokeDialog.close();
    await loadAccount();
    showToast(`${name} 已退出。`);
  } catch (error) {
    elements.revokeDialog.close();
    showToast(error instanceof ApiError ? error.message : "设备退出失败，请稍后重试。", "error");
  } finally {
    elements.confirmRevoke.disabled = false;
  }
}

async function verifyFromUrl(token) {
  showAuth();
  setAuthMode("login", { focus: false });
  showNotice("正在验证邮箱", "请稍候…", "success");
  try {
    const payload = await request(`${API}/auth/verify-email`, {
      method: "POST",
      body: JSON.stringify({ token }),
    });
    if (payload.user?.email) elements.loginEmail.value = payload.user.email;
    showNotice("邮箱验证成功", "你的账户已启用，现在可以登录。", "success");
  } catch (error) {
    showNotice("邮箱验证失败", error instanceof ApiError ? error.message : "无法连接服务，请稍后重试。", "error");
  } finally {
    history.replaceState({}, "", `${location.pathname}${location.hash}`);
  }
}

function bindEvents() {
  elements.loginTab.addEventListener("click", () => setAuthMode("login"));
  elements.registerTab.addEventListener("click", () => setAuthMode("register"));
  document.querySelectorAll("[data-switch]").forEach((button) => {
    button.addEventListener("click", () => setAuthMode(button.dataset.switch));
  });
  document.querySelector("#dismiss-notice").addEventListener("click", hideNotice);
  document.querySelector("#back-to-login").addEventListener("click", () => setAuthMode("login"));
  elements.pendingInvitesClose?.addEventListener("click", () => {
    state.pendingInvitesDismissed = true;
    renderPendingInvites();
  });
  elements.loginForm.addEventListener("submit", handleLogin);
  elements.registerForm.addEventListener("submit", handleRegister);
  document.querySelector("#logout-button").addEventListener("click", handleLogout);
  elements.subscribeMonthly.addEventListener("click", () => {
    void startCheckout("month", elements.subscribeMonthly);
  });
  elements.subscribeMonth3.addEventListener("click", () => {
    void startCheckout("month3", elements.subscribeMonth3);
  });
  elements.subscribeHalfyear.addEventListener("click", () => {
    void startCheckout("halfyear", elements.subscribeHalfyear);
  });
  elements.subscribeYearly.addEventListener("click", () => {
    void startCheckout("year", elements.subscribeYearly);
  });
  elements.manageBilling.addEventListener("click", () => {
    void openBillingPortal();
  });
  document.querySelector("#refresh-devices").addEventListener("click", async (event) => {
    const button = event.currentTarget;
    button.disabled = true;
    try {
      await loadAccount();
      showToast("设备列表已刷新。");
    } catch (error) {
      showToast(error instanceof ApiError ? error.message : "刷新失败，请稍后重试。", "error");
    } finally {
      button.disabled = false;
    }
  });
  document.querySelector("#cancel-revoke").addEventListener("click", () => {
    state.pendingDevice = null;
  });
  elements.confirmRevoke.addEventListener("click", (event) => {
    event.preventDefault();
    void handleRevoke();
  });
  elements.revokeDialog.addEventListener("cancel", () => {
    state.pendingDevice = null;
  });

  document.querySelectorAll(".password-toggle").forEach((button) => {
    button.addEventListener("click", () => {
      const input = document.getElementById(button.dataset.target);
      const reveal = input.type === "password";
      input.type = reveal ? "text" : "password";
      button.setAttribute("aria-label", reveal ? "隐藏密码" : "显示密码");
    });
  });

  document.addEventListener("click", (event) => {
    if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const link = event.target.closest("a[href]");
    if (!link || link.target === "_blank") return;
    const url = new URL(link.href, location.href);
    if (url.origin === location.origin && url.pathname === location.pathname && url.search === location.search && url.hash) {
      const target = document.querySelector(url.hash);
      if (target) {
        event.preventDefault();
        history.replaceState({}, "", `${url.pathname}${url.search}${url.hash}`);
        target.scrollIntoView({ block: "start" });
        return;
      }
    }
    const pageName = accountPageFromPath(url.pathname);
    const route = ACCOUNT_PAGES[pageName];
    if (url.origin !== location.origin || !route || route.path !== url.pathname.replace(/\/$/, "")) return;
    if (!state.account) return;
    event.preventDefault();
    if (`${url.pathname}${url.search}${url.hash}` === `${location.pathname}${location.search}${location.hash}`) return;
    void navigateAccount(url);
  });

  window.addEventListener("popstate", () => {
    if (!state.account) return;
    if (location.pathname === "/") history.replaceState({}, "", ACCOUNT_PAGES.account.path);
    renderAccountPage();
    window.scrollTo({ top: 0 });
    void loadAccount({ preserveView: true });
  });

}

function isDesktopLogin() {
  return new URLSearchParams(location.search).get("desktop_login") === "1";
}

function showDesktopLogin(message) {
  elements.authView.hidden = true;
  elements.accountView.hidden = true;
  document.querySelector("#desktop-login-view").hidden = false;
  document.querySelector("#desktop-login-status").textContent = message;
  document.title = "网页登录 · 灵枢";
  finishBoot();
}

async function finishDesktopLogin() {
  const query = new URLSearchParams(location.search);
  const retry = document.querySelector("#desktop-login-retry");
  retry.hidden = true;
  showDesktopLogin("正在验证当前网页账号，完成后将自动返回灵枢软件…");
  try {
    const payload = Object.fromEntries(["redirect_uri", "state", "code_challenge", "code_challenge_method", "device_name", "platform", "device_id"]
      .filter(key => query.has(key)).map(key => [key, query.get(key)]));
    const result = await authorizedRequest(`${API}/auth/desktop/authorize`, { method: "POST", body: JSON.stringify(payload) });
    const callback = new URL(result.redirect_uri);
    const expected = new URL(payload.redirect_uri);
    if (callback.protocol !== "http:" || callback.hostname !== "127.0.0.1" || callback.pathname !== "/callback" ||
        callback.origin !== expected.origin || callback.searchParams.get("state") !== payload.state) throw new Error("Invalid callback");
    showDesktopLogin("网页验证完成，正在返回灵枢软件。若软件已关闭，请重新从软件发起登录。");
    // A top-level loopback navigation works without exposing tokens, opening a
    // public listener, or asking the browser to fetch a private-network API.
    location.replace(callback.href);
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) {
      document.querySelector("#desktop-login-view").hidden = true;
      clearSession();
      showAuth();
      setAuthMode("login", { focus: false });
      showNotice("登录软件", "请先登录网页账号，成功后软件将自动登录。", "success");
    } else {
      showDesktopLogin(error instanceof ApiError ? error.message : "无法完成网页验证，请返回软件重试。");
      retry.hidden = false;
    }
  }
}

async function init() {
  bindEvents();
  bindTeamForms();
  document.querySelector("#desktop-login-retry").addEventListener("click", () => void finishDesktopLogin());
  const query = new URLSearchParams(location.search);
  const verificationToken = query.get("verify");
  if (verificationToken) {
    await verifyFromUrl(verificationToken);
    return;
  }

  // Upgrade a previously open sessionStorage-only tab to the shared HttpOnly
  // browser session as soon as it loads the updated application.
  if (state.refreshToken) await refreshSession();
  if (isDesktopLogin()) { await finishDesktopLogin(); return; }

  if (state.accessToken || state.refreshToken || await refreshSession()) {
    try {
      await loadAccount();
      const billingResult = query.get("billing");
      if (billingResult === "success") {
        showToast("付款流程已完成，订阅状态将在账单确认后自动更新。");
      } else if (billingResult === "canceled") {
        showToast("已取消付款，没有产生费用。", "error");
      }
      if (billingResult) history.replaceState({}, "", location.pathname);
      return;
    } catch {
      clearSession();
    }
  }

  showAuth();
  setAuthMode("login", { focus: false });
}

void init();
