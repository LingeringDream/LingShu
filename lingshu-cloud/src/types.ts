export interface AppEnv {
  ASSETS?: Fetcher;
  DB: D1Database;
  MEMORY_BUCKET: R2Bucket;
  JWT_SECRET?: string;
  RESEND_API_KEY?: string;
  ENVIRONMENT?: string;
  APP_BASE_URL?: string;
  RESEND_API_URL?: string;
  RESEND_FROM_EMAIL?: string;
  ACCESS_TOKEN_TTL_SECONDS?: string;
  REFRESH_TOKEN_TTL_SECONDS?: string;
  VERIFICATION_TOKEN_TTL_SECONDS?: string;
  LOGIN_WINDOW_SECONDS?: string;
  LOGIN_MAX_ATTEMPTS?: string;
  LOGIN_BLOCK_SECONDS?: string;
  MAX_BODY_BYTES?: string;
  CORS_ORIGINS?: string;
  STRIPE_SECRET_KEY?: string;
  STRIPE_WEBHOOK_SECRET?: string;
  STRIPE_API_BASE_URL?: string;
  STRIPE_PRICE_DEVELOPER_MONTHLY?: string;
  STRIPE_PRICE_DEVELOPER_YEARLY?: string;
  STRIPE_DEVELOPER_MONTHLY_LABEL?: string;
  STRIPE_DEVELOPER_YEARLY_LABEL?: string;
  STRIPE_PORTAL_CONFIGURATION_ID?: string;
}

export interface VerificationEmail {
  to: string;
  displayName: string;
  token: string;
  verificationUrl: string;
}

export interface TeamInviteEmail {
  to: string;
  inviterName: string;
  teamName: string;
  role: "admin" | "editor" | "viewer";
  inviteId: string;
  acceptUrl: string;
}

export type Mailer = (
  message: VerificationEmail,
  env: AppEnv,
) => Promise<void>;

export interface AuthContext {
  userId: string;
  sessionId: string;
  deviceId: string;
  email: string;
  displayName: string;
  sessionExpiresAt: string;
}

export interface PublicUser {
  id: string;
  email: string;
  display_name: string;
  email_verified_at: string;
  created_at: string;
}
