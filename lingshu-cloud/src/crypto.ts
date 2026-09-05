const PASSWORD_SCHEME = "pbkdf2_sha256";
// Cloudflare Workers currently caps a single PBKDF2 deriveBits operation at
// 100,000 iterations. Keep the encoded iteration count explicit so future
// password schemes can be upgraded without breaking existing hashes.
const PASSWORD_ITERATIONS = 100_000;
const PASSWORD_SALT_BYTES = 16;
const PASSWORD_KEY_BYTES = 32;

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

function bytesToBase64Url(bytes: Uint8Array): string {
  let binary = "";
  const chunkSize = 0x8000;
  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(
      ...bytes.subarray(index, Math.min(index + chunkSize, bytes.length)),
    );
  }
  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");
}

function base64UrlToBytes(value: string): Uint8Array {
  const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
  const padded = normalized + "=".repeat((4 - (normalized.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function encodeJson(value: unknown): string {
  return bytesToBase64Url(textEncoder.encode(JSON.stringify(value)));
}

function decodeJson<T>(value: string): T {
  return JSON.parse(textDecoder.decode(base64UrlToBytes(value))) as T;
}

function arrayBufferOf(bytes: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return copy.buffer;
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  const maxLength = Math.max(left.length, right.length);
  let difference = left.length ^ right.length;
  for (let index = 0; index < maxLength; index += 1) {
    difference |= (left[index] ?? 0) ^ (right[index] ?? 0);
  }
  return difference === 0;
}

async function derivePasswordKey(
  password: string,
  salt: Uint8Array,
  iterations: number,
): Promise<Uint8Array> {
  const key = await crypto.subtle.importKey(
    "raw",
    textEncoder.encode(password),
    { name: "PBKDF2" },
    false,
    ["deriveBits"],
  );
  const bits = await crypto.subtle.deriveBits(
    {
      name: "PBKDF2",
      hash: "SHA-256",
      salt: arrayBufferOf(salt),
      iterations,
    },
    key,
    PASSWORD_KEY_BYTES * 8,
  );
  return new Uint8Array(bits);
}

export async function hashPassword(password: string): Promise<string> {
  const salt = crypto.getRandomValues(new Uint8Array(PASSWORD_SALT_BYTES));
  const derived = await derivePasswordKey(password, salt, PASSWORD_ITERATIONS);
  return [
    PASSWORD_SCHEME,
    String(PASSWORD_ITERATIONS),
    bytesToBase64Url(salt),
    bytesToBase64Url(derived),
  ].join("$");
}

export async function verifyPassword(
  password: string,
  encodedHash: string,
): Promise<boolean> {
  const [scheme, iterationText, saltText, expectedText] = encodedHash.split("$");
  const iterations = Number(iterationText);
  if (
    scheme !== PASSWORD_SCHEME ||
    !Number.isSafeInteger(iterations) ||
    iterations < 10_000 ||
    !saltText ||
    !expectedText
  ) {
    return false;
  }

  try {
    const actual = await derivePasswordKey(
      password,
      base64UrlToBytes(saltText),
      iterations,
    );
    return equalBytes(actual, base64UrlToBytes(expectedText));
  } catch {
    return false;
  }
}

export async function burnPasswordKdf(password: string): Promise<void> {
  await derivePasswordKey(password, new Uint8Array(PASSWORD_SALT_BYTES), PASSWORD_ITERATIONS);
}

export function randomToken(byteLength = 32): string {
  return bytesToBase64Url(crypto.getRandomValues(new Uint8Array(byteLength)));
}

export async function sha256Base64Url(value: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", textEncoder.encode(value));
  return bytesToBase64Url(new Uint8Array(digest));
}

export interface AccessTokenClaims {
  iss: "lingshu-cloud";
  aud: "lingshu-client";
  sub: string;
  sid: string;
  did: string;
  typ: "access";
  jti: string;
  iat: number;
  exp: number;
}

async function hmacKey(secret: string, usages: KeyUsage[]): Promise<CryptoKey> {
  return crypto.subtle.importKey(
    "raw",
    textEncoder.encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    usages,
  );
}

export async function signAccessToken(
  claims: AccessTokenClaims,
  secret: string,
): Promise<string> {
  const header = encodeJson({ alg: "HS256", typ: "JWT" });
  const payload = encodeJson(claims);
  const signingInput = `${header}.${payload}`;
  const key = await hmacKey(secret, ["sign"]);
  const signature = await crypto.subtle.sign(
    "HMAC",
    key,
    textEncoder.encode(signingInput),
  );
  return `${signingInput}.${bytesToBase64Url(new Uint8Array(signature))}`;
}

export async function verifyAccessToken(
  token: string,
  secret: string,
): Promise<AccessTokenClaims | null> {
  const parts = token.split(".");
  if (parts.length !== 3) return null;

  try {
    const header = decodeJson<{ alg?: string; typ?: string }>(parts[0]);
    if (header.alg !== "HS256" || header.typ !== "JWT") return null;

    const key = await hmacKey(secret, ["verify"]);
    const valid = await crypto.subtle.verify(
      "HMAC",
      key,
      arrayBufferOf(base64UrlToBytes(parts[2])),
      textEncoder.encode(`${parts[0]}.${parts[1]}`),
    );
    if (!valid) return null;

    const claims = decodeJson<AccessTokenClaims>(parts[1]);
    const now = Math.floor(Date.now() / 1000);
    if (
      claims.iss !== "lingshu-cloud" ||
      claims.aud !== "lingshu-client" ||
      claims.typ !== "access" ||
      typeof claims.sub !== "string" ||
      typeof claims.sid !== "string" ||
      typeof claims.did !== "string" ||
      typeof claims.exp !== "number" ||
      claims.exp <= now
    ) {
      return null;
    }
    return claims;
  } catch {
    return null;
  }
}
