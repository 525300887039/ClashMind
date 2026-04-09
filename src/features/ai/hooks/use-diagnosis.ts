import { useMutation, useQuery } from "@tanstack/react-query";
import {
  api,
  type AnomalyAlert,
  type DiagnosisOverview,
  type DiagnosisReport,
  type DiagnosisSummary,
} from "@/lib/tauri-api";
import { normalizeError } from "@/lib/error";
import { resolveAiProviderSettings } from "./use-ai-settings";

const DIAGNOSIS_ROOT_KEY = ["diagnosis"] as const;

export const DIAGNOSIS_KEYS = {
  overview: (timeRangeMinutes: number) =>
    [...DIAGNOSIS_ROOT_KEY, "overview", timeRangeMinutes] as const,
  report: (timeRangeMinutes: number) =>
    [...DIAGNOSIS_ROOT_KEY, "report", timeRangeMinutes] as const,
};

function diagnosisOverviewQueryFn(timeRangeMinutes: number) {
  return async () => {
    try {
      return await api.diagnosis.getOverview(timeRangeMinutes);
    } catch (error) {
      throw normalizeError(error);
    }
  };
}

export function useDiagnosisOverview(timeRangeMinutes = 30) {
  return useQuery<DiagnosisOverview, Error>({
    queryKey: DIAGNOSIS_KEYS.overview(timeRangeMinutes),
    queryFn: diagnosisOverviewQueryFn(timeRangeMinutes),
    refetchInterval: 60_000,
    staleTime: 30_000,
    refetchOnWindowFocus: false,
  });
}

export function useDiagnosisSummary(timeRangeMinutes = 30) {
  return useQuery<DiagnosisOverview, Error, DiagnosisSummary>({
    queryKey: DIAGNOSIS_KEYS.overview(timeRangeMinutes),
    queryFn: diagnosisOverviewQueryFn(timeRangeMinutes),
    select: (overview) => overview.summary,
    refetchInterval: 60_000,
    staleTime: 30_000,
    refetchOnWindowFocus: false,
  });
}

export function useAnomalyAlerts(timeRangeMinutes = 30) {
  return useQuery<DiagnosisOverview, Error, AnomalyAlert[]>({
    queryKey: DIAGNOSIS_KEYS.overview(timeRangeMinutes),
    queryFn: diagnosisOverviewQueryFn(timeRangeMinutes),
    select: (overview) => overview.alerts,
    refetchInterval: 60_000,
    staleTime: 30_000,
    refetchOnWindowFocus: false,
  });
}

export function useAiDiagnosis() {
  return useMutation<DiagnosisReport, Error, number | undefined>({
    mutationFn: async (timeRangeMinutes) => {
      const settings = await resolveAiProviderSettings();

      try {
        return await api.ai.generateDiagnosis(timeRangeMinutes, settings);
      } catch (error) {
        throw normalizeError(error);
      }
    },
  });
}

export function useQuickDiagnosis() {
  const diagnosisMutation = useAiDiagnosis();

  return {
    runDiagnosis: async (timeRangeMinutes = 30) =>
      diagnosisMutation.mutateAsync(timeRangeMinutes),
    isLoading: diagnosisMutation.isPending,
    data: diagnosisMutation.data ?? null,
    error: diagnosisMutation.error ?? null,
    reset: diagnosisMutation.reset,
  };
}
