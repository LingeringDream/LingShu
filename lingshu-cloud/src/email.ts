import { HttpError } from "./errors";
import type { AppEnv, VerificationEmail } from "./types";

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
