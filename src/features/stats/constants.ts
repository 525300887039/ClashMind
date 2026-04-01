export type StatsRange = 0 | 7 | 30;

interface RangeOption {
  days: StatsRange;
  label: string;
  caption: string;
}

export const RANGE_OPTIONS: readonly RangeOption[] = [
  { days: 0, label: "1天", caption: "今天" },
  { days: 7, label: "7天", caption: "近 7 天" },
  { days: 30, label: "30天", caption: "近 30 天" },
];

export const integerFormatter = new Intl.NumberFormat("zh-CN");

export function getRangeCaption(days: StatsRange): string {
  return RANGE_OPTIONS.find((option) => option.days === days)?.caption ?? "";
}
