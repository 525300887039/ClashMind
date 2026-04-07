export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function getErrorMessage(error: unknown, fallback = "unknown error"): string {
  return error instanceof Error ? error.message : fallback;
}
