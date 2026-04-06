import { motion } from "framer-motion";
import { format } from "date-fns";
import {
  CalendarRange,
  FileText,
  Gauge,
  RadioTower,
  RefreshCw,
  Settings2,
  Sparkles,
  TrendingUp,
} from "lucide-react";
import { useMemo, useState, useTransition } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useAiReport } from "./hooks/use-ai-report";
import { isAiConfigured, useAiSettingsQuery } from "./hooks/use-ai-settings";
import { markdownComponents } from "./markdown-components";
import type { ReportResult, ReportType } from "@/lib/tauri-api";
import { cn, formatBytes } from "@/lib/utils";
import { useAppStore } from "@/stores/app-store";
import { integerFormatter } from "@/features/stats/constants";

const REPORT_TYPE_OPTIONS: Array<{
  id: ReportType;
  label: string;
  caption: string;
}> = [
  { id: "daily", label: "日报", caption: "单日行为切片" },
  { id: "weekly", label: "周报", caption: "一周模式解读" },
];

function getDefaultReportDate() {
  return format(new Date(), "yyyy-MM-dd");
}

function formatPeriodLabel(report: ReportResult) {
  return report.type === "daily"
    ? report.period.end
    : `${report.period.start} ~ ${report.period.end}`;
}

function formatSignedPercent(value: number | undefined) {
  if (value === undefined || !Number.isFinite(value)) {
    return "0%";
  }

  const prefix = value > 0 ? "+" : "";
  return `${prefix}${value.toFixed(1)}%`;
}

function formatPercent(value: number | undefined) {
  if (value === undefined || !Number.isFinite(value)) {
    return "0%";
  }

  return `${value.toFixed(1)}%`;
}

function formatPeakHour(value: string | undefined) {
  if (!value) {
    return "暂无峰值";
  }

  return value.replace("T", " ").replace(":00Z", " UTC").replace("Z", " UTC");
}

function hasMeaningfulData(report: ReportResult | undefined) {
  if (!report) {
    return false;
  }

  const totalTraffic = report.stats.totalTraffic.upload + report.stats.totalTraffic.download;
  return (
    totalTraffic > 0 ||
    report.stats.totalConnections > 0 ||
    report.stats.topDomains.length > 0 ||
    report.stats.topRules.length > 0
  );
}

function leadingDomain(report: ReportResult | undefined) {
  return report?.stats.topDomains[0];
}

function trailingGeneratedLabel(generatedAt: string | undefined) {
  if (!generatedAt) {
    return "尚未生成";
  }

  const parsed = new Date(generatedAt);
  if (Number.isNaN(parsed.valueOf())) {
    return generatedAt;
  }

  return format(parsed, "MM-dd HH:mm");
}

function MetricCard({
  label,
  value,
  caption,
  icon: Icon,
}: {
  label: string;
  value: string;
  caption: string;
  icon: typeof Gauge;
}) {
  return (
    <article className="rounded-[1.3rem] border border-border/70 bg-background/70 p-4 shadow-[0_20px_60px_-44px_rgba(15,23,42,0.65)]">
      <div className="flex items-center justify-between gap-3">
        <span className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
          {label}
        </span>
        <span className="inline-flex size-9 items-center justify-center rounded-full border border-primary/15 bg-primary/10 text-primary">
          <Icon className="size-4" />
        </span>
      </div>
      <div className="mt-3 text-lg font-semibold tracking-tight text-foreground">{value}</div>
      <p className="mt-2 text-sm leading-6 text-muted-foreground">{caption}</p>
    </article>
  );
}

function DataStrip({
  label,
  value,
  emphasis,
}: {
  label: string;
  value: string;
  emphasis?: boolean;
}) {
  return (
    <div
      className={cn(
        "rounded-full border px-3 py-2 text-xs tracking-[0.16em] uppercase",
        emphasis
          ? "border-primary/20 bg-primary/10 text-primary"
          : "border-border/70 bg-background/70 text-muted-foreground",
      )}
    >
      <span className="mr-2 opacity-80">{label}</span>
      <span className="font-medium">{value}</span>
    </div>
  );
}

function RankedList({
  title,
  items,
  emptyLabel,
}: {
  title: string;
  items: Array<{ label: string; value: string }>;
  emptyLabel: string;
}) {
  return (
    <section className="rounded-[1.35rem] border border-border/70 bg-background/65 p-4">
      <div className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
        {title}
      </div>
      <div className="mt-4 space-y-3">
        {items.length === 0 ? (
          <p className="text-sm leading-6 text-muted-foreground">{emptyLabel}</p>
        ) : (
          items.map((item, index) => (
            <div
              key={`${title}-${item.label}-${index}`}
              className="flex items-start justify-between gap-4 border-b border-border/60 pb-3 last:border-b-0 last:pb-0"
            >
              <div className="min-w-0">
                <div className="text-[11px] tracking-[0.18em] text-muted-foreground uppercase">
                  #{index + 1}
                </div>
                <div className="mt-1 truncate text-sm font-medium text-foreground">
                  {item.label}
                </div>
              </div>
              <div className="shrink-0 text-sm font-medium text-primary">{item.value}</div>
            </div>
          ))
        )}
      </div>
    </section>
  );
}

function EmptyPanel({
  isConfigured,
  onOpenSettings,
}: {
  isConfigured: boolean;
  onOpenSettings: () => void;
}) {
  return (
    <div className="relative overflow-hidden rounded-[1.8rem] border border-dashed border-border/70 bg-muted/10 px-6 py-10 text-center">
      <div className="pointer-events-none absolute inset-x-10 top-0 h-24 bg-linear-to-r from-transparent via-primary/12 to-transparent blur-2xl" />
      <div className="relative mx-auto max-w-sm">
        <div className="mx-auto inline-flex size-14 items-center justify-center rounded-[1.35rem] border border-primary/20 bg-primary/10 text-primary shadow-[0_18px_50px_-32px_var(--color-primary)]">
          <FileText className="size-6" />
        </div>
        <h3 className="mt-5 text-xl font-semibold tracking-tight text-foreground">
          统计洞察工作台
        </h3>
        <p className="mt-3 text-sm leading-7 text-muted-foreground">
          选择日报或周报、指定结束日期，系统会先在 Rust 端聚合统计，再由模型输出一份完整的 Markdown 报告。
        </p>
        {!isConfigured ? (
          <button
            type="button"
            onClick={onOpenSettings}
            className="mt-5 inline-flex items-center gap-2 rounded-full border border-primary/20 bg-primary/10 px-4 py-2 text-sm font-medium text-primary transition-colors hover:bg-primary/15"
          >
            <Settings2 className="size-4" />
            前往设置 AI
          </button>
        ) : null}
      </div>
    </div>
  );
}

export function ReportViewer() {
  const setCurrentPage = useAppStore((state) => state.setCurrentPage);
  const { data: aiSettings } = useAiSettingsQuery();
  const [reportType, setReportType] = useState<ReportType>("daily");
  const [selectedDate, setSelectedDate] = useState(getDefaultReportDate);
  const [isTransitionPending, startTransition] = useTransition();
  const reportMutation = useAiReport();
  const report = reportMutation.data;
  const isConfigured = isAiConfigured(aiSettings);
  const resolvedHasData = hasMeaningfulData(report);
  const totalTraffic = report
    ? report.stats.totalTraffic.upload + report.stats.totalTraffic.download
    : 0;
  const dominantDomain = leadingDomain(report);
  const topDomainItems = useMemo(
    () =>
      (report?.stats.topDomains ?? []).map((item) => ({
        label: item.domain,
        value: formatBytes(item.traffic),
      })),
    [report],
  );
  const topRuleItems = useMemo(
    () =>
      (report?.stats.topRules ?? []).map((item) => ({
        label: item.rule,
        value: `${integerFormatter.format(item.hitCount)} 次`,
      })),
    [report],
  );

  const handleGenerate = () => {
    reportMutation.mutate({
      type: reportType,
      date: selectedDate || undefined,
    });
  };

  const handleTypeChange = (nextType: ReportType) => {
    startTransition(() => {
      setReportType(nextType);
    });
  };

  const handleDateChange = (value: string) => {
    startTransition(() => {
      setSelectedDate(value);
    });
  };

  return (
    <motion.section
      initial={{ opacity: 0, x: 18 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ duration: 0.28, ease: "easeOut" }}
      className="relative overflow-hidden rounded-[2rem] border border-border/70 bg-linear-to-br from-primary/12 via-background/96 to-background/94 p-5 shadow-[0_28px_100px_-52px_rgba(15,23,42,0.62)]"
    >
      <div className="pointer-events-none absolute -right-12 top-0 size-32 rounded-full bg-primary/12 blur-3xl" />
      <div className="pointer-events-none absolute bottom-0 left-0 h-32 w-64 bg-linear-to-r from-primary/10 to-transparent" />

      <div className="relative">
        <header className="flex flex-col gap-5 border-b border-border/70 pb-5">
          <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
            <div className="max-w-2xl">
              <div className="inline-flex items-center gap-2 rounded-full border border-primary/15 bg-primary/10 px-3 py-1 text-xs font-medium tracking-[0.18em] text-primary uppercase">
                <Sparkles className="size-3.5" />
                Insight Atelier
              </div>
              <h2 className="mt-4 text-2xl font-semibold tracking-tight text-foreground">
                AI 统计报告
              </h2>
              <p className="mt-2 text-sm leading-6 text-muted-foreground">
                这里聚焦于结构化统计解读。日报适合看单日波动与异常，周报更适合看策略分布、MATCH 兜底率和一周节奏变化。
              </p>
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <DataStrip
                label="最近生成"
                value={trailingGeneratedLabel(report?.generatedAt)}
                emphasis
              />
              <DataStrip
                label="模型"
                value={isConfigured && aiSettings ? aiSettings.model : "未配置"}
              />
            </div>
          </div>

          <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_12rem_auto]">
            <div className="rounded-[1.4rem] border border-border/70 bg-background/70 p-3">
              <div className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
                报告类型
              </div>
              <div className="mt-3 flex flex-wrap gap-2">
                {REPORT_TYPE_OPTIONS.map((option) => {
                  const isActive = option.id === reportType;

                  return (
                    <button
                      key={option.id}
                      type="button"
                      onClick={() => handleTypeChange(option.id)}
                      className={cn(
                        "rounded-full border px-3 py-2 text-left transition-all",
                        isActive
                          ? "border-primary/20 bg-primary text-primary-foreground shadow-[0_12px_32px_-18px_var(--color-primary)]"
                          : "border-border/70 bg-background/85 text-muted-foreground hover:border-primary/20 hover:text-foreground",
                      )}
                    >
                      <span className="text-sm font-medium">{option.label}</span>
                      <span
                        className={cn(
                          "ml-2 text-xs",
                          isActive ? "text-primary-foreground/80" : "text-muted-foreground",
                        )}
                      >
                        {option.caption}
                      </span>
                    </button>
                  );
                })}
              </div>
            </div>

            <label className="rounded-[1.4rem] border border-border/70 bg-background/70 p-3">
              <div className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
                {reportType === "daily" ? "报告日期" : "结束日期"}
              </div>
              <div className="mt-3 flex items-center gap-2 rounded-full border border-border/70 bg-background/70 px-3 py-2">
                <CalendarRange className="size-4 text-primary" />
                <input
                  type="date"
                  value={selectedDate}
                  onChange={(event) => handleDateChange(event.target.value)}
                  className="w-full bg-transparent text-sm text-foreground outline-none"
                />
              </div>
            </label>

            <button
              type="button"
              onClick={handleGenerate}
              disabled={!isConfigured || reportMutation.isPending}
              className={cn(
                "inline-flex items-center justify-center gap-2 rounded-[1.4rem] border px-4 py-3 text-sm font-medium transition-all",
                "border-primary/20 bg-primary text-primary-foreground shadow-[0_20px_48px_-28px_var(--color-primary)] hover:translate-y-[-1px] hover:bg-primary/92",
                "disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:translate-y-0",
              )}
            >
              {reportMutation.isPending ? (
                <RefreshCw className="size-4 animate-spin" />
              ) : (
                <Sparkles className="size-4" />
              )}
              {reportMutation.isPending ? "生成中" : "生成报告"}
            </button>
          </div>
        </header>

        <div className="mt-5 space-y-4">
          {reportMutation.error ? (
            <div className="rounded-[1.3rem] border border-destructive/20 bg-destructive/5 px-4 py-3 text-sm text-destructive">
              {reportMutation.error.message}
            </div>
          ) : null}

          {!report ? (
            <EmptyPanel
              isConfigured={isConfigured}
              onOpenSettings={() => setCurrentPage("settings")}
            />
          ) : (
            <>
              <div className="grid gap-3 sm:grid-cols-2 2xl:grid-cols-4">
                <MetricCard
                  label="总流量"
                  value={formatBytes(totalTraffic)}
                  caption={`下载 ${formatBytes(report.stats.totalTraffic.download)} · 上传 ${formatBytes(report.stats.totalTraffic.upload)}`}
                  icon={RadioTower}
                />
                <MetricCard
                  label="连接总数"
                  value={integerFormatter.format(report.stats.totalConnections)}
                  caption={`${formatPeriodLabel(report)} 内记录到的连接数量`}
                  icon={Gauge}
                />
                <MetricCard
                  label="领先域名"
                  value={dominantDomain?.domain ?? "暂无数据"}
                  caption={
                    dominantDomain
                      ? `${formatBytes(dominantDomain.traffic)} 流量集中在该域名`
                      : "当前窗口还没有域名聚合样本"
                  }
                  icon={TrendingUp}
                />
                <MetricCard
                  label="环比变化"
                  value={formatSignedPercent(report.stats.comparison?.trafficChange)}
                  caption={`连接数 ${formatSignedPercent(report.stats.comparison?.connectionChange)}`}
                  icon={Sparkles}
                />
              </div>

              <div className="flex flex-wrap gap-2">
                <DataStrip label="报告周期" value={formatPeriodLabel(report)} emphasis />
                <DataStrip label="峰值时段" value={formatPeakHour(report.stats.peakHour)} />
                <DataStrip
                  label="MATCH 比例"
                  value={formatPercent(report.stats.matchRate)}
                />
                <DataStrip
                  label="样本状态"
                  value={resolvedHasData ? "已有聚合样本" : "样本不足"}
                />
              </div>

              {!resolvedHasData ? (
                <div className="rounded-[1.3rem] border border-amber-500/20 bg-amber-500/8 px-4 py-3 text-sm leading-6 text-amber-200">
                  当前时间窗口内没有足够的统计样本。报告会保留结构，但重点会转为说明数据缺口，而不是推断不存在的行为。
                </div>
              ) : null}

              <div className="grid gap-4 2xl:grid-cols-[minmax(0,1.1fr)_minmax(16rem,0.9fr)]">
                <article className="overflow-hidden rounded-[1.7rem] border border-border/70 bg-background/85 shadow-[0_24px_80px_-42px_rgba(15,23,42,0.58)]">
                  <div className="border-b border-border/70 px-5 py-4">
                    <div className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
                      Markdown Report
                    </div>
                    <h3 className="mt-2 text-lg font-semibold tracking-tight text-foreground">
                      {report.type === "daily" ? "每日使用报告" : "每周使用报告"}
                    </h3>
                    <p className="mt-1 text-sm text-muted-foreground">
                      生成时间 {trailingGeneratedLabel(report.generatedAt)} · 支持表格、列表与强调标记。
                    </p>
                  </div>
                  <div className="max-h-[42rem] overflow-y-auto px-5 py-5">
                    <div className="text-sm leading-7 text-foreground">
                      <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                        {report.content}
                      </ReactMarkdown>
                    </div>
                  </div>
                </article>

                <div className="space-y-4">
                  <RankedList
                    title="Top Domains"
                    items={topDomainItems}
                    emptyLabel="当前周期没有域名流量聚合。"
                  />
                  <RankedList
                    title="Top Rules"
                    items={topRuleItems}
                    emptyLabel="当前周期没有规则命中聚合。"
                  />
                </div>
              </div>
            </>
          )}
        </div>
      </div>

      {isTransitionPending ? (
        <div className="pointer-events-none absolute right-5 top-5 inline-flex items-center gap-2 rounded-full border border-primary/20 bg-primary/10 px-3 py-2 text-xs tracking-[0.18em] text-primary uppercase">
          <RefreshCw className="size-3.5 animate-spin" />
          更新中
        </div>
      ) : null}
    </motion.section>
  );
}
