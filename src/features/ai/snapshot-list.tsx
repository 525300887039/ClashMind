import { useState } from "react";
import {
  Camera,
  ChevronDown,
  ChevronUp,
  History,
  RefreshCw,
  RotateCcw,
} from "lucide-react";
import type { ConfigSnapshot } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";
import { useCreateSnapshot, useRestoreSnapshot, useSnapshots } from "./hooks/use-snapshots";

const timeFormatter = new Intl.DateTimeFormat("zh-CN", {
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
});

function formatSnapshotTime(value: string) {
  const normalized = value.includes("T") ? value : `${value.replace(" ", "T")}Z`;
  const parsed = new Date(normalized);

  if (Number.isNaN(parsed.valueOf())) {
    return value;
  }

  return timeFormatter.format(parsed);
}

function sourceMeta(snapshot: ConfigSnapshot) {
  if (snapshot.source === "ai") {
    return {
      label: "AI",
      className: "border-cyan-500/25 bg-cyan-500/10 text-cyan-300",
      title: "AI 自动快照",
    };
  }

  return {
    label: "手动",
    className: "border-emerald-500/25 bg-emerald-500/10 text-emerald-300",
    title: "手动快照",
  };
}

function snapshotTitle(snapshot: ConfigSnapshot) {
  const trimmed = snapshot.description?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : sourceMeta(snapshot).title;
}

export function SnapshotList() {
  const [expandedSnapshotId, setExpandedSnapshotId] = useState<number | null>(null);
  const { data: snapshots = [], isLoading, isFetching, error } = useSnapshots();
  const createMutation = useCreateSnapshot();
  const restoreMutation = useRestoreSnapshot();
  const restoringSnapshotId =
    restoreMutation.isPending && restoreMutation.variables !== undefined
      ? restoreMutation.variables
      : null;
  const isBusy = createMutation.isPending || restoreMutation.isPending;

  return (
    <section className="relative overflow-hidden rounded-[1.75rem] border border-border/70 bg-linear-to-br from-background via-background to-muted/20 p-5 shadow-[0_24px_90px_-50px_rgba(15,23,42,0.45)]">
      <div className="pointer-events-none absolute -right-8 top-0 size-24 rounded-full bg-primary/10 blur-3xl" />

      <div className="relative flex items-start justify-between gap-3">
        <div>
          <div className="inline-flex size-10 items-center justify-center rounded-[1rem] bg-primary/10 text-primary">
            <History className="size-5" />
          </div>
          <h3 className="mt-4 text-base font-semibold text-foreground">配置快照</h3>
          <p className="mt-2 text-sm leading-6 text-muted-foreground">
            每次 AI 应用配置和手动保存前都会先备份，系统只保留最近 100 个快照。
          </p>
        </div>

        <button
          type="button"
          onClick={() => createMutation.mutate("手动快照")}
          disabled={isBusy}
          className="inline-flex items-center gap-2 rounded-full border border-primary/20 bg-primary/10 px-3 py-2 text-sm font-medium text-primary transition-colors hover:bg-primary/15 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {createMutation.isPending ? (
            <RefreshCw className="size-4 animate-spin" />
          ) : (
            <Camera className="size-4" />
          )}
          {createMutation.isPending ? "创建中" : "创建快照"}
        </button>
      </div>

      <div className="relative mt-5 space-y-3">
        {isLoading ? (
          <div className="rounded-[1.25rem] border border-border/70 bg-muted/20 px-4 py-6 text-sm text-muted-foreground">
            快照列表加载中...
          </div>
        ) : null}

        {error ? (
          <div className="rounded-[1.25rem] border border-destructive/20 bg-destructive/5 px-4 py-3 text-sm text-destructive">
            加载快照失败: {error.message}
          </div>
        ) : null}

        {!isLoading && !error && snapshots.length === 0 ? (
          <div className="rounded-[1.25rem] border border-dashed border-border/70 bg-muted/15 px-4 py-6 text-sm leading-6 text-muted-foreground">
            还没有可用快照。首次应用 AI 配置变更或手动创建后，这里会显示历史版本。
          </div>
        ) : null}

        {!isLoading && !error
          ? snapshots.map((snapshot) => {
              const meta = sourceMeta(snapshot);
              const isExpanded = expandedSnapshotId === snapshot.id;
              const isRestoring = restoringSnapshotId === snapshot.id;

              return (
                <article
                  key={snapshot.id}
                  className="overflow-hidden rounded-[1.25rem] border border-border/70 bg-background/70"
                >
                  <div className="flex flex-wrap items-start justify-between gap-3 px-4 py-4">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="text-sm font-semibold text-foreground">
                          {snapshotTitle(snapshot)}
                        </span>
                        <span
                          className={cn(
                            "inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] font-medium tracking-[0.16em] uppercase",
                            meta.className,
                          )}
                        >
                          {meta.label}
                        </span>
                      </div>

                      <p className="mt-2 text-xs tracking-[0.14em] text-muted-foreground uppercase">
                        #{snapshot.id} · {formatSnapshotTime(snapshot.createdAt)}
                        {snapshot.filePath ? ` · ${snapshot.filePath}` : ""}
                      </p>
                    </div>

                    <div className="flex items-center gap-2">
                      <button
                        type="button"
                        onClick={() =>
                          setExpandedSnapshotId((currentValue) =>
                            currentValue === snapshot.id ? null : snapshot.id,
                          )
                        }
                        className="inline-flex items-center gap-2 rounded-full border border-border/70 bg-background/70 px-3 py-2 text-sm text-muted-foreground transition-colors hover:border-primary/25 hover:bg-primary/5 hover:text-foreground"
                      >
                        {isExpanded ? (
                          <ChevronUp className="size-4" />
                        ) : (
                          <ChevronDown className="size-4" />
                        )}
                        {isExpanded ? "收起预览" : "预览"}
                      </button>

                      <button
                        type="button"
                        onClick={() => restoreMutation.mutate(snapshot.id)}
                        disabled={isBusy}
                        className="inline-flex items-center gap-2 rounded-full border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-sm font-medium text-emerald-300 transition-colors hover:bg-emerald-500/15 disabled:cursor-not-allowed disabled:opacity-60"
                      >
                        {isRestoring ? (
                          <RefreshCw className="size-4 animate-spin" />
                        ) : (
                          <RotateCcw className="size-4" />
                        )}
                        {isRestoring ? "恢复中" : "恢复"}
                      </button>
                    </div>
                  </div>

                  {isExpanded ? (
                    <div className="border-t border-border/70 bg-muted/15 px-4 py-4">
                      <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-words rounded-[1rem] border border-border/70 bg-slate-950/92 p-4 font-mono text-[12px] leading-6 text-slate-100 shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]">
                        {snapshot.content}
                      </pre>
                    </div>
                  ) : null}
                </article>
              );
            })
          : null}

        {isFetching && !isLoading ? (
          <p className="text-xs tracking-[0.14em] text-muted-foreground uppercase">
            正在刷新快照列表...
          </p>
        ) : null}
      </div>
    </section>
  );
}
