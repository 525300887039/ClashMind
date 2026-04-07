import { useState, useTransition } from "react";
import {
  ChartColumnBig,
  ChartNoAxesCombined,
  Globe,
  LayoutDashboard,
  MapPinned,
} from "lucide-react";
import { useStatsOverview } from "@/features/stats/hooks/use-stats";
import { cn, formatBytes } from "@/lib/utils";
import { integerFormatter } from "./constants";
import { DomainStats } from "./domain-stats";
import { GeoMap } from "./geo-map";
import { TrafficTimeline } from "./traffic-timeline";
import { PageHeader } from "@/components/ui/page-header";
import { StatusBadge } from "@/components/ui/status-badge";

type StatsTab = "overview" | "domains" | "traffic" | "geo";

const OVERVIEW_DAYS = 7;

const STATS_TABS: {
  id: StatsTab;
  label: string;
  description: string;
  icon: React.ComponentType<{ className?: string }>;
}[] = [
  {
    id: "overview",
    label: "概览",
    description: "近 7 天关键摘要",
    icon: LayoutDashboard,
  },
  {
    id: "domains",
    label: "域名",
    description: "按域名分析流量",
    icon: Globe,
  },
  {
    id: "traffic",
    label: "流量",
    description: "上传、下载与连接数趋势",
    icon: ChartColumnBig,
  },
  {
    id: "geo",
    label: "地理",
    description: "按国家查看连接热区",
    icon: MapPinned,
  },
];

export function StatsPage() {
  const [activeTab, setActiveTab] = useState<StatsTab>("overview");
  const [isPending, startTransition] = useTransition();

  return (
    <section className="flex flex-col gap-6">
      <PageHeader
        eyebrow="Traffic Observatory"
        eyebrowIcon={ChartNoAxesCombined}
        title="统计仪表板"
        description="聚合域名、流量与地理维度的数据采样。当前已接入域名分析、流量趋势和地理分布，可以从同一窗口快速定位带宽热点。"
        actions={<StatusBadge busy={isPending} busyText="切换中" readyText="视图已就绪" />}
      >
        <div className="grid gap-3 lg:grid-cols-4">
            {STATS_TABS.map(({ id, label, description, icon: Icon }) => {
              const isActive = id === activeTab;

              return (
                <button
                  key={id}
                  type="button"
                  onClick={() => startTransition(() => setActiveTab(id))}
                  className={cn(
                    "group rounded-[1.5rem] border p-4 text-left transition-all",
                    isActive
                      ? "border-primary/20 bg-primary/10 shadow-[0_20px_56px_-34px_var(--color-primary)]"
                      : "border-border/70 bg-background/85 hover:border-primary/20 hover:bg-muted/30",
                  )}
                >
                  <div className="flex items-center justify-between">
                    <div
                      className={cn(
                        "rounded-full p-2 transition-colors",
                        isActive
                          ? "bg-primary text-primary-foreground"
                          : "bg-muted text-muted-foreground group-hover:text-foreground",
                      )}
                    >
                      <Icon className="size-4" />
                    </div>
                    <span
                      className={cn(
                        "text-xs font-medium uppercase tracking-[0.16em]",
                        isActive ? "text-primary" : "text-muted-foreground",
                      )}
                    >
                      {label}
                    </span>
                  </div>
                  <div className="mt-4">
                    <div className="text-base font-semibold text-foreground">{label}</div>
                    <p className="mt-1 text-sm text-muted-foreground">{description}</p>
                  </div>
                </button>
              );
            })}
          </div>
      </PageHeader>

      {activeTab === "overview" && <StatsOverviewPanel />}
      {activeTab === "domains" && <DomainStats />}
      {activeTab === "traffic" && <TrafficTimeline />}
      {activeTab === "geo" && <GeoMap />}
    </section>
  );
}

function StatsOverviewPanel() {
  const { data, isLoading, isFetching, error } = useStatsOverview(OVERVIEW_DAYS);

  if (isLoading) {
    return (
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {Array.from({ length: 4 }, (_, index) => (
          <div
            key={index}
            className="h-40 animate-pulse rounded-[1.5rem] border border-border/70 bg-muted/25"
          />
        ))}
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-[1.5rem] border border-destructive/20 bg-destructive/5 px-5 py-4 text-sm text-destructive">
        加载统计概览失败: {error.message}
      </div>
    );
  }

  if (!data) {
    return (
      <div className="rounded-[1.5rem] border border-border/70 bg-background/90 px-5 py-4 text-sm text-muted-foreground">
        当前没有可展示的统计概览数据。
      </div>
    );
  }

  const totalTraffic = data.totalUpload + data.totalDownload;

  return (
    <div className="grid gap-5 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
      <div className="grid gap-4 md:grid-cols-2">
        <OverviewCard
          eyebrow="近 7 天"
          label="总连接数"
          value={integerFormatter.format(data.totalConnections)}
          description="窗口内已归档和活跃连接总和"
          tone="primary"
        />
        <OverviewCard
          eyebrow="自动汇总"
          label="总流量"
          value={formatBytes(totalTraffic)}
          description={`下载 ${formatBytes(data.totalDownload)} · 上传 ${formatBytes(data.totalUpload)}`}
          tone="neutral"
        />
        <OverviewCard
          eyebrow="实时状态"
          label="活跃连接"
          value={integerFormatter.format(data.activeConnections)}
          description="当前仍处于打开状态的连接数"
          tone="success"
        />
        <OverviewCard
          eyebrow="域名画像"
          label="唯一域名"
          value={integerFormatter.format(data.uniqueDomains)}
          description="近 7 天已观测到的唯一域名数量"
          tone="warning"
        />
      </div>

      <aside className="rounded-[1.5rem] border border-border/70 bg-background/95 p-5 shadow-[0_24px_80px_-40px_rgba(15,23,42,0.45)]">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-xs font-medium tracking-[0.18em] text-muted-foreground uppercase">
              观测摘要
            </p>
            <h2 className="mt-2 text-xl font-semibold tracking-tight text-foreground">
              网络活动概况
            </h2>
          </div>
          <StatusBadge busy={isFetching} />
        </div>

        <div className="mt-6 space-y-4">
          <InsightRow
            label="连接密度"
            value={data.totalConnections === 0 ? "无活动" : "有采样"}
            description={
              data.activeConnections > 0
                ? `仍有 ${integerFormatter.format(data.activeConnections)} 个连接保持活跃。`
                : "当前没有处于打开状态的连接。"
            }
          />
          <InsightRow
            label="下载占比"
            value={
              totalTraffic === 0
                ? "0%"
                : `${((data.totalDownload / totalTraffic) * 100).toFixed(1)}%`
            }
            description="用来判断当前网络使用更偏下行还是上行。"
          />
          <InsightRow
            label="域名覆盖"
            value={integerFormatter.format(data.uniqueDomains)}
            description="可切到“域名”标签查看详细排行和流量分布。"
          />
        </div>
      </aside>
    </div>
  );
}

function OverviewCard({
  eyebrow,
  label,
  value,
  description,
  tone,
}: {
  eyebrow: string;
  label: string;
  value: string;
  description: string;
  tone: "primary" | "neutral" | "success" | "warning";
}) {
  const toneClassName = {
    primary: "from-primary/16 to-primary/4",
    neutral: "from-muted/70 to-background",
    success: "from-success/12 to-background",
    warning: "from-warning/12 to-background",
  }[tone];

  return (
    <article
      className={cn(
        "rounded-[1.5rem] border border-border/70 bg-linear-to-br p-5 shadow-[0_24px_80px_-40px_rgba(15,23,42,0.45)]",
        toneClassName,
      )}
    >
      <p className="text-xs font-medium tracking-[0.18em] text-muted-foreground uppercase">
        {eyebrow}
      </p>
      <div className="mt-6 text-sm font-medium text-foreground">{label}</div>
      <div className="mt-2 text-3xl font-semibold tracking-tight text-foreground">
        {value}
      </div>
      <p className="mt-3 text-sm leading-6 text-muted-foreground">{description}</p>
    </article>
  );
}

function InsightRow({
  label,
  value,
  description,
}: {
  label: string;
  value: string;
  description: string;
}) {
  return (
    <div className="rounded-[1.25rem] border border-border/70 bg-muted/20 p-4">
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm font-medium text-foreground">{label}</span>
        <span className="text-sm font-semibold text-primary">{value}</span>
      </div>
      <p className="mt-2 text-sm text-muted-foreground">{description}</p>
    </div>
  );
}
