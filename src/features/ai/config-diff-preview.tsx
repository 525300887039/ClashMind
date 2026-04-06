import {
  CheckCheck,
  FileDiff,
  Minus,
  PencilLine,
  Plus,
  RotateCw,
  X,
} from "lucide-react";
import type { ConfigDiff } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";
import type { AiToolCallStatus } from "@/stores/ai-store";

interface ParsedDiffLine {
  kind: "meta" | "line";
  content: string;
  marker?: " " | "+" | "-";
  oldLine?: number | null;
  newLine?: number | null;
}

export interface ConfigDiffPreviewProps {
  diff: ConfigDiff;
  onConfirm: () => void;
  onReject: () => void;
  isApplying: boolean;
  isRejecting: boolean;
  error?: string | null;
  status?: AiToolCallStatus;
}

function parseUnifiedDiff(unifiedDiff: string): ParsedDiffLine[] {
  const lines = unifiedDiff.replace(/\r\n/g, "\n").split("\n");
  if (lines[lines.length - 1] === "") {
    lines.pop();
  }

  let oldCursor = 0;
  let newCursor = 0;

  return lines.map((line) => {
    if (line.startsWith("--- ") || line.startsWith("+++ ") || line.startsWith("@@ ")) {
      const hunkMatch = /^@@ -(\d+),(\d+) \+(\d+),(\d+) @@/.exec(line);
      if (hunkMatch) {
        oldCursor = Number.parseInt(hunkMatch[1] ?? "0", 10);
        newCursor = Number.parseInt(hunkMatch[3] ?? "0", 10);
      }

      return {
        kind: "meta",
        content: line,
      };
    }

    if (line.startsWith("+")) {
      const parsedLine: ParsedDiffLine = {
        kind: "line",
        content: line.slice(1),
        marker: "+",
        oldLine: null,
        newLine: newCursor,
      };
      newCursor += 1;
      return parsedLine;
    }

    if (line.startsWith("-")) {
      const parsedLine: ParsedDiffLine = {
        kind: "line",
        content: line.slice(1),
        marker: "-",
        oldLine: oldCursor,
        newLine: null,
      };
      oldCursor += 1;
      return parsedLine;
    }

    const parsedLine: ParsedDiffLine = {
      kind: "line",
      content: line.startsWith(" ") ? line.slice(1) : line,
      marker: " ",
      oldLine: oldCursor,
      newLine: newCursor,
    };
    oldCursor += 1;
    newCursor += 1;
    return parsedLine;
  });
}

function formatLineNumber(lineNumber: number | null | undefined) {
  return lineNumber === null || lineNumber === undefined || lineNumber <= 0
    ? ""
    : lineNumber.toString();
}

function statusMessage(status: AiToolCallStatus | undefined) {
  if (status === "applied") {
    return {
      text: "配置已写入并完成热重载。",
      className:
        "border-emerald-500/25 bg-emerald-500/10 text-emerald-700 dark:text-emerald-200",
    };
  }

  if (status === "rejected") {
    return {
      text: "本次配置变更已丢弃，未写入配置文件。",
      className:
        "border-border/80 bg-muted/45 text-muted-foreground dark:border-slate-700/80 dark:bg-slate-900/70 dark:text-slate-300",
    };
  }

  return null;
}

function buildChangeStats(diff: ConfigDiff) {
  const addCount = diff.changes.filter((change) => change.type === "add").length;
  const removeCount = diff.changes.filter((change) => change.type === "remove").length;
  const modifyCount = diff.changes.filter((change) => change.type === "modify").length;

  return [
    {
      key: "add",
      label: "新增",
      value: addCount,
      icon: Plus,
      className:
        "border-emerald-500/25 bg-emerald-500/10 text-emerald-700 dark:text-emerald-200",
    },
    {
      key: "remove",
      label: "删除",
      value: removeCount,
      icon: Minus,
      className:
        "border-rose-500/25 bg-rose-500/10 text-rose-700 dark:text-rose-200",
    },
    {
      key: "modify",
      label: "调整",
      value: modifyCount,
      icon: PencilLine,
      className:
        "border-sky-500/25 bg-sky-500/10 text-sky-700 dark:text-sky-200",
    },
  ].filter((stat) => stat.value > 0);
}

export function ConfigDiffPreview({
  diff,
  onConfirm,
  onReject,
  isApplying,
  isRejecting,
  error,
  status = "awaiting_confirmation",
}: ConfigDiffPreviewProps) {
  const parsedLines = parseUnifiedDiff(diff.unifiedDiff);
  const stats = buildChangeStats(diff);
  const notice = statusMessage(status);
  const isResolved = status === "applied" || status === "rejected";
  const isBusy = isApplying || isRejecting;

  return (
    <section className="relative overflow-hidden rounded-[1.6rem] border border-border/80 bg-[linear-gradient(180deg,rgba(255,255,255,0.92),rgba(248,250,252,0.96))] shadow-[0_28px_80px_-44px_rgba(15,23,42,0.22)] dark:border-slate-800/90 dark:bg-[radial-gradient(circle_at_top_right,rgba(56,189,248,0.14),transparent_32%),linear-gradient(180deg,rgba(15,23,42,0.96),rgba(2,6,23,0.98))] dark:shadow-[0_28px_80px_-42px_rgba(2,6,23,0.95)]">
      <div className="pointer-events-none absolute inset-x-8 top-0 h-px bg-linear-to-r from-transparent via-sky-400/40 to-transparent" />

      <div className="relative border-b border-border/70 px-5 py-4 dark:border-slate-800/90">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="max-w-3xl">
            <div className="inline-flex items-center gap-2 rounded-full border border-sky-500/20 bg-sky-500/10 px-3 py-1 text-[11px] font-medium tracking-[0.2em] text-sky-700 uppercase dark:text-sky-100">
              <FileDiff className="size-3.5" />
              待确认配置
            </div>
            <h3 className="mt-3 text-base font-semibold text-foreground dark:text-slate-50">
              配置变更预览
            </h3>
            <p className="mt-1 text-sm leading-6 text-muted-foreground dark:text-slate-300/85">
              {diff.summary}
            </p>
          </div>

          <div className="flex flex-wrap items-center justify-end gap-2">
            {(stats.length > 0 ? stats : [{ key: "all", label: "变更", value: diff.changes.length, icon: FileDiff, className: "border-border/80 bg-muted/45 text-muted-foreground dark:border-slate-700/80 dark:bg-slate-900/70 dark:text-slate-300" }]).map(
              ({ key, label, value, icon: Icon, className }) => (
                <span
                  key={key}
                  className={cn(
                    "inline-flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-[11px] font-medium tracking-[0.18em] uppercase",
                    className,
                  )}
                >
                  <Icon className="size-3.5" />
                  {label} {value}
                </span>
              ),
            )}
          </div>
        </div>
      </div>

      <div className="grid gap-4 px-5 py-4 xl:grid-cols-[18rem_minmax(0,1fr)]">
        <section className="rounded-[1.35rem] border border-border/70 bg-background/72 p-4 dark:border-slate-800/80 dark:bg-slate-950/55">
          <p className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase dark:text-slate-400">
            变更摘要
          </p>

          {diff.changes.length > 0 ? (
            <ul className="mt-3 space-y-2.5 text-sm leading-6 text-foreground dark:text-slate-200">
              {diff.changes.map((change) => (
                <li key={`${change.type}-${change.path}-${change.description}`} className="flex gap-2.5">
                  <span
                    className={cn(
                      "mt-2 size-1.5 shrink-0 rounded-full",
                      change.type === "add"
                        ? "bg-emerald-500"
                        : change.type === "remove"
                          ? "bg-rose-500"
                          : "bg-sky-500",
                    )}
                  />
                  <span>{change.description}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="mt-3 text-sm leading-6 text-muted-foreground dark:text-slate-300/80">
              未检测到结构化摘要。
            </p>
          )}

          <div className="mt-5 rounded-[1rem] border border-border/70 bg-muted/35 px-3 py-3 text-xs leading-6 text-muted-foreground dark:border-slate-800/80 dark:bg-slate-900/70 dark:text-slate-400">
            确认后将写入当前配置并触发 mihomo 热重载；若重载失败，后端会自动写回旧配置并返回回滚提示。
          </div>
        </section>

        <section className="overflow-hidden rounded-[1.35rem] border border-border/70 bg-background/86 dark:border-slate-800/80 dark:bg-slate-950/80">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border/70 px-4 py-3 dark:border-slate-800/80">
            <div className="grid grid-cols-[4.5rem_4.5rem_2rem_minmax(0,1fr)] text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase dark:text-slate-400">
              <span>旧行号</span>
              <span>新行号</span>
              <span>标记</span>
              <span>变更内容</span>
            </div>

            <div className="flex flex-wrap items-center gap-2 text-[11px] font-medium tracking-[0.16em] text-muted-foreground uppercase dark:text-slate-400">
              <span className="inline-flex items-center gap-1.5 rounded-full border border-border/70 bg-muted/35 px-2.5 py-1 dark:border-slate-700/80 dark:bg-slate-900/70">
                <span className="size-2 rounded-full bg-emerald-500" />
                新增
              </span>
              <span className="inline-flex items-center gap-1.5 rounded-full border border-border/70 bg-muted/35 px-2.5 py-1 dark:border-slate-700/80 dark:bg-slate-900/70">
                <span className="size-2 rounded-full bg-rose-500" />
                删除
              </span>
            </div>
          </div>

          <div className="max-h-[28rem] overflow-auto">
            {parsedLines.map((line, index) =>
              line.kind === "meta" ? (
                <div
                  key={`${line.content}-${index}`}
                  className="border-b border-border/60 bg-muted/40 px-4 py-2 font-mono text-[12px] text-muted-foreground dark:border-slate-900/70 dark:bg-slate-900/90 dark:text-slate-400"
                >
                  {line.content}
                </div>
              ) : (
                <div
                  key={`${line.marker}-${line.oldLine ?? "n"}-${line.newLine ?? "n"}-${index}`}
                  className={cn(
                    "grid grid-cols-[4.5rem_4.5rem_2rem_minmax(0,1fr)] border-b border-border/50 font-mono text-[12.5px] leading-6 dark:border-slate-900/60",
                    line.marker === "+"
                      ? "bg-emerald-500/10 text-emerald-950 dark:bg-emerald-500/12 dark:text-emerald-100"
                      : line.marker === "-"
                        ? "bg-rose-500/10 text-rose-950 dark:bg-rose-500/12 dark:text-rose-100"
                        : "bg-transparent text-foreground/90 dark:text-slate-200/90",
                  )}
                >
                  <span className="border-r border-border/50 px-4 py-1.5 text-muted-foreground/90 dark:border-slate-900/60 dark:text-slate-500">
                    {formatLineNumber(line.oldLine)}
                  </span>
                  <span className="border-r border-border/50 px-4 py-1.5 text-muted-foreground/90 dark:border-slate-900/60 dark:text-slate-500">
                    {formatLineNumber(line.newLine)}
                  </span>
                  <span
                    className={cn(
                      "border-r border-border/50 px-3 py-1.5 font-semibold dark:border-slate-900/60",
                      line.marker === "+"
                        ? "text-emerald-700 dark:text-emerald-300"
                        : line.marker === "-"
                          ? "text-rose-700 dark:text-rose-300"
                          : "text-muted-foreground dark:text-slate-500",
                    )}
                  >
                    {line.marker === " " ? "" : line.marker}
                  </span>
                  <span className="overflow-x-auto px-4 py-1.5 whitespace-pre">{line.content}</span>
                </div>
              ),
            )}
          </div>
        </section>
      </div>

      <div className="border-t border-border/70 px-5 py-4 dark:border-slate-800/90">
        {notice ? (
          <div className={cn("rounded-[1rem] border px-4 py-3 text-sm", notice.className)}>
            {notice.text}
          </div>
        ) : null}

        {error ? (
          <div className="mt-3 rounded-[1rem] border border-rose-500/25 bg-rose-500/10 px-4 py-3 text-sm text-rose-700 dark:text-rose-200">
            {error}
          </div>
        ) : null}

        {!isResolved ? (
          <div className="mt-4 flex flex-wrap items-center justify-end gap-3">
            <button
              type="button"
              onClick={onReject}
              disabled={isBusy}
              className="inline-flex items-center gap-2 rounded-full border border-border/80 bg-background/82 px-4 py-2.5 text-sm font-medium text-foreground transition-colors hover:border-rose-500/30 hover:bg-rose-500/6 hover:text-rose-700 disabled:cursor-not-allowed disabled:opacity-60 dark:border-slate-700/80 dark:bg-slate-900/85 dark:text-slate-200 dark:hover:border-slate-500 dark:hover:bg-slate-800 dark:hover:text-slate-50"
            >
              {isRejecting ? <RotateCw className="size-4 animate-spin" /> : <X className="size-4" />}
              {isRejecting ? "正在丢弃..." : "取消"}
            </button>

            <button
              type="button"
              onClick={onConfirm}
              disabled={isBusy}
              className="inline-flex items-center gap-2 rounded-full border border-emerald-400/35 bg-linear-to-r from-emerald-500 via-emerald-400 to-teal-400 px-4 py-2.5 text-sm font-semibold text-white shadow-[0_16px_40px_-20px_rgba(16,185,129,0.55)] transition-transform hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-60 dark:text-slate-950"
            >
              {isApplying ? <RotateCw className="size-4 animate-spin" /> : <CheckCheck className="size-4" />}
              {isApplying ? "正在应用..." : "确认应用"}
            </button>
          </div>
        ) : null}
      </div>
    </section>
  );
}
