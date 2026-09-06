import { HttpError } from "./errors";
import type { AppEnv, TeamInviteEmail, VerificationEmail } from "./types";

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/**
 * Uses Resend in deployed environments. Local development deliberately falls
 * back to a console-only delivery mode when no API key is configured, so the
 * Worker can be exercised without sending real email.
 */
export async function sendVerificationEmail(
  message: VerificationEmail,
  env: AppEnv,
): Promise<void> {
  const apiKey = env.RESEND_API_KEY?.trim();
  if (!apiKey) {
    if (env.ENVIRONMENT === "development" || env.ENVIRONMENT === "test") {
      console.info(
        `[lingshu-cloud] development verification URL for ${message.to}: ${message.verificationUrl}`,
      );
      return;
    }
    throw new HttpError(
      503,
      "Email delivery is not configured",
      "EMAIL_NOT_CONFIGURED",
    );
  }

  const from = env.RESEND_FROM_EMAIL?.trim();
  if (!from) {
    throw new HttpError(
      500,
      "Email sender is not configured",
      "EMAIL_SENDER_NOT_CONFIGURED",
    );
  }

  const response = await fetch(env.RESEND_API_URL?.trim() || "https://api.resend.com/emails", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      from,
      to: [message.to],
      subject: "Verify your LingShu account",
      html: [
        `<p>Hello ${escapeHtml(message.displayName)},</p>`,
        "<p>Click the link below to verify your LingShu account:</p>",
        `<p><a href="${escapeHtml(message.verificationUrl)}">Verify email address</a></p>`,
        "<p>This link expires soon and can only be used once.</p>",
      ].join(""),
    }),
  });

  if (!response.ok) {
    throw new HttpError(
      502,
      "Email delivery failed",
      "EMAIL_DELIVERY_FAILED",
    );
  }
}

/**
 * Notify an invitee that someone has invited them to a team workspace. The
 * email contains the in-app accept URL so the invitee can join from the
 * account-center SPA without having to type a token. Delivery failures bubble
 * up so callers can roll back the invite row rather than silently leaving the
 * invitee unaware.
 */
export async function sendTeamInviteEmail(
  message: TeamInviteEmail,
  env: AppEnv,
): Promise<void> {
  const apiKey = env.RESEND_API_KEY?.trim();
  const from = env.RESEND_FROM_EMAIL?.trim();
  if (!apiKey || !from) {
    if (env.ENVIRONMENT === "development" || env.ENVIRONMENT === "test") {
      console.info(
        `[lingshu-cloud] development invite URL for ${message.to}: ${message.acceptUrl}`,
      );
      return;
    }
    throw new HttpError(
      503,
      "Email delivery is not configured",
      "EMAIL_NOT_CONFIGURED",
    );
  }
  const roleLabel = message.role === "admin"
    ? "管理员"
    : message.role === "editor"
      ? "编辑者"
      : "只读成员";
  const response = await fetch(env.RESEND_API_URL?.trim() || "https://api.resend.com/emails", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      from,
      to: [message.to],
      subject: `[灵枢] ${escapeHtml(message.inviterName)} 邀请您加入团队「${escapeHtml(message.teamName)}」`,
      html: [
        `<p>${escapeHtml(message.inviterName)} 邀请您加入灵枢团队 <strong>${escapeHtml(message.teamName)}</strong>（角色：${escapeHtml(roleLabel)}）。</p>`,
        "<p>点击下面的链接登录灵枢账户中心接受邀请：</p>",
        `<p><a href="${escapeHtml(message.acceptUrl)}">${escapeHtml(message.acceptUrl)}</a></p>`,
        `<p>邀请 7 天内有效。如果链接打不开，请登录 account.lingeringdream.com，进入侧栏「团队」页面查看待处理邀请。</p>`,
        "<p>如果您没有灵枢账户，请先到账户中心使用此邮箱注册后再接受邀请。</p>",
      ].join(""),
    }),
  });
  if (!response.ok) {
    throw new HttpError(
      502,
      "Email delivery failed",
      "EMAIL_DELIVERY_FAILED",
    );
  }
}
