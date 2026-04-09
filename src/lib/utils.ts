import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";

  const base = 1024;
  const units = ["B", "KB", "MB", "GB", "TB"] as const;
  const safeBytes = Math.max(bytes, 0);
  const unitIndex = Math.min(
    Math.floor(Math.log(safeBytes) / Math.log(base)),
    units.length - 1,
  );

  return `${Number((safeBytes / base ** unitIndex).toFixed(2))} ${units[unitIndex]}`;
}

/**
 * Formats a UTC timestamp string to a short zh-CN locale string (MM/DD HH:mm).
 * Returns the fallback if the value is missing or unparseable.
 */
export function formatZhDateTime(
  value: string | undefined,
  fallback = "尚未生成",
): string {
  if (!value) return fallback;

  const parsed = new Date(value);
  if (Number.isNaN(parsed.valueOf())) return value;

  return parsed.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}
