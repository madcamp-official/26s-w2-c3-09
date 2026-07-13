const permanentlyInvalidTokenCodes = new Set([
  "messaging/registration-token-not-registered",
  "messaging/invalid-registration-token",
]);

export function isPermanentlyInvalidTokenError(
  error: { code?: string } | undefined,
) {
  return Boolean(error?.code && permanentlyInvalidTokenCodes.has(error.code));
}
