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
  download: { path: "/download", title: "下载中心" },
};

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
  writeSession(SESSION_KEYS.access, null);
  writeSession(SESSION_KEYS.refresh, null);
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
  return entry?.[0] || "account";
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
  elements.currentPlanName.textContent = subscription.plan_name || "免费版";
  elements.subscriptionStatus.textContent = statusLabels[subscription.status] || "状态未知";
  elements.subscriptionStatus.dataset.status = subscription.status || "unknown";

  const periodEnd = formatBillingDate(subscription.current_period_end);
  if (subscription.cancel_at_period_end && periodEnd) {
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
  const hasManagedSubscription = subscription.plan_code === "developer" &&
    ["active", "trialing", "past_due", "incomplete"].includes(subscription.status);

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

  if (hasManagedSubscription) {
    elements.billingAvailability.textContent = "已有付费权益，请使用“管理账单”查看订单与到期时间。";
  } else if (billing.configured) {
    elements.billingAvailability.textContent = "权益按购买时长生效，当前不自动续费，到期前会提醒。";
  } else {
    elements.billingAvailability.textContent = "支付通道尚未启用，当前不会产生任何费用。";
  }
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
  document.querySelectorAll("[data-github-platform]").forEach((link) => {
    const entry = downloads[link.dataset.githubPlatform] || {};
    const manifestUrl = validDriveUrl(entry.github_url);
    const fallbackUrl = validDriveUrl(link.getAttribute("href"));
    const url = manifestUrl || fallbackUrl;
    if (url) {
      link.href = url;
      link.removeAttribute("aria-disabled");
    } else {
      link.removeAttribute("href");
      link.setAttribute("aria-disabled", "true");
    }
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
    const code = typeof manifest.access_code === "string" && manifest.access_code.trim()
      ? `提取码：${manifest.access_code.trim()}`
      : "将在新窗口打开网盘页面。";
    text.textContent = code;
    elements.downloadStatus.classList.add("is-ready");
  } else {
    title.textContent = "网盘链接整理中";
    text.textContent = "GitHub 下载需登录有仓库权限的账户；网盘地址配置后供普通用户使用。";
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
  const [account, devices, billing] = await Promise.all([
    authorizedRequest(`${API}/account`),
    pageName === "devices" ? authorizedRequest(`${API}/account/devices`) : Promise.resolve(null),
    pageName === "billing" ? authorizedRequest(`${API}/account/subscription`) : Promise.resolve(null),
  ]);
  state.account = account;
  if (devices) state.devices = devices.devices || [];
  if (billing) state.billing = billing;
  renderAccount();
  if (billing) renderBilling();
  if (pageName === "download") await loadDownloads();
  if (pageName === requestedAccountPage()) showAccount();
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
