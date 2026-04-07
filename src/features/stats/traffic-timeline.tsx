import { useState, useTransition } from "react";
import {
  addDays,
  addHours,
  addMinutes,
  format,
  parseISO,
  subDays,
  subHours,
} from "date-fns";
import { Activity, Clock3, Waypoints } from "lucide-react";
import {
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { TooltipContentProps } from "recharts";
import type { TrafficPoint } from "@/lib/tauri-api";
import { cn, formatBytes } from "@/lib/utils";
import { ChartEmptyState } from "./components/chart-empty-state";
import { HighlightCard } from "./components/highlight-card";
import { StatusBadge } from "@/components/ui/status-badge";
import { SummaryCard } from "./components/summary-card";
import { integerFormatter } from "./constants";
import { useTrafficDaily, useTrafficHourly } from "./hooks/use-stats";

type TrafficGranularity = "hourly" | "daily";
type HourlyRange = "24h" | "48h" | "168h";
type DailyRange = "7d" | "30d" | "90d";

interface RangeOption<Id extends string> {
  id: Id;
  label: string;
  caption: string;
  amount: number;
}

interface TrafficWindow {
  start: string;
  end: string;
}

interface TrafficChartRow {
  time: string;
  upload: number;
  download: number;
  connCount: number;
  total: number;
}

const UPLOAD_COLOR = "#60a5fa";
const DOWNLOAD_COLOR = "#34d399";
const CONNECTION_COLOR = "#fbbf24";

const HOURLY_RANGE_OPTIONS: RangeOption<HourlyRange>[] = [
  { id: "24h", label: "24h", caption: "最近 24 小时", amount: 24 },
  { id: "48h", label: "48h", caption: "最近 48 小时", amount: 48 },
  { id: "168h", label: "7天", caption: "最近 7 天", amount: 168 },
];

const DAILY_RANGE_OPTIONS: RangeOption<DailyRange>[] = [
  { id: "7d", label: "7天", caption: "最近 7 天", amount: 7 },
  { id: "30d", label: "30天", caption: "最近 30 天", amount: 30 },
  { id: "90d", label: "90天", caption: "最近 90 天", amount: 90 },
];

function startOfUtcHour(date: Date): Date {
  return new Date(
    Date.UTC(
      date.getUTCFullYear(),
      date.getUTCMonth(),
      date.getUTCDate(),
      date.getUTCHours(),
      0,
      0,
      0,
    ),
  );
}

function startOfUtcDay(date: Date): Date {
  return new Date(
    Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate(), 0, 0, 0, 0),
  );
}

function toUtcIso(date: Date): string {
  return date.toISOString().replace(".000Z", "Z");
}

function getHourlyOption(range: HourlyRange): RangeOption<HourlyRange> {
  const matchedOption = HOURLY_RANGE_OPTIONS.find((option) => option.id === range);
  if (matchedOption) {
    return matchedOption;
  }

  const fallbackOption = HOURLY_RANGE_OPTIONS[0];
  if (!fallbackOption) {
    throw new Error("Missing hourly range options");
  }

  return fallbackOption;
}

function getDailyOption(range: DailyRange): RangeOption<DailyRange> {
  const matchedOption = DAILY_RANGE_OPTIONS.find((option) => option.id === range);
  if (matchedOption) {
    return matchedOption;
  }

  const fallbackOption = DAILY_RANGE_OPTIONS[0];
  if (!fallbackOption) {
    throw new Error("Missing daily range options");
  }

  return fallbackOption;
}

function getHourlyWindow(range: HourlyRange): TrafficWindow {
  const option = getHourlyOption(range);
  const end = addHours(startOfUtcHour(new Date()), 1);
  const start = subHours(end, option.amount);

  return { start: toUtcIso(start), end: toUtcIso(end) };
}

function getDailyWindow(range: DailyRange): TrafficWindow {
  const option = getDailyOption(range);
  const end = addDays(startOfUtcDay(new Date()), 1);
  const start = subDays(end, option.amount);

  return { start: toUtcIso(start), end: toUtcIso(end) };
}

function normalizeBucketTime(time: string): string {
  return toUtcIso(parseISO(time));
}

function toChartRow(point: TrafficPoint): TrafficChartRow {
  const normalizedTime = normalizeBucketTime(point.time);

  return {
    time: normalizedTime,
    upload: point.upload,
    download: point.download,
    connCount: point.connCount,
    total: point.upload + point.download,
  };
}

function buildChartRows(
  points: readonly TrafficPoint[],
  granularity: TrafficGranularity,
  start: string,
  end: string,
): TrafficChartRow[] {
  const pointMap = new Map<string, TrafficPoint>(
    points.map((point) => [normalizeBucketTime(point.time), point]),
  );
  const rows: TrafficChartRow[] = [];
  const endDate = parseISO(end);

  for (let cursor = parseISO(start); cursor < endDate; ) {
    const bucketTime = toUtcIso(cursor);
    const point = pointMap.get(bucketTime);

    rows.push({
      time: bucketTime,
      upload: point?.upload ?? 0,
      download: point?.download ?? 0,
      connCount: point?.connCount ?? 0,
      total: (point?.upload ?? 0) + (point?.download ?? 0),
    });

    cursor = granularity === "hourly" ? addHours(cursor, 1) : addDays(cursor, 1);
  }

  return rows;
}

function getLastBucketTime(end: string, granularity: TrafficGranularity): string {
  const endDate = parseISO(end);

  return toUtcIso(granularity === "hourly" ? subHours(endDate, 1) : subDays(endDate, 1));
}

function getUtcDisplayDate(time: string): Date {
  const date = parseISO(time);
  return addMinutes(date, date.getTimezoneOffset());
}

function formatAxisTime(time: string, granularity: TrafficGranularity): string {
  return format(
    getUtcDisplayDate(time),
    granularity === "hourly" ? "MM-dd HH:mm" : "MM-dd",
  );
}

function formatDetailedTime(time: string, granularity: TrafficGranularity): string {
  return format(
    getUtcDisplayDate(time),
    granularity === "hourly" ? "yyyy-MM-dd HH:mm 'UTC'" : "yyyy-MM-dd 'UTC'",
  );
}

function formatPreciseBytes(bytes: number): string {
  return `${formatBytes(bytes)} · ${integerFormatter.format(bytes)} B`;
}

function renderLegendLabel(value: string | number) {
  return <span className="text-sm text-foreground">{value}</span>;
}

export function TrafficTimeline() {
  const [granularity, setGranularity] = useState<TrafficGranularity>("hourly");
  const [hourlyRange, setHourlyRange] = useState<HourlyRange>("24h");
  const [dailyRange, setDailyRange] = useState<DailyRange>("7d");
  const [isPending, startTransition] = useTransition();

  const hourlyWindow = getHourlyWindow(hourlyRange);
  const dailyWindow = getDailyWindow(dailyRange);

  const hourlyQuery = useTrafficHourly(
    hourlyWindow.start,
    hourlyWindow.end,
    granularity === "hourly",
  );
  const dailyQuery = useTrafficDaily(
    dailyWindow.start,
    dailyWindow.end,
    granularity === "daily",
  );

  const activeQuery = granularity === "hourly" ? hourlyQuery : dailyQuery;
  const activeWindow = granularity === "hourly" ? hourlyWindow : dailyWindow;
  const activeRangeOption =
    granularity === "hourly" ? getHourlyOption(hourlyRange) : getDailyOption(dailyRange);
  const hasResolvedWindow = activeQuery.data !== undefined;
  const isWindowPending =
    !hasResolvedWindow && (activeQuery.isPending || activeQuery.isFetching);
  const actualRows = (activeQuery.data ?? []).map(toChartRow);
  const chartRows = hasResolvedWindow
    ? buildChartRows(activeQuery.data ?? [], granularity, activeWindow.start, activeWindow.end)
    : [];

  const totals = actualRows.reduce(
    (accumulator, row) => ({
      upload: accumulator.upload + row.upload,
      download: accumulator.download + row.download,
      peakConnections: Math.max(accumulator.peakConnections, row.connCount),
      peakTraffic: Math.max(accumulator.peakTraffic, row.total),
    }),
    { upload: 0, download: 0, peakConnections: 0, peakTraffic: 0 },
  );

  const peakConnectionRow = actualRows.reduce<TrafficChartRow | null>(
    (currentPeak, row) =>
      currentPeak === null || row.connCount > currentPeak.connCount ? row : currentPeak,
    null,
  );
  const peakTrafficRow = actualRows.reduce<TrafficChartRow | null>(
    (currentPeak, row) =>
      currentPeak === null || row.total > currentPeak.total ? row : currentPeak,
    null,
  );

  const refreshLabel = granularity === "hourly" ? "自动刷新 60s" : "自动刷新 5m";
  const hasData = actualRows.length > 0;
  const windowStartLabel = formatDetailedTime(activeWindow.start, granularity);
  const windowEndLabel = formatDetailedTime(
    getLastBucketTime(activeWindow.end, granularity),
    granularity,
  );

  return (
    <div className="flex flex-col gap-5">
      <section className="relative overflow-hidden rounded-[1.75rem] border border-border/70 bg-background/90 p-5 shadow-[0_24px_80px_-40px_rgba(15,23,42,0.55)]">
        <div className="pointer-events-none absolute inset-y-0 right-0 w-44 bg-linear-to-l from-primary/12 to-transparent" />
        <div className="pointer-events-none absolute -left-10 top-8 h-24 w-24 rounded-full bg-primary/10 blur-3xl" />
        <div className="relative flex flex-col gap-5">
          <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
            <div className="space-y-2">
              <div className="inline-flex items-center gap-2 rounded-full border border-primary/15 bg-primary/10 px-3 py-1 text-xs font-medium tracking-[0.18em] text-primary uppercase">
                <Activity className="size-3.5" />
                Signal Trace
              </div>
              <div>
                <h2 className="text-2xl font-semibold tracking-tight text-foreground">
                  流量趋势
                </h2>
                <p className="mt-1 max-w-2xl text-sm text-muted-foreground">
                  按小时或天追踪上传、下载与连接数的时间走势。切换粒度或范围后会自动重新查询当前窗口。
                </p>
              </div>
            </div>

            <div className="flex flex-col gap-3 sm:min-w-[22rem]">
              <ControlGroup
                label="粒度"
                options={[
                  { id: "hourly", label: "小时", caption: "短期波动" },
                  { id: "daily", label: "天", caption: "长期趋势" },
                ]}
                value={granularity}
                onChange={(nextGranularity) =>
                  startTransition(() => setGranularity(nextGranularity as TrafficGranularity))
                }
              />
              <ControlGroup
                label="范围"
                options={
                  granularity === "hourly"
                    ? HOURLY_RANGE_OPTIONS
                    : DAILY_RANGE_OPTIONS
                }
                value={granularity === "hourly" ? hourlyRange : dailyRange}
                onChange={(nextRange) =>
                  startTransition(() => {
                    if (granularity === "hourly") {
                      setHourlyRange(nextRange as HourlyRange);
                      return;
                    }

                    setDailyRange(nextRange as DailyRange);
                  })
                }
              />
            </div>
          </div>

          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <SummaryCard
              label="总上传"
              value={isWindowPending ? "加载中" : formatBytes(totals.upload)}
              caption={`${activeRangeOption.caption} 累计上行`}
            />
            <SummaryCard
              label="总下载"
              value={isWindowPending ? "加载中" : formatBytes(totals.download)}
              caption={`${activeRangeOption.caption} 累计下行`}
            />
            <SummaryCard
              label="峰值连接"
              value={
                isWindowPending
                  ? "加载中"
                  : integerFormatter.format(totals.peakConnections)
              }
              caption="右侧 Y 轴显示连接数"
            />
            <SummaryCard
              label="采样桶"
              value={isWindowPending ? "加载中" : integerFormatter.format(chartRows.length)}
              caption={`${granularity === "hourly" ? "小时级" : "天级"} 补齐显示`}
            />
          </div>
        </div>
      </section>

      <section className="grid gap-5 2xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
        <article className="overflow-hidden rounded-[1.5rem] border border-border/70 bg-background/95 shadow-[0_24px_80px_-40px_rgba(15,23,42,0.45)]">
          <div className="flex flex-col gap-3 border-b border-border/70 px-5 py-4 sm:flex-row sm:items-end sm:justify-between">
            <div>
              <h3 className="text-base font-semibold text-foreground">上传 / 下载 / 连接数</h3>
              <p className="mt-1 text-sm text-muted-foreground">
                左轴为流量，右轴为连接数，悬浮可查看精确字节值。
              </p>
            </div>
            <StatusBadge busy={activeQuery.isFetching || isPending} readyText={refreshLabel} />
          </div>

          <div className="h-[32rem] px-2 pb-4 pt-2">
            {activeQuery.isLoading ? (
              <ChartSkeleton />
            ) : activeQuery.error ? (
              <ChartEmptyState
                title="流量趋势加载失败"
                description={activeQuery.error.message}
                icon={<Clock3 className="size-5" />}
              />
            ) : !hasData ? (
              <ChartEmptyState
                title="当前时间窗口没有趋势数据"
                description="等待采样任务写入后，这里会自动显示上传、下载和连接数变化。"
                icon={<Clock3 className="size-5" />}
              />
            ) : (
              <ResponsiveContainer width="100%" height="100%">
                <LineChart
                  data={chartRows}
                  margin={{ top: 16, right: 6, bottom: 10, left: 0 }}
                >
                  <CartesianGrid
                    stroke="var(--color-border)"
                    strokeDasharray="4 8"
                    vertical={false}
                  />
                  <XAxis
                    dataKey="time"
                    axisLine={false}
                    tickLine={false}
                    tickMargin={10}
                    minTickGap={granularity === "hourly" ? 32 : 24}
                    stroke="var(--color-muted-foreground)"
                    tickFormatter={(value) => formatAxisTime(String(value), granularity)}
                  />
                  <YAxis
                    yAxisId="traffic"
                    width={92}
                    axisLine={false}
                    tickLine={false}
                    tickMargin={10}
                    stroke="var(--color-muted-foreground)"
                    tickFormatter={(value) => formatBytes(Number(value))}
                  />
                  <YAxis
                    yAxisId="connections"
                    orientation="right"
                    width={64}
                    axisLine={false}
                    tickLine={false}
                    tickMargin={10}
                    allowDecimals={false}
                    stroke="var(--color-muted-foreground)"
                    tickFormatter={(value) => integerFormatter.format(Number(value))}
                  />
                  <Tooltip
                    cursor={{
                      stroke: "var(--color-border)",
                      strokeDasharray: "4 8",
                      strokeWidth: 1,
                    }}
                    content={(props) => (
                      <TrafficTooltip {...props} granularity={granularity} />
                    )}
                  />
                  <Legend
                    verticalAlign="top"
                    align="right"
                    height={36}
                    iconType="plainline"
                    wrapperStyle={{ color: "var(--color-foreground)", paddingBottom: "12px" }}
                    formatter={renderLegendLabel}
                  />
                  <Line
                    yAxisId="traffic"
                    type="monotone"
                    dataKey="upload"
                    name="上传"
                    stroke={UPLOAD_COLOR}
                    strokeWidth={2.75}
                    dot={false}
                    activeDot={{ r: 4, fill: UPLOAD_COLOR, stroke: "var(--color-surface)" }}
                  />
                  <Line
                    yAxisId="traffic"
                    type="monotone"
                    dataKey="download"
                    name="下载"
                    stroke={DOWNLOAD_COLOR}
                    strokeWidth={2.75}
                    dot={false}
                    activeDot={{ r: 4, fill: DOWNLOAD_COLOR, stroke: "var(--color-surface)" }}
                  />
                  <Line
                    yAxisId="connections"
                    type="monotone"
                    dataKey="connCount"
                    name="连接数"
                    stroke={CONNECTION_COLOR}
                    strokeWidth={2.4}
                    strokeDasharray="7 6"
                    dot={false}
                    activeDot={{ r: 4, fill: CONNECTION_COLOR, stroke: "var(--color-surface)" }}
                  />
                </LineChart>
              </ResponsiveContainer>
            )}
          </div>
        </article>

        <aside className="grid gap-5">
          <HighlightCard
            label="峰值吞吐"
            value={isWindowPending ? "加载中" : formatBytes(totals.peakTraffic)}
            description={
              peakTrafficRow
                ? `${formatDetailedTime(peakTrafficRow.time, granularity)} 出现窗口内最高总流量。`
                : "等待流量样本写入后展示峰值时段。"
            }
          />
          <HighlightCard
            label="峰值连接"
            value={
              isWindowPending
                ? "加载中"
                : integerFormatter.format(totals.peakConnections)
            }
            description={
              peakConnectionRow
                ? `${formatDetailedTime(peakConnectionRow.time, granularity)} 达到最大连接数。`
                : "当前还没有可用的连接聚合数据。"
            }
          />
          <HighlightCard
            label="查询窗口"
            value={activeRangeOption.caption}
            description={`${windowStartLabel} 至 ${windowEndLabel}`}
          />
          <HighlightCard
            label="坐标说明"
            value={granularity === "hourly" ? "小时视角" : "天级视角"}
            description="蓝线为上传，绿线为下载，橙色虚线为连接数。"
            icon={<Waypoints className="size-4" />}
          />
        </aside>
      </section>
    </div>
  );
}

function ControlGroup<Id extends string>({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: ReadonlyArray<{ id: Id; label: string; caption: string }>;
  value: Id;
  onChange: (value: Id) => void;
}) {
  return (
    <div className="rounded-[1.25rem] border border-border/70 bg-muted/25 p-3">
      <div className="text-xs font-medium tracking-[0.16em] text-muted-foreground uppercase">
        {label}
      </div>
      <div className="mt-3 flex flex-wrap gap-2">
        {options.map((option) => {
          const isActive = option.id === value;

          return (
            <button
              key={option.id}
              type="button"
              onClick={() => onChange(option.id)}
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
  );
}

function ChartSkeleton() {
  return (
    <div className="flex h-full flex-col gap-6 px-4 py-5">
      <div className="flex items-center justify-between">
        <div className="h-4 w-28 animate-pulse rounded-full bg-muted" />
        <div className="h-4 w-40 animate-pulse rounded-full bg-muted" />
      </div>
      <div className="flex flex-1 items-end gap-3">
        {[44, 68, 52, 76, 40, 60, 72].map((height, index) => (
          <div key={index} className="flex flex-1 flex-col justify-end gap-2">
            <div
              className="animate-pulse rounded-[1rem] bg-muted/85"
              style={{ height: `${height}%` }}
            />
            <div className="h-3 animate-pulse rounded-full bg-muted/65" />
          </div>
        ))}
      </div>
    </div>
  );
}

interface TrafficTooltipProps extends TooltipContentProps {
  granularity: TrafficGranularity;
}

function TrafficTooltip({
  active,
  payload,
  granularity,
}: TrafficTooltipProps) {
  if (!active || !payload?.length) {
    return null;
  }

  const row = payload[0]?.payload as TrafficChartRow | undefined;
  if (!row) {
    return null;
  }

  return (
    <div className="min-w-[15rem] rounded-[1.25rem] border border-border/70 bg-background/95 p-4 shadow-[0_24px_80px_-36px_rgba(15,23,42,0.65)]">
      <div className="text-xs font-medium tracking-[0.18em] text-muted-foreground uppercase">
        {granularity === "hourly" ? "小时桶" : "日桶"}
      </div>
      <div className="mt-2 text-sm font-semibold text-foreground">
        {formatDetailedTime(row.time, granularity)}
      </div>
      <div className="mt-4 space-y-2 text-sm">
        <TooltipRow label="上传" value={formatPreciseBytes(row.upload)} color={UPLOAD_COLOR} />
        <TooltipRow label="下载" value={formatPreciseBytes(row.download)} color={DOWNLOAD_COLOR} />
        <TooltipRow
          label="连接数"
          value={`${integerFormatter.format(row.connCount)} 个`}
          color={CONNECTION_COLOR}
        />
      </div>
    </div>
  );
}

function TooltipRow({
  label,
  value,
  color,
}: {
  label: string;
  value: string;
  color: string;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="inline-flex items-center gap-2 text-muted-foreground">
        <span className="size-2.5 rounded-full" style={{ backgroundColor: color }} />
        <span>{label}</span>
      </div>
      <span className="text-right font-medium text-foreground">{value}</span>
    </div>
  );
}
