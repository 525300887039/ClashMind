import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/tauri-api";

const ONE_MINUTE = 60_000;
const STATS_ROOT_KEY = ["stats"] as const;

export const STATS_KEYS = {
  all: STATS_ROOT_KEY,
  domains: (days: number, limit: number) =>
    [...STATS_ROOT_KEY, "domains", days, limit] as const,
  trafficHourly: (start: string, end: string) =>
    [...STATS_ROOT_KEY, "traffic-hourly", start, end] as const,
  trafficDaily: (start: string, end: string) =>
    [...STATS_ROOT_KEY, "traffic-daily", start, end] as const,
  overview: (days: number) =>
    [...STATS_ROOT_KEY, "overview", days] as const,
} as const;

export function useDomainStats(days = 7, limit = 50) {
  return useQuery({
    queryKey: STATS_KEYS.domains(days, limit),
    queryFn: () => api.stats.domains(days, limit),
    staleTime: ONE_MINUTE,
    refetchInterval: ONE_MINUTE,
    placeholderData: (previousData) => previousData,
  });
}

export function useStatsOverview(days = 7) {
  return useQuery({
    queryKey: STATS_KEYS.overview(days),
    queryFn: () => api.stats.overview(days),
    staleTime: ONE_MINUTE,
    refetchInterval: ONE_MINUTE,
    placeholderData: (previousData) => previousData,
  });
}
