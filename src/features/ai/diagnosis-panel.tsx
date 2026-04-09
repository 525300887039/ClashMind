import { useEffect, useMemo, useState } from "react";
import { motion } from "framer-motion";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Activity,
  AlertCircle,
  AlertTriangle,
  FileText,
  Info,
  RefreshCw,
} from "lucide-react";
import { EmptyState } from "@/components/ui/empty-state";
import { SectionCard } from "@/components/ui/section-card";
import { markdownComponents } from "./markdown-components";
import { DiagnosisAlertCard } from "./diagnosis-alert-card";
import {
  useDiagnosisOverview,
  useQuickDiagnosis,
} from "./hooks/use-diagnosis";
import { isAiConfigured, useAiSettingsQuery } from "./hooks/use-ai-settings";
import type { DiagnosisSummary, DiagnosisTimeRangeMinutes } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/app-store";

const DIAGNOSIS_TIME_RANGE_OPTIONS: Array<{
  value: DiagnosisTimeRangeMinutes;
  label: string;
  caption: string;
}> = [
  { value: 15, label: "15 分钟", caption: "瞬时波动" },
  { value: 30, label: "30 分钟", caption: "默认窗口" },
  { value: 60, label: "1 小时", caption: "短期趋势" },
  { value: 360, label: "6 小时", caption: "半日回看" },
  { value: 1440, label: "24 小时", caption: "全天诊断" },
];

interface SummaryMetricCardProps {
  label: string;
  value: string;
  caption: string;
  icon: React.ComponentType<{ className?: string }>;
  accentClassName: string;
}

function formatCount(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

function formatPercent(value: number | undefined) {
  if (value === undefined || !Number.isFinite(value)) {
    return "0%";
  }

  return `${(value * 100).toFixed(1)}%`;
}

function formatTimestamp(value: string | undefined) {
  if (!value) {
    return "尚未生成";
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.valueOf())) {
    return value;
  }

  return parsed.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

function getTotalErrorCount(summary: DiagnosisSummary | undefined) {
  return summary?.errorStats.reduce((total, item) => total + item.count, 0) ?? 0;
}

function SummaryMetricCard({
  label,
  value,
  caption,
  icon: Icon,
  accentClassName,
}: SummaryMetricCardProps) {
  return (
    <article className="rounded-[1.3rem] border border-border/70 bg-background/82 p-4 shadow-[0_18px_48px_-34px_rgba(15,23,42,0.55)]">
      <div className="flex items-center justify-between gap-3">
        <div className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
          {label}
        </div>
        <div
          className={cn(
            "inline-flex size-9 items-center justify-center rounded-full ring-1 ring-inset",
            accentClassName,
          )}
        >
          <Icon className="size-4" />
        </div>
      </div>
      <div className="mt-3 text-2xl font-semibold tracking-tight text-foreground">{value}</div>
      <p className="mt-2 text-sm leading-6 text-muted-foreground">{caption}</p>
    </article>
  );
}

function QueryStateBanner({
  tone,
  message,
}: {
  tone: "warning" | "error";
  message: string;
}) {
  return (
    <div
      className={cn(
        "rounded-[1.2rem] border px-4 py-3 text-sm leading-6",
        tone === "error"
          ? "border-destructive/20 bg-destructive/5 text-destructive"
          : "border-amber-500/20 bg-amber-500/8 text-amber-700 dark:text-amber-200",
      )}
    >
      {message}
    </div>
  );
}

export function DiagnosisPanel() {
  const [timeRange, setTimeRange] = useState<DiagnosisTimeRangeMinutes>(30);
  const setCurrentPage = useAppStore((state) => state.setCurrentPage);
  const { data: aiSettings } = useAiSettingsQuery();
  const overviewQuery = useDiagnosisOverview(timeRange);
  const { runDiagnosis, isLoading, data: report, error, reset } = useQuickDiagnosis();
  const isConfigured = isAiConfigured(aiSettings);

  useEffect(() => {
    reset();
  }, [timeRange, reset]);

  const summary = overviewQuery.data?.summary;
  const alerts = overviewQuery.data?.alerts ?? [];
  const totalErrorCount = useMemo(() => getTotalErrorCount(summary), [summary]);
  const leadingFailureHost = summary?.topFailureHosts[0];
  const leadingErrorNode = summary?.topErrorNodes[0];
  const reportContent = report?.report.trim() ?? "";

  const metricCards = [
    {
      label: "采样连接",
      value: formatCount(summary?.totalConnections ?? 0),
      caption: `最近 ${timeRange} 分钟累计连接样本`,
      icon: Activity,
      accentClassName: "bg-primary/12 text-primary ring-primary/20",
    },
    {
      label: "错误事件",
      value: formatCount(totalErrorCount),
      caption:
        leadingErrorNode !== undefined
          ? `${leadingErrorNode.proxyNode} 是当前最高频错误节点`
          : "当前窗口没有新的错误聚合事件",
      icon: AlertTriangle,
      accentClassName: "bg-amber-500/12 text-amber-300 ring-amber-500/20",
    },
    {
      label: "DNS 错误",
      value: formatCount(summary?.dnsErrorCount ?? 0),
      caption: "单独追踪解析失败聚类，便于区分网络与规则问题",
      icon: AlertCircle,
      accentClassName: "bg-red-500/12 text-red-300 ring-red-500/20",
    },
    {
      label: "MATCH 兜底",
      value: formatCount(summary?.matchFallbackCount ?? 0),
      caption:
        leadingFailureHost !== undefined
          ? `${leadingFailureHost.host} 失败率 ${formatPercent(leadingFailureHost.failureRate)}`
          : "尚未识别到高失败率主机",
      icon: Info,
      accentClassName: "bg-blue-500/12 text-blue-300 ring-blue-500/20",
    },
  ] as const;

  const handleOpenSettings = () => {
    setCurrentPage("settings");
  };

  const handleRunDiagnosis = () => {
    void runDiagnosis(timeRange);
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.24, ease: "easeOut" }}
      className="flex min-h-0 flex-1 flex-col gap-4"
    >
      <SectionCard glow className="space-y-5">
        <div className="flex flex-col gap-5 xl:flex-row xl:items-start xl:justify-between">
          <div className="max-w-2xl">
            <div className="inline-flex items-center gap-2 rounded-full border border-primary/15 bg-primary/10 px-3 py-1 text-xs font-medium tracking-[0.18em] text-primary uppercase">
              <Activity className="size-3.5" />
              Diagnostic Surface
            </div>
            <h2 className="mt-4 text-2xl font-semibold tracking-tight text-foreground">
              AI 诊断结果页
            </h2>
            <p className="mt-2 text-sm leading-6 text-muted-foreground">
              摘要和告警每分钟自动刷新。AI 诊断报告仅在你手动触发时生成，适合先看信号，再决定是否需要更重的推理分析。
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <label className="inline-flex items-center gap-3 rounded-full border border-border/70 bg-background/80 px-3 py-2 text-sm text-muted-foreground">
              <span className="text-xs font-medium tracking-[0.16em] uppercase">时间窗口</span>
              <select
                value={timeRange}
                onChange={(event) =>
                  setTimeRange(Number(event.target.value) as DiagnosisTimeRangeMinutes)
                }
                className="bg-transparent text-sm text-foreground outline-none"
                aria-label="诊断时间范围"
              >
                {DIAGNOSIS_TIME_RANGE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <button
              type="button"
              onClick={handleRunDiagnosis}
              disabled={!isConfigured || isLoading}
              className={cn(
                "inline-flex items-center justify-center gap-2 rounded-full border px-4 py-2.5 text-sm font-medium transition-all",
                "border-primary/20 bg-primary text-primary-foreground shadow-[0_18px_42px_-24px_var(--color-primary)] hover:translate-y-[-1px] hover:bg-primary/92",
                "disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:translate-y-0",
              )}
            >
              {isLoading ? (
                <RefreshCw className="size-4 animate-spin" />
              ) : (
                <FileText className="size-4" />
              )}
              {isLoading ? "诊断中..." : "一键诊断"}
            </button>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <div className="inline-flex items-center gap-2 rounded-full border border-border/70 bg-muted/35 px-3 py-2 text-xs tracking-[0.16em] text-muted-foreground uppercase">
            <RefreshCw
              className={cn(
                "size-3.5",
                overviewQuery.isFetching ? "animate-spin" : "",
              )}
            />
            每分钟自动刷新
          </div>
          <div className="rounded-full border border-border/70 bg-background/80 px-3 py-2 text-xs tracking-[0.16em] text-muted-foreground uppercase">
            最近更新 {formatTimestamp(summary?.generatedAt)}
          </div>
          <div className="rounded-full border border-border/70 bg-background/80 px-3 py-2 text-xs tracking-[0.16em] text-muted-foreground uppercase">
            告警 {alerts.length}
          </div>
        </div>

        {!isConfigured ? (
          <QueryStateBanner
            tone="warning"
            message="AI 尚未配置。你仍然可以查看自动轮询的摘要与告警；若要生成自然语言诊断报告，请前往设置页完成 Provider、模型和凭据配置。"
          />
        ) : null}

        {overviewQuery.error ? (
          <QueryStateBanner tone="error" message={overviewQuery.error.message} />
        ) : null}

        <div className="grid gap-3 md:grid-cols-2 2xl:grid-cols-4">
          {metricCards.map((item) => (
            <SummaryMetricCard
              key={item.label}
              label={item.label}
              value={item.value}
              caption={item.caption}
              icon={item.icon}
              accentClassName={item.accentClassName}
            />
          ))}
        </div>

        <div className="grid gap-3 lg:grid-cols-2">
          <div className="rounded-[1.2rem] border border-border/70 bg-background/75 px-4 py-3">
            <div className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
              最高频错误节点
            </div>
            <div className="mt-2 text-sm font-medium text-foreground">
              {leadingErrorNode?.proxyNode ?? "暂无样本"}
            </div>
            <p className="mt-1 text-sm text-muted-foreground">
              {leadingErrorNode !== undefined
                ? `${formatCount(leadingErrorNode.count)} 次异常命中`
                : "当前窗口内还没有节点级错误聚合结果。"}
            </p>
          </div>

          <div className="rounded-[1.2rem] border border-border/70 bg-background/75 px-4 py-3">
            <div className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
              最高失败率主机
            </div>
            <div className="mt-2 truncate text-sm font-medium text-foreground">
              {leadingFailureHost?.host ?? "暂无样本"}
            </div>
            <p className="mt-1 text-sm text-muted-foreground">
              {leadingFailureHost !== undefined
                ? `${formatPercent(leadingFailureHost.failureRate)} · ${formatCount(leadingFailureHost.failureCount)} / ${formatCount(leadingFailureHost.totalCount)}`
                : "当前窗口内还没有主机失败率聚合结果。"}
            </p>
          </div>
        </div>
      </SectionCard>

      <div className="grid min-h-0 flex-1 gap-4 xl:grid-cols-[minmax(22rem,0.88fr)_minmax(0,1.12fr)]">
        <SectionCard className="flex min-h-0 flex-col gap-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
                Alert Feed
              </p>
              <h3 className="mt-1 text-lg font-semibold text-foreground">
                异常告警列表
              </h3>
            </div>
            <div className="rounded-full border border-border/70 bg-background/80 px-3 py-2 text-xs tracking-[0.16em] text-muted-foreground uppercase">
              {alerts.length} 条
            </div>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto pr-1">
            {overviewQuery.isLoading && alerts.length === 0 ? (
              <div className="flex h-full min-h-[18rem] items-center justify-center rounded-[1.35rem] border border-dashed border-border/70 bg-muted/10 px-6 text-sm text-muted-foreground">
                <div className="flex items-center gap-2">
                  <RefreshCw className="size-4 animate-spin text-primary" />
                  正在聚合当前窗口的异常告警...
                </div>
              </div>
            ) : alerts.length > 0 ? (
              <div className="space-y-3">
                {alerts.map((alert) => (
                  <DiagnosisAlertCard key={alert.id} alert={alert} />
                ))}
              </div>
            ) : (
              <EmptyState
                icon={Activity}
                title="当前窗口没有新告警"
                description="系统仍会继续按分钟轮询诊断信号。你可以扩大时间范围，或者手动触发 AI 诊断生成更完整的分析报告。"
              />
            )}
          </div>
        </SectionCard>

        <SectionCard className="flex min-h-0 flex-col gap-4">
          <div className="flex items-start justify-between gap-3">
            <div>
              <p className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
                Markdown Report
              </p>
              <h3 className="mt-1 text-lg font-semibold text-foreground">
                AI 诊断报告
              </h3>
              <p className="mt-1 text-sm text-muted-foreground">
                生成时间 {formatTimestamp(report?.generatedAt)}
              </p>
            </div>

            {isLoading ? (
              <div className="inline-flex items-center gap-2 rounded-full border border-primary/20 bg-primary/10 px-3 py-2 text-xs tracking-[0.16em] text-primary uppercase">
                <RefreshCw className="size-3.5 animate-spin" />
                推理中
              </div>
            ) : null}
          </div>

          {error ? <QueryStateBanner tone="error" message={error.message} /> : null}

          <div className="min-h-0 flex-1 overflow-y-auto pr-1">
            {report !== null && reportContent.length > 0 ? (
              <div className="rounded-[1.4rem] border border-border/70 bg-background/82 p-5 shadow-[0_24px_72px_-42px_rgba(15,23,42,0.55)]">
                <div className="mb-4 flex flex-wrap items-center gap-2">
                  <span className="rounded-full border border-primary/20 bg-primary/10 px-3 py-1 text-xs tracking-[0.16em] text-primary uppercase">
                    最近 {report.summary.timeRangeMinutes} 分钟
                  </span>
                  <span className="rounded-full border border-border/70 bg-background/85 px-3 py-1 text-xs tracking-[0.16em] text-muted-foreground uppercase">
                    {report.alerts.length} 条告警输入
                  </span>
                </div>

                <div className="text-sm leading-7 text-foreground">
                  <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                    {report.report}
                  </ReactMarkdown>
                </div>
              </div>
            ) : isLoading ? (
              <div className="flex h-full min-h-[18rem] items-center justify-center rounded-[1.35rem] border border-dashed border-primary/20 bg-primary/5 px-6 text-center text-sm text-muted-foreground">
                <div>
                  <RefreshCw className="mx-auto size-5 animate-spin text-primary" />
                  <p className="mt-3 font-medium text-foreground">AI 正在生成诊断报告</p>
                  <p className="mt-1 leading-6">
                    将结合当前窗口的摘要与告警，输出一份适合直接阅读的 Markdown 分析。
                  </p>
                </div>
              </div>
            ) : (
              <EmptyState
                icon={FileText}
                title={isConfigured ? "尚未生成 AI 诊断报告" : "配置 AI 后可生成诊断报告"}
                description={
                  isConfigured
                    ? "点击上方“一键诊断”，让模型结合当前告警和摘要给出原因分析、排查方向与下一步建议。"
                    : "当前页面仍会自动展示摘要和告警，但自然语言诊断需要 AI Provider、模型和凭据配置。"
                }
                action={
                  isConfigured ? (
                    <button
                      type="button"
                      onClick={handleRunDiagnosis}
                      className="inline-flex items-center gap-2 rounded-full border border-primary/20 bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-[0_18px_42px_-24px_var(--color-primary)] transition-colors hover:bg-primary/92"
                    >
                      <FileText className="size-4" />
                      立即诊断
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={handleOpenSettings}
                      className="inline-flex items-center gap-2 rounded-full border border-primary/20 bg-primary/10 px-4 py-2 text-sm font-medium text-primary transition-colors hover:bg-primary/15"
                    >
                      <Info className="size-4" />
                      前往设置 AI
                    </button>
                  )
                }
              />
            )}
          </div>
        </SectionCard>
      </div>
    </motion.div>
  );
}
