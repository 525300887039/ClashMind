import { useEffect, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  ArrowRightLeft,
  CheckCircle2,
  ShieldCheck,
  Sparkles,
  XCircle,
} from "lucide-react";
import { toast } from "sonner";

import { invalidateRuntimeQueries } from "@/lib/query-client";
import {
  api,
  type HealthGrade,
  type OptimizationSuggestion,
  type OptimizationToolResult,
} from "@/lib/tauri-api";
import { normalizeErrorMessage } from "@/lib/error";
import { useAiStore, type AiToolCallStatus } from "@/stores/ai-store";
import { cn } from "@/lib/utils";

const GRADE_META: Record<
  HealthGrade,
  {
    label: string;
    className: string;
  }
> = {
  excellent: {
    label: "Excellent",
    className: "border-emerald-500/25 bg-emerald-500/10 text-emerald-300",
  },
  good: {
    label: "Good",
    className: "border-cyan-500/25 bg-cyan-500/10 text-cyan-300",
  },
  fair: {
    label: "Fair",
    className: "border-amber-500/25 bg-amber-500/10 text-amber-300",
  },
  poor: {
    label: "Poor",
    className: "border-orange-500/25 bg-orange-500/10 text-orange-300",
  },
  critical: {
    label: "Critical",
    className: "border-destructive/25 bg-destructive/10 text-destructive",
  },
};

function formatScore(score: number | null): string {
  return score === null ? "暂无评分" : `${score.toFixed(1)} 分`;
}

function formatScoreDelta(scoreDelta: number | null): string | null {
  if (scoreDelta === null) {
    return null;
  }

  return `${scoreDelta > 0 ? "+" : ""}${scoreDelta.toFixed(1)} 分`;
}

function GradeBadge({ grade }: { grade: HealthGrade | null }) {
  if (grade === null) {
    return (
      <span className="inline-flex items-center rounded-full border border-border/70 bg-muted/40 px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
        未评分
      </span>
    );
  }

  const meta = GRADE_META[grade];

  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] font-medium",
        meta.className,
      )}
    >
      {meta.label}
    </span>
  );
}

function ScorePanel({
  label,
  score,
  grade,
}: {
  label: string;
  score: number | null;
  grade: HealthGrade | null;
}) {
  return (
    <div className="rounded-xl border border-border/70 bg-background/60 p-3">
      <p className="text-[11px] font-medium tracking-[0.14em] text-muted-foreground uppercase">
        {label}
      </p>
      <div className="mt-2 flex items-center justify-between gap-3">
        <span className="text-sm font-medium text-foreground">{formatScore(score)}</span>
        <GradeBadge grade={grade} />
      </div>
    </div>
  );
}

function resultHeadline(status: AiToolCallStatus): string | null {
  if (status === "applied") {
    return "切换已执行";
  }

  if (status === "rejected") {
    return "建议已忽略";
  }

  return null;
}

export function OptimizationCard({
  toolCallId,
  result,
  status,
}: {
  toolCallId: string;
  result: OptimizationToolResult;
  status: AiToolCallStatus;
}) {
  const queryClient = useQueryClient();
  const setToolCallStatus = useAiStore((state) => state.setToolCallStatus);
  const [dismissedTargets, setDismissedTargets] = useState<string[]>([]);
  const [selectedTarget, setSelectedTarget] = useState<string | null>(null);

  useEffect(() => {
    setDismissedTargets([]);
    setSelectedTarget(null);
  }, [toolCallId]);

  const visibleSuggestions = result.suggestions.filter(
    (suggestion) => !dismissedTargets.includes(suggestion.targetNode),
  );
  const headline = resultHeadline(status);
  const isResolved = status === "applied" || status === "rejected";

  const switchMutation = useMutation<void, Error, OptimizationSuggestion>({
    mutationFn: async (suggestion) => {
      await api.proxy.switch(suggestion.group, suggestion.targetNode);
    },
    onSuccess: async (_, suggestion) => {
      setToolCallStatus(toolCallId, "applied");
      await invalidateRuntimeQueries(queryClient);
      toast.success(`已将代理组「${suggestion.group}」切换到「${suggestion.targetNode}」`);
    },
    onError: (error, suggestion) => {
      toast.error(
        `切换到「${suggestion.targetNode}」失败: ${normalizeErrorMessage(error)}`,
      );
    },
  });

  const handleConfirm = (suggestion: OptimizationSuggestion) => {
    switchMutation.reset();
    setSelectedTarget(suggestion.targetNode);
    switchMutation.mutate(suggestion);
  };

  const handleReject = (targetNode: string) => {
    switchMutation.reset();
    setSelectedTarget(targetNode);

    if (visibleSuggestions.length <= 1) {
      setDismissedTargets((current) =>
        current.includes(targetNode) ? current : [...current, targetNode],
      );
      setToolCallStatus(toolCallId, "rejected");
      toast.success("已忽略本次优化建议");
      return;
    }

    setDismissedTargets((current) =>
      current.includes(targetNode) ? current : [...current, targetNode],
    );
    toast.success(`已忽略「${targetNode}」的切换建议`);
  };

  const currentNodeLabel = result.context.currentNode ?? "未选中节点";
  const resolutionTarget =
    selectedTarget ?? visibleSuggestions[0]?.targetNode ?? result.suggestions[0]?.targetNode ?? null;

  return (
    <section className="rounded-[1rem] border border-border/70 bg-muted/20 p-4">
      <div className="flex items-start gap-3">
        <div className="mt-0.5 inline-flex size-9 shrink-0 items-center justify-center rounded-full bg-amber-500/12 text-amber-300">
          <Sparkles className="size-4" />
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h4 className="text-sm font-medium text-foreground">优化建议</h4>
            <span className="rounded-full border border-border/70 bg-background/60 px-2 py-0.5 text-[11px] text-muted-foreground">
              代理组 · {result.context.group}
            </span>
          </div>

          <p className="mt-1 text-xs leading-6 text-muted-foreground">{result.message}</p>

          {headline !== null ? (
            <div
              className={cn(
                "mt-3 flex items-center gap-2 rounded-xl border px-3 py-2 text-sm",
                status === "applied"
                  ? "border-emerald-500/20 bg-emerald-500/8 text-emerald-300"
                  : "border-slate-500/20 bg-slate-500/8 text-slate-300",
              )}
            >
              {status === "applied" ? (
                <ShieldCheck className="size-4" />
              ) : (
                <XCircle className="size-4" />
              )}
              <span>
                {headline}
                {resolutionTarget === null ? "" : `：${currentNodeLabel} → ${resolutionTarget}`}
              </span>
            </div>
          ) : null}

          {visibleSuggestions.length > 0 ? (
            <div className="mt-4 space-y-3">
              {visibleSuggestions.map((suggestion) => {
                const isSubmitting =
                  switchMutation.isPending && selectedTarget === suggestion.targetNode;
                const deltaLabel = formatScoreDelta(suggestion.scoreDelta);
                const showInlineError =
                  switchMutation.isError &&
                  selectedTarget === suggestion.targetNode &&
                  !isResolved;

                return (
                  <article
                    key={suggestion.targetNode}
                    className="rounded-[1rem] border border-border/70 bg-background/70 p-4"
                  >
                    <div className="flex flex-wrap items-center gap-2 text-sm">
                      <span className="font-medium text-foreground">{currentNodeLabel}</span>
                      <ArrowRightLeft className="size-4 text-muted-foreground" />
                      <span className="font-medium text-foreground">{suggestion.targetNode}</span>
                      <GradeBadge grade={suggestion.targetGrade} />
                      {deltaLabel !== null ? (
                        <span className="text-xs font-medium text-emerald-300">{deltaLabel}</span>
                      ) : null}
                    </div>

                    <p className="mt-2 text-xs leading-6 text-muted-foreground">
                      {suggestion.reason}
                    </p>

                    <div className="mt-3 grid gap-3 md:grid-cols-2">
                      <ScorePanel
                        label="当前节点评分"
                        score={suggestion.currentScore}
                        grade={suggestion.currentGrade}
                      />
                      <ScorePanel
                        label="目标节点评分"
                        score={suggestion.targetScore}
                        grade={suggestion.targetGrade}
                      />
                    </div>

                    <div className="mt-3 flex flex-wrap items-center gap-2">
                      <button
                        type="button"
                        onClick={() => handleConfirm(suggestion)}
                        disabled={isResolved || switchMutation.isPending}
                        className="inline-flex items-center gap-1.5 rounded-full bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        <CheckCircle2 className="size-3.5" />
                        {isSubmitting ? "执行中..." : "确认切换"}
                      </button>

                      <button
                        type="button"
                        onClick={() => handleReject(suggestion.targetNode)}
                        disabled={isResolved || switchMutation.isPending}
                        className="inline-flex items-center gap-1.5 rounded-full border border-border/70 bg-background/80 px-3 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:border-border hover:bg-accent/60 hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        <XCircle className="size-3.5" />
                        忽略
                      </button>
                    </div>

                    {showInlineError ? (
                      <p className="mt-2 text-xs text-destructive">
                        {normalizeErrorMessage(switchMutation.error)}
                      </p>
                    ) : null}
                  </article>
                );
              })}
            </div>
          ) : (
            <div className="mt-4 rounded-[1rem] border border-border/70 bg-background/60 px-4 py-3 text-sm text-muted-foreground">
              当前没有可确认的切换建议。
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
