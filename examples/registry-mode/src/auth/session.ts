export type AuthResult =
  | { ok: true; userId: string }
  | { ok: false; reason: "missing" | "invalid" };

export function readSession(token: string | undefined): AuthResult {
  if (!token) {
    return { ok: false, reason: "missing" };
  }

  if (token.startsWith("demo_")) {
    return { ok: true, userId: token.slice("demo_".length) };
  }

  return { ok: false, reason: "invalid" };
}
