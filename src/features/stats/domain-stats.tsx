import { useDeferredValue, useId, useState, useTransition } from "react";
import {
  BarChart3,
  ChevronDown,
  ChevronUp,
  RefreshCw,
  Search,
} from "lucide-react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { DomainStat } from "@/lib/tauri-api";
import { cn, formatBytes } from "@/lib/utils";
import { useDomainStats } from "./hooks/use-stats";

type DomainRange = 0 | 7 | 30;
type SortKey = "rank" | "domain" | "hitCount" | "upload" | "download" | "total";
type SortDirection = "asc" | "desc";

interface DomainRow extends DomainStat {
  displayDomain: string;
  normalizedDomain: string;
  rank: number;
  total: number;
}

interface ChartRow {
  domain: string;
  domainLabel: string;
  hitCount: number;
  upload: number;
  download: number;
  total: number;
}

const DOMAIN_TABLE_LIMIT = 2_147_483_647;

const RANGE_OPTIONS: { days: DomainRange; label: string; caption: string }[] = [
  { days: 0, label: "1天", caption: "今天" },
  { days: 7, label: "7天", caption: "近 7 天" },
  { days: 30, label: "30天", caption: "近 30 天" },
];

const TABLE_COLUMNS: { key: SortKey; label: string; align?: "left" | "right" }[] = [
  { key: "rank", label: "排名" },
  { key: "domain", label: "域名" },
  { key: "hitCount", label: "访问次数", align: "right" },
  { key: "upload", label: "上传", align: "right" },
  { key: "download", label: "下载", align: "right" },
  { key: "total", label: "总流量", align: "right" },
];

const integerFormatter = new Intl.NumberFormat("zh-CN");

function normalizeDomain(domain: string): string {
  const trimmed = domain.trim();
  return trimmed.length > 0 ? trimmed : "UNKNOWN";
}

function getSortValue(row: DomainRow, key: SortKey): number | string {
  switch (key) {
    case "rank":
      return row.rank;
    case "domain":
      return row.normalizedDomain;
    case "hitCount":
      return row.hitCount;
    case "upload":
      return row.upload;
    case "download":
      return row.download;
    case "total":
      return row.total;
  }
}

function compareDomainRows(
  left: DomainRow,
  right: DomainRow,
  key: SortKey,
  direction: SortDirection,
): number {
  const leftValue = getSortValue(left, key);
  const rightValue = getSortValue(right, key);

  const result =
    typeof leftValue === "string" && typeof rightValue === "string"
      ? leftValue.localeCompare(rightValue, "zh-CN")
      : Number(leftValue) - Number(rightValue);
  const finalResult = result === 0 ? left.rank - right.rank : result;

  return direction === "asc" ? finalResult : -finalResult;
}

function shortenDomain(domain: string): string {
  return domain.length > 24 ? `${domain.slice(0, 21)}...` : domain;
}

export function DomainStats() {
  const [selectedDays, setSelectedDays] = useState<DomainRange>(7);
  const [search, setSearch] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("total");
  const [sortDirection, setSortDirection] = useState<SortDirection>("desc");
  const [isPending, startTransition] = useTransition();
  const deferredSearch = useDeferredValue(search.trim().toLowerCase());
  const chartGradientId = useId();

  const { data, isLoading, isFetching, error } = useDomainStats(
    selectedDays,
    DOMAIN_TABLE_LIMIT,
  );

  const rankedRows: DomainRow[] = (data ?? []).map((item, index) => {
    const displayDomain = normalizeDomain(item.domain);

    return {
      ...item,
      displayDomain,
      normalizedDomain: displayDomain.toLowerCase(),
      rank: index + 1,
      total: item.upload + item.download,
    };
  });

  const filteredRows = deferredSearch
    ? rankedRows.filter((row) => row.normalizedDomain.includes(deferredSearch))
    : rankedRows;

  const sortedRows: DomainRow[] = [...filteredRows].sort((left, right) =>
    compareDomainRows(left, right, sortKey, sortDirection),
  );

  const chartRows: ChartRow[] = [...filteredRows]
    .sort((left, right) => compareDomainRows(left, right, "total", "desc"))
    .slice(0, 20)
    .map((row) => ({
      domain: row.displayDomain,
      domainLabel: shortenDomain(row.displayDomain),
      hitCount: row.hitCount,
      upload: row.upload,
      download: row.download,
      total: row.total,
    }));

  const totals = filteredRows.reduce(
    (accumulator, row) => ({
      domains: accumulator.domains + 1,
      hitCount: accumulator.hitCount + row.hitCount,
      upload: accumulator.upload + row.upload,
      download: accumulator.download + row.download,
      total: accumulator.total + row.total,
    }),
    { domains: 0, hitCount: 0, upload: 0, download: 0, total: 0 },
  );

  const selectedRangeCaption =
    selectedDays === 0 ? "今天" : selectedDays === 7 ? "近 7 天" : "近 30 天";

  const handleSort = (nextKey: SortKey) => {
    startTransition(() => {
      if (sortKey === nextKey) {
        setSortDirection((currentDirection) =>
          currentDirection === "asc" ? "desc" : "asc",
        );
        return;
      }

      setSortKey(nextKey);
      setSortDirection(nextKey === "domain" || nextKey === "rank" ? "asc" : "desc");
    });
  };

  return (
    <div className="flex flex-col gap-5">
      <section className="relative overflow-hidden rounded-[1.75rem] border border-border/70 bg-background/90 p-5 shadow-[0_24px_80px_-40px_rgba(15,23,42,0.55)]">
        <div className="pointer-events-none absolute inset-y-0 right-0 w-40 bg-linear-to-l from-primary/10 to-transparent" />
        <div className="relative flex flex-col gap-5">
          <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
            <div className="space-y-2">
              <div className="inline-flex items-center gap-2 rounded-full border border-primary/15 bg-primary/10 px-3 py-1 text-xs font-medium tracking-[0.18em] text-primary uppercase">
                <BarChart3 className="size-3.5" />
                Domain Radar
              </div>
              <div>
                <h2 className="text-2xl font-semibold tracking-tight text-foreground">
                  域名流量画像
                </h2>
                <p className="mt-1 text-sm text-muted-foreground">
                  按总流量追踪最活跃的域名，支持搜索、排序和 1 分钟自动刷新。
                </p>
              </div>
            </div>

            <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
              <div className="inline-flex rounded-full border border-border bg-muted/50 p-1">
                {RANGE_OPTIONS.map((option) => {
                  const isActive = option.days === selectedDays;

                  return (
                    <button
                      key={option.days}
                      type="button"
                      onClick={() => startTransition(() => setSelectedDays(option.days))}
                      className={cn(
                        "rounded-full px-3 py-2 text-sm transition-all",
                        isActive
                          ? "bg-primary text-primary-foreground shadow-[0_12px_32px_-18px_var(--color-primary)]"
                          : "text-muted-foreground hover:bg-background hover:text-foreground",
                      )}
                    >
                      <span className="font-medium">{option.label}</span>
                      <span className="ml-1 hidden text-xs opacity-80 sm:inline">
                        {option.caption}
                      </span>
                    </button>
                  );
                })}
              </div>

              <label className="relative block min-w-[18rem] flex-1">
                <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
                <input
                  type="search"
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder="搜索域名，例如 github.com"
                  className="h-11 w-full rounded-full border border-border bg-background/90 pl-10 pr-4 text-sm text-foreground outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20"
                />
              </label>
            </div>
          </div>

          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <SummaryCard
              label="命中域名"
              value={integerFormatter.format(totals.domains)}
              caption={`${selectedRangeCaption} 过滤结果`}
            />
            <SummaryCard
              label="访问次数"
              value={integerFormatter.format(totals.hitCount)}
              caption="按域名聚合的访问命中"
            />
            <SummaryCard
              label="上传流量"
              value={formatBytes(totals.upload)}
              caption="当前筛选范围内累计上传"
            />
            <SummaryCard
              label="总流量"
              value={formatBytes(totals.total)}
              caption={`下载 ${formatBytes(totals.download)}`}
            />
          </div>
        </div>
      </section>

      <section className="grid gap-5 2xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
        <article className="overflow-hidden rounded-[1.5rem] border border-border/70 bg-background/95 shadow-[0_24px_80px_-40px_rgba(15,23,42,0.45)]">
          <div className="flex items-center justify-between border-b border-border/70 px-5 py-4">
            <div>
              <h3 className="text-base font-semibold text-foreground">Top 20 域名流量</h3>
              <p className="mt-1 text-sm text-muted-foreground">
                柱状图按总流量排序，适合快速定位主要带宽消耗点。
              </p>
            </div>
            <StatusBadge busy={isFetching || isPending} />
          </div>

          <div className="h-[30rem] px-2 pb-4 pt-2">
            {isLoading ? (
              <ChartSkeleton />
            ) : error ? (
              <EmptyChartState
                title="域名统计加载失败"
                description={error.message}
              />
            ) : chartRows.length === 0 ? (
              <EmptyChartState
                title="没有可展示的域名数据"
                description="当前时间范围或搜索条件下没有命中记录。"
              />
            ) : (
              <ResponsiveContainer width="100%" height="100%">
                <BarChart
                  data={chartRows}
                  layout="vertical"
                  margin={{ top: 12, right: 24, bottom: 12, left: 12 }}
                  barCategoryGap="18%"
                >
                  <defs>
                    <linearGradient id={chartGradientId} x1="0" x2="1" y1="0" y2="0">
                      <stop offset="0%" stopColor="var(--color-primary-light)" stopOpacity={0.72} />
                      <stop offset="100%" stopColor="var(--color-primary)" stopOpacity={0.98} />
                    </linearGradient>
                  </defs>
                  <CartesianGrid
                    stroke="var(--color-border)"
                    strokeDasharray="4 8"
                    horizontal={false}
                  />
                  <XAxis
                    type="number"
                    axisLine={false}
                    tickLine={false}
                    tickMargin={10}
                    stroke="var(--color-muted-foreground)"
                    tickFormatter={(value) => formatBytes(Number(value))}
                  />
                  <YAxis
                    type="category"
                    dataKey="domainLabel"
                    width={176}
                    axisLine={false}
                    tickLine={false}
                    tickMargin={12}
                    interval={0}
                    stroke="var(--color-muted-foreground)"
                  />
                  <Tooltip
                    cursor={{ fill: "var(--color-accent)", opacity: 0.35 }}
                    contentStyle={{
                      borderColor: "var(--color-border)",
                      borderRadius: "18px",
                      backgroundColor: "var(--color-surface)",
                      boxShadow: "0 24px 80px -36px rgba(15, 23, 42, 0.65)",
                    }}
                    labelStyle={{ color: "var(--color-foreground)", fontWeight: 600 }}
                    itemStyle={{ color: "var(--color-foreground)" }}
                    labelFormatter={(_, payload) =>
                      String(payload[0]?.payload.domain ?? "UNKNOWN")
                    }
                    formatter={(value, _name, entry) => [
                      <TooltipValue
                        key="tooltip-value"
                        total={Number(value)}
                        upload={Number(entry.payload.upload)}
                        download={Number(entry.payload.download)}
                        hitCount={Number(entry.payload.hitCount)}
                      />,
                      "统计",
                    ]}
                  />
                  <Bar
                    dataKey="total"
                    fill={`url(#${chartGradientId})`}
                    radius={[0, 999, 999, 0]}
                  />
                </BarChart>
              </ResponsiveContainer>
            )}
          </div>
        </article>

        <aside className="grid gap-5">
          <HighlightCard
            label="榜首域名"
            value={chartRows[0]?.domain ?? "暂无数据"}
            description={
              chartRows[0]
                ? `${formatBytes(chartRows[0].total)} · ${integerFormatter.format(chartRows[0].hitCount)} 次访问`
                : "等待采样数据写入后自动展示"
            }
          />
          <HighlightCard
            label="当前刷新"
            value={isFetching || isPending ? "同步中" : "已同步"}
            description="域名统计每 60 秒自动刷新一次。"
          />
          <HighlightCard
            label="筛选结果"
            value={`${integerFormatter.format(sortedRows.length)} 条`}
            description={
              deferredSearch
                ? `搜索关键字 “${search.trim()}”`
                : "未使用域名过滤"
            }
          />
        </aside>
      </section>

      <section className="overflow-hidden rounded-[1.5rem] border border-border/70 bg-background/95 shadow-[0_24px_80px_-40px_rgba(15,23,42,0.45)]">
        <div className="flex flex-col gap-2 border-b border-border/70 px-5 py-4 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <h3 className="text-base font-semibold text-foreground">域名详情表格</h3>
            <p className="mt-1 text-sm text-muted-foreground">
              点击表头可排序，排名基于总流量的全局顺序。
            </p>
          </div>
          <StatusBadge busy={isFetching || isPending} />
        </div>

        {isLoading ? (
          <TableSkeleton />
        ) : error ? (
          <div className="px-5 py-10 text-sm text-destructive">
            加载失败: {error.message}
          </div>
        ) : sortedRows.length === 0 ? (
          <div className="px-5 py-10 text-sm text-muted-foreground">
            当前筛选条件下没有匹配的域名记录。
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead className="bg-muted/45">
                <tr className="border-b border-border/70">
                  {TABLE_COLUMNS.map((column) => (
                    <th
                      key={column.key}
                      className={cn(
                        "whitespace-nowrap px-5 py-3",
                        column.align === "right" ? "text-right" : "text-left",
                      )}
                    >
                      <button
                        type="button"
                        onClick={() => handleSort(column.key)}
                        className={cn(
                          "inline-flex items-center gap-1.5 font-medium text-muted-foreground transition hover:text-foreground",
                          column.align === "right" && "ml-auto",
                        )}
                      >
                        <span>{column.label}</span>
                        {sortKey === column.key ? (
                          sortDirection === "asc" ? (
                            <ChevronUp className="size-3.5" />
                          ) : (
                            <ChevronDown className="size-3.5" />
                          )
                        ) : (
                          <span className="text-[10px] opacity-60">↕</span>
                        )}
                      </button>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {sortedRows.map((row, index) => (
                  <tr
                    key={`${row.displayDomain}-${row.rank}`}
                    className={cn(
                      "border-b border-border/60 transition hover:bg-muted/35",
                      index % 2 === 0 ? "bg-background" : "bg-muted/10",
                    )}
                  >
                    <td className="px-5 py-3 font-medium text-foreground">{row.rank}</td>
                    <td className="max-w-[26rem] px-5 py-3">
                      <div className="truncate font-medium text-foreground">
                        {row.displayDomain}
                      </div>
                      <div className="mt-1 text-xs text-muted-foreground">
                        {integerFormatter.format(row.hitCount)} 次访问
                      </div>
                    </td>
                    <td className="px-5 py-3 text-right text-foreground">
                      {integerFormatter.format(row.hitCount)}
                    </td>
                    <td className="px-5 py-3 text-right text-warning">
                      {formatBytes(row.upload)}
                    </td>
                    <td className="px-5 py-3 text-right text-success">
                      {formatBytes(row.download)}
                    </td>
                    <td className="px-5 py-3 text-right font-medium text-foreground">
                      {formatBytes(row.total)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}

function SummaryCard({
  label,
  value,
  caption,
}: {
  label: string;
  value: string;
  caption: string;
}) {
  return (
    <div className="rounded-[1.25rem] border border-border/70 bg-background/80 p-4">
      <div className="text-xs font-medium tracking-[0.16em] text-muted-foreground uppercase">
        {label}
      </div>
      <div className="mt-3 text-2xl font-semibold tracking-tight text-foreground">
        {value}
      </div>
      <p className="mt-2 text-sm text-muted-foreground">{caption}</p>
    </div>
  );
}

function HighlightCard({
  label,
  value,
  description,
}: {
  label: string;
  value: string;
  description: string;
}) {
  return (
    <article className="rounded-[1.5rem] border border-border/70 bg-muted/20 p-5">
      <div className="text-xs font-medium tracking-[0.16em] text-muted-foreground uppercase">
        {label}
      </div>
      <div className="mt-3 truncate text-xl font-semibold tracking-tight text-foreground">
        {value}
      </div>
      <p className="mt-2 text-sm text-muted-foreground">{description}</p>
    </article>
  );
}

function StatusBadge({ busy }: { busy: boolean }) {
  return (
    <div
      className={cn(
        "inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-medium",
        busy
          ? "border-primary/20 bg-primary/10 text-primary"
          : "border-border bg-muted/45 text-muted-foreground",
      )}
    >
      <RefreshCw className={cn("size-3.5", busy && "animate-spin")} />
      {busy ? "刷新中" : "自动刷新 60s"}
    </div>
  );
}

function EmptyChartState({
  title,
  description,
}: {
  title: string;
  description: string;
}) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
      <div className="rounded-full border border-border bg-muted/50 p-3 text-muted-foreground">
        <BarChart3 className="size-5" />
      </div>
      <div>
        <p className="text-base font-medium text-foreground">{title}</p>
        <p className="mt-1 text-sm text-muted-foreground">{description}</p>
      </div>
    </div>
  );
}

function TooltipValue({
  total,
  upload,
  download,
  hitCount,
}: {
  total: number;
  upload: number;
  download: number;
  hitCount: number;
}) {
  return (
    <div className="space-y-1 text-xs">
      <div>总流量 {formatBytes(total)}</div>
      <div>下载 {formatBytes(download)}</div>
      <div>上传 {formatBytes(upload)}</div>
      <div>访问 {integerFormatter.format(hitCount)} 次</div>
    </div>
  );
}

function ChartSkeleton() {
  return (
    <div className="flex h-full items-end gap-3 px-5 py-6">
      {[48, 72, 56, 84, 40, 64].map((height, index) => (
        <div key={index} className="flex flex-1 flex-col justify-end gap-2">
          <div
            className="animate-pulse rounded-t-[1.25rem] bg-muted"
            style={{ height: `${height}%` }}
          />
          <div className="h-3 animate-pulse rounded-full bg-muted" />
        </div>
      ))}
    </div>
  );
}

function TableSkeleton() {
  return (
    <div className="space-y-3 px-5 py-5">
      {Array.from({ length: 7 }, (_, index) => (
        <div
          key={index}
          className="grid animate-pulse grid-cols-[5rem_minmax(14rem,1fr)_8rem_8rem_8rem_8rem] gap-3 rounded-[1rem] border border-border/60 bg-muted/20 px-4 py-3"
        >
          <div className="h-4 rounded-full bg-muted" />
          <div className="h-4 rounded-full bg-muted" />
          <div className="h-4 rounded-full bg-muted" />
          <div className="h-4 rounded-full bg-muted" />
          <div className="h-4 rounded-full bg-muted" />
          <div className="h-4 rounded-full bg-muted" />
        </div>
      ))}
    </div>
  );
}
