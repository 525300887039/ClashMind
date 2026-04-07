import { useCallback, useMemo, useRef, useState, useTransition } from "react";
import {
  Globe2,
  MapPinned,
  RefreshCw,
  Radar,
  SatelliteDish,
} from "lucide-react";
import { scaleLinear } from "d3-scale";
import { ComposableMap, Geographies, Geography } from "react-simple-maps";
import type { GeoStat } from "@/lib/tauri-api";
import { cn, formatBytes } from "@/lib/utils";
import { ChartEmptyState } from "./components/chart-empty-state";
import { HighlightCard } from "./components/highlight-card";
import { RangeSelector } from "./components/range-selector";
import { StatusBadge } from "@/components/ui/status-badge";
import { SummaryCard } from "./components/summary-card";
import { TableSkeleton } from "./components/table-skeleton";
import { getRangeCaption, integerFormatter, type StatsRange } from "./constants";
import { useGeoStats } from "./hooks/use-stats";
import worldGeographyUrl from "@/assets/geo/world-countries-110m.json?url";

interface HoveredCountry {
  country: string;
  traffic: number;
  connCount: number;
}

const DEFAULT_TOOLTIP_OFFSET = 24;
const TOOLTIP_CURSOR_OFFSET = 12;
const TOOLTIP_MARGIN = 16;
const TOOLTIP_WIDTH = 196;
const TOOLTIP_HEIGHT = 96;

const COUNTRY_NAME_ALIASES: Record<string, readonly string[]> = {
  "bosnia and herzegovina": ["bosnia and herz."],
  "central african republic": ["central african rep."],
  "democratic republic of the congo": ["dem. rep. congo"],
  "dominican republic": ["dominican rep."],
  "equatorial guinea": ["eq. guinea"],
  "north macedonia": ["macedonia"],
  "solomon islands": ["solomon is."],
  "south sudan": ["s. sudan"],
  "united states": ["united states of america"],
};

function normalizeCountryName(value: string): string {
  return value.trim().toLowerCase().replace(/[().,']/g, "").replace(/\s+/g, " ");
}

function getCountryLookupKeys(country: string): string[] {
  const normalizedCountry = normalizeCountryName(country);
  const aliases = COUNTRY_NAME_ALIASES[normalizedCountry] ?? [];

  return [normalizedCountry, ...aliases];
}

function buildCountryLookup(stats: readonly GeoStat[]): Map<string, GeoStat> {
  const lookup = new Map<string, GeoStat>();

  for (const stat of stats) {
    for (const key of getCountryLookupKeys(stat.country)) {
      if (!lookup.has(key)) {
        lookup.set(key, stat);
      }
    }
  }

  return lookup;
}

function getCountryName(geography: { properties?: unknown }): string {
  if (typeof geography.properties !== "object" || geography.properties === null) {
    return "Unknown";
  }

  const name = (geography.properties as Record<string, unknown>).name;
  return typeof name === "string" && name.length > 0 ? name : "Unknown";
}

function getCountryFlag(countryCode: string): string {
  const normalizedCode = countryCode.trim().toUpperCase();
  if (!/^[A-Z]{2}$/.test(normalizedCode)) {
    return "··";
  }

  return String.fromCodePoint(
    ...Array.from(normalizedCode, (character) => 0x1f1a5 + character.charCodeAt(0)),
  );
}

function getTooltipPosition(event: React.MouseEvent<SVGPathElement>) {
  const svgRect = event.currentTarget.ownerSVGElement?.getBoundingClientRect();

  if (!svgRect) {
    return {
      left: DEFAULT_TOOLTIP_OFFSET,
      top: DEFAULT_TOOLTIP_OFFSET,
    };
  }

  const rawX = event.clientX - svgRect.left + TOOLTIP_CURSOR_OFFSET;
  const rawY = event.clientY - svgRect.top + TOOLTIP_CURSOR_OFFSET;

  return {
    left: Math.min(
      Math.max(rawX, TOOLTIP_MARGIN),
      Math.max(svgRect.width - TOOLTIP_WIDTH - TOOLTIP_MARGIN, TOOLTIP_MARGIN),
    ),
    top: Math.min(
      Math.max(rawY, TOOLTIP_MARGIN),
      Math.max(svgRect.height - TOOLTIP_HEIGHT - TOOLTIP_MARGIN, TOOLTIP_MARGIN),
    ),
  };
}

function formatTrafficShare(totalTraffic: number, traffic: number): string {
  if (totalTraffic <= 0) {
    return "0%";
  }

  return `${((traffic / totalTraffic) * 100).toFixed(1)}%`;
}

function createTrafficScale(maxTraffic: number) {
  const safeMax = maxTraffic > 0 ? maxTraffic : 1;

  return scaleLinear<string>()
    .domain([0, safeMax * 0.35, safeMax])
    .range(["#0f2741", "#57c6ff", "#8b5cf6"]);
}

export function GeoMap() {
  const [selectedDays, setSelectedDays] = useState<StatsRange>(7);
  const [hoveredCountry, setHoveredCountry] = useState<HoveredCountry | null>(null);
  const [isPending, startTransition] = useTransition();
  const tooltipRef = useRef<HTMLDivElement>(null);

  const { data, isLoading, isFetching, error } = useGeoStats(selectedDays);

  const stats = useMemo(
    () =>
      [...(data ?? [])].sort(
        (left, right) =>
          right.totalTraffic - left.totalTraffic ||
          right.connCount - left.connCount ||
          left.country.localeCompare(right.country, "zh-CN"),
      ),
    [data],
  );

  const totalTraffic = useMemo(
    () => stats.reduce((sum, stat) => sum + stat.totalTraffic, 0),
    [stats],
  );
  const totalConnections = useMemo(
    () => stats.reduce((sum, stat) => sum + stat.connCount, 0),
    [stats],
  );
  const maxTraffic = stats[0]?.totalTraffic ?? 0;
  const trafficScale = useMemo(() => createTrafficScale(maxTraffic), [maxTraffic]);
  const countryLookup = useMemo(() => buildCountryLookup(stats), [stats]);
  const leadingCountry = stats[0];
  const selectedRangeCaption = getRangeCaption(selectedDays);
  const updateTooltipPosition = useCallback(
    (event: React.MouseEvent<SVGPathElement>) => {
      const tooltip = tooltipRef.current;
      if (!tooltip) {
        return;
      }

      const { left, top } = getTooltipPosition(event);
      tooltip.style.left = `${left}px`;
      tooltip.style.top = `${top}px`;
    },
    [],
  );
  const handleMouseEnter = useCallback(
    (
      event: React.MouseEvent<SVGPathElement>,
      country: string,
      traffic: number,
      connCount: number,
    ) => {
      setHoveredCountry((previous) => {
        if (
          previous?.country === country &&
          previous?.traffic === traffic &&
          previous?.connCount === connCount
        ) {
          return previous;
        }

        return { country, traffic, connCount };
      });
      updateTooltipPosition(event);
    },
    [updateTooltipPosition],
  );
  const handleMouseMove = useCallback(
    (event: React.MouseEvent<SVGPathElement>) => {
      updateTooltipPosition(event);
    },
    [updateTooltipPosition],
  );
  const handleMouseLeave = useCallback(() => {
    setHoveredCountry(null);
  }, []);

  return (
    <div className="flex flex-col gap-5">
      <section className="relative overflow-hidden rounded-[1.75rem] border border-border/70 bg-background/90 p-5 shadow-[0_24px_80px_-40px_rgba(15,23,42,0.55)]">
        <div className="pointer-events-none absolute inset-y-0 right-0 w-44 bg-linear-to-l from-primary/12 to-transparent" />
        <div className="pointer-events-none absolute -left-10 top-10 size-32 rounded-full bg-primary/12 blur-3xl" />
        <div className="relative flex flex-col gap-5">
          <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
            <div className="space-y-2">
              <div className="inline-flex items-center gap-2 rounded-full border border-primary/15 bg-primary/10 px-3 py-1 text-xs font-medium tracking-[0.18em] text-primary uppercase">
                <Radar className="size-3.5" />
                Atlas Pulse
              </div>
              <div>
                <h2 className="text-2xl font-semibold tracking-tight text-foreground">
                  地理分布热力图
                </h2>
                <p className="mt-1 max-w-2xl text-sm text-muted-foreground">
                  使用本地 Country.mmdb 优先解析目标 IP，再按国家聚合流量与连接数，地图和排行会保持同一窗口同步刷新。
                </p>
              </div>
            </div>

            <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
              <RangeSelector
                selectedDays={selectedDays}
                onSelect={(days) => startTransition(() => setSelectedDays(days))}
                isPending={isPending}
              />
              <StatusBadge busy={isFetching || isPending} />
            </div>
          </div>

          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <SummaryCard
              label="覆盖国家"
              value={integerFormatter.format(stats.length)}
              caption={`${selectedRangeCaption} 已命中的国家/地区`}
              truncate
            />
            <SummaryCard
              label="连接总数"
              value={integerFormatter.format(totalConnections)}
              caption="按国家聚合后的连接命中次数"
              truncate
            />
            <SummaryCard
              label="总流量"
              value={formatBytes(totalTraffic)}
              caption="所有已识别国家的累计流量"
              truncate
            />
            <SummaryCard
              label="榜首国家"
              value={leadingCountry?.country ?? "暂无数据"}
              caption={
                leadingCountry
                  ? `${formatBytes(leadingCountry.totalTraffic)} · ${integerFormatter.format(leadingCountry.connCount)} 次连接`
                  : "等待采样数据进入地理解析"
              }
              truncate
            />
          </div>
        </div>
      </section>

      <section className="grid gap-5 2xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
        <article className="overflow-hidden rounded-[1.5rem] border border-slate-900/90 bg-[#081623] shadow-[0_30px_110px_-52px_rgba(2,8,23,0.92)]">
          <div className="flex flex-col gap-3 border-b border-white/10 px-5 py-4 sm:flex-row sm:items-end sm:justify-between">
            <div>
              <h3 className="text-base font-semibold text-white">World Traffic Atlas</h3>
              <p className="mt-1 text-sm text-slate-300/80">
                颜色越深表示国家总流量越高，悬浮可查看连接数和流量。
              </p>
            </div>
            <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-medium text-slate-200">
              <MapPinned className="size-3.5" />
              本地地图资产
            </div>
          </div>

          <div className="relative h-[33rem] overflow-hidden">
            <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(88,151,255,0.18),transparent_42%),radial-gradient(circle_at_78%_80%,rgba(217,70,239,0.14),transparent_34%)]" />
            <div className="pointer-events-none absolute inset-x-6 top-6 h-px bg-linear-to-r from-transparent via-white/20 to-transparent" />
            <div className="pointer-events-none absolute inset-y-10 left-6 w-px bg-linear-to-b from-transparent via-white/10 to-transparent" />

            {isLoading ? (
              <MapSkeleton />
            ) : error ? (
              <ChartEmptyState
                title="地理统计加载失败"
                description={error.message}
                icon={<MapPinned className="size-5" />}
                variant="dark"
              />
            ) : (
              <div className="relative h-full">
                <ComposableMap
                  className="h-full w-full"
                  projection="geoEqualEarth"
                  projectionConfig={{ scale: 168 }}
                >
                  <Geographies geography={worldGeographyUrl}>
                    {({ geographies }) =>
                      geographies.map((geography) => {
                        const geographyName = getCountryName(geography);
                        const stat = countryLookup.get(
                          normalizeCountryName(geographyName),
                        );
                        const country = stat?.country ?? geographyName;
                        const traffic = stat?.totalTraffic ?? 0;
                        const connCount = stat?.connCount ?? 0;
                        const fill = stat
                          ? trafficScale(stat.totalTraffic)
                          : "rgba(255,255,255,0.08)";

                        return (
                          <Geography
                            key={geography.rsmKey}
                            geography={geography}
                            fill={fill}
                            stroke="rgba(148,163,184,0.28)"
                            strokeWidth={0.45}
                            onMouseEnter={(event) =>
                              handleMouseEnter(event, country, traffic, connCount)
                            }
                            onMouseMove={handleMouseMove}
                            onMouseLeave={handleMouseLeave}
                            style={{
                              default: { outline: "none" },
                              hover: {
                                outline: "none",
                                fill: stat
                                  ? trafficScale(stat.totalTraffic)
                                  : "rgba(255,255,255,0.18)",
                              },
                              pressed: { outline: "none" },
                            }}
                          />
                        );
                      })
                    }
                  </Geographies>
                </ComposableMap>

                <div
                  ref={tooltipRef}
                  aria-hidden={hoveredCountry === null}
                  className={cn(
                    "pointer-events-none absolute top-6 left-6 min-w-[12rem] rounded-[1rem] border border-white/10 bg-slate-950/92 px-4 py-3 text-xs shadow-[0_24px_80px_-36px_rgba(2,6,23,0.95)] backdrop-blur",
                    hoveredCountry ? "visible" : "invisible",
                  )}
                >
                  <div className="text-[11px] font-medium tracking-[0.18em] text-slate-400 uppercase">
                    Geo Snapshot
                  </div>
                  <div className="mt-2 text-sm font-semibold text-white">
                    {hoveredCountry?.country ?? ""}
                  </div>
                  <div className="mt-3 space-y-1.5 text-slate-200">
                    <div>总流量 {formatBytes(hoveredCountry?.traffic ?? 0)}</div>
                    <div>连接数 {integerFormatter.format(hoveredCountry?.connCount ?? 0)}</div>
                  </div>
                </div>

                {hoveredCountry === null ? (
                  <div className="pointer-events-none absolute bottom-6 right-6 max-w-[13rem] rounded-[1rem] border border-white/10 bg-slate-950/75 px-4 py-3 text-xs text-slate-300/85 backdrop-blur">
                    将鼠标移到地图上查看国家流量与连接数。
                  </div>
                ) : null}

                {!stats.length ? (
                  <div className="pointer-events-none absolute inset-0 flex items-center justify-center px-6">
                    <div className="rounded-[1.25rem] border border-white/10 bg-slate-950/72 px-6 py-5 text-center backdrop-blur">
                      <div className="inline-flex rounded-full border border-white/10 bg-white/5 p-3 text-slate-200">
                        <Globe2 className="size-5" />
                      </div>
                      <p className="mt-3 text-base font-medium text-white">
                        当前窗口没有可展示的地理数据
                      </p>
                      <p className="mt-1 text-sm text-slate-300/75">
                        等待连接样本写入并完成 GeoIP 解析后，这里会自动点亮热区。
                      </p>
                    </div>
                  </div>
                ) : null}
              </div>
            )}
          </div>

          <div className="flex flex-wrap items-center justify-between gap-4 border-t border-white/10 px-5 py-4 text-xs text-slate-300/85">
            <div className="inline-flex items-center gap-2">
              <span className="font-medium text-slate-200">流量图例</span>
              <span>低</span>
              <span className="h-2.5 w-28 rounded-full bg-linear-to-r from-[#0f2741] via-[#57c6ff] to-[#8b5cf6]" />
              <span>高</span>
            </div>
            <div>地图底图已本地打包，离线环境下也可渲染。</div>
          </div>
        </article>

        <aside className="grid gap-5">
          <HighlightCard
            label="最活跃国家"
            value={leadingCountry?.country ?? "暂无数据"}
            description={
              leadingCountry
                ? `${formatBytes(leadingCountry.totalTraffic)} · 占比 ${formatTrafficShare(totalTraffic, leadingCountry.totalTraffic)}`
                : "等待 GeoIP 命中后展示主要流量目的地。"
            }
            icon={<SatelliteDish className="size-4" />}
          />
          <HighlightCard
            label="识别质量"
            value={`${integerFormatter.format(stats.length)} 个国家`}
            description="仅统计已成功解析到国家信息的目标 IP。未命中记录不会污染热力图。"
            icon={<Globe2 className="size-4" />}
          />
          <HighlightCard
            label="刷新状态"
            value={isFetching || isPending ? "同步中" : "已同步"}
            description="地理统计每 60 秒自动刷新一次，本地 MMDB 缓存命中会显著缩短查询时间。"
            icon={<RefreshCw className={cn("size-4", (isFetching || isPending) && "animate-spin")} />}
          />
        </aside>
      </section>

      <section className="overflow-hidden rounded-[1.5rem] border border-border/70 bg-background/95 shadow-[0_24px_80px_-40px_rgba(15,23,42,0.45)]">
        <div className="flex flex-col gap-2 border-b border-border/70 px-5 py-4 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <h3 className="text-base font-semibold text-foreground">国家 / 地区排行</h3>
            <p className="mt-1 text-sm text-muted-foreground">
              按总流量降序排列，同时展示连接次数和流量占比。
            </p>
          </div>
          <div className="text-sm text-muted-foreground">
            当前窗口 {selectedRangeCaption}
          </div>
        </div>

        {isLoading ? (
          <TableSkeleton rows={6} columns={5} />
        ) : error ? (
          <div className="px-5 py-10 text-sm text-destructive">
            加载失败: {error.message}
          </div>
        ) : !stats.length ? (
          <div className="px-5 py-10 text-sm text-muted-foreground">
            当前窗口没有已解析的国家数据。
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead className="bg-muted/45">
                <tr className="border-b border-border/70">
                  <th className="px-5 py-3 text-left font-medium text-muted-foreground">排名</th>
                  <th className="px-5 py-3 text-left font-medium text-muted-foreground">国家</th>
                  <th className="px-5 py-3 text-right font-medium text-muted-foreground">连接数</th>
                  <th className="px-5 py-3 text-right font-medium text-muted-foreground">总流量</th>
                  <th className="px-5 py-3 text-right font-medium text-muted-foreground">占比</th>
                </tr>
              </thead>
              <tbody>
                {stats.map((stat, index) => (
                  <tr
                    key={`${stat.countryCode}-${stat.country}`}
                    className={cn(
                      "border-b border-border/60 transition hover:bg-muted/35",
                      index % 2 === 0 ? "bg-background" : "bg-muted/20",
                    )}
                  >
                    <td className="px-5 py-3 font-medium text-foreground">{index + 1}</td>
                    <td className="px-5 py-3">
                      <div className="flex items-center gap-3">
                        <span className="inline-flex size-8 items-center justify-center rounded-full border border-border/70 bg-muted/20 text-base">
                          {getCountryFlag(stat.countryCode)}
                        </span>
                        <div>
                          <div className="font-medium text-foreground">{stat.country}</div>
                          <div className="mt-1 text-xs text-muted-foreground">
                            {stat.countryCode}
                          </div>
                        </div>
                      </div>
                    </td>
                    <td className="px-5 py-3 text-right text-foreground">
                      {integerFormatter.format(stat.connCount)}
                    </td>
                    <td className="px-5 py-3 text-right font-medium text-foreground">
                      {formatBytes(stat.totalTraffic)}
                    </td>
                    <td className="px-5 py-3 text-right text-primary">
                      {formatTrafficShare(totalTraffic, stat.totalTraffic)}
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

function MapSkeleton() {
  return (
    <div className="flex h-full items-center justify-center px-6">
      <div className="grid w-full max-w-3xl gap-4">
        <div className="h-5 w-40 animate-pulse rounded-full bg-white/10" />
        <div className="h-[21rem] animate-pulse rounded-[2rem] border border-white/10 bg-white/5" />
        <div className="flex items-center gap-3">
          <div className="h-3 w-8 animate-pulse rounded-full bg-white/10" />
          <div className="h-3 flex-1 animate-pulse rounded-full bg-white/10" />
          <div className="h-3 w-8 animate-pulse rounded-full bg-white/10" />
        </div>
      </div>
    </div>
  );
}
