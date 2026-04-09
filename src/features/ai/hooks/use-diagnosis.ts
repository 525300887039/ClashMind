import { useMutation, useQuery } from "@tanstack/react-query";
import {
  api,
  type AnomalyAlert,
  type DiagnosisReport,
  type DiagnosisSummary,
} from "@/lib/tauri-api";
import { normalizeError } from "@/lib/error";
import { resolveAiProviderSettings } from "./use-ai-settings";

const DIAGNOSIS_ROOT_KEY = ["diagnosis"] as const;

export const DIAGNOSIS_KEYS = {
  summary: (timeRangeMinutes: number) =>
    [...DIAGNOSIS_ROOT_KEY, "summary", timeRangeMinutes] as const,
  alerts: (timeRangeMinutes: number) =>
    [...DIAGNOSIS_ROOT_KEY, "alerts", timeRangeMinutes] as const,
  report: (timeRangeMinutes: number) =>
    [...DIAGNOSIS_ROOT_KEY, "report", timeRangeMinutes] as const,
};

export function useDiagnosisSummary(timeRangeMinutes = 30) {
  return useQuery<DiagnosisSummary, Error>({
    queryKey: DIAGNOSIS_KEYS.summary(timeRangeMinutes),
    queryFn: async () => {
      try {
        return await api.diagnosis.getSummary(timeRangeMinutes);
      } catch (error) {
        throw normalizeError(error);
      }
    },
    refetchInterval: 60_000,
    staleTime: 30_000,
    refetchOnWindowFocus: false,
  });
}

export function useAnomalyAlerts(timeRangeMinutes = 30) {
  return useQuery<AnomalyAlert[], Error>({
    queryKey: DIAGNOSIS_KEYS.alerts(timeRangeMinutes),
    queryFn: async () => {
      try {
        return await api.diagnosis.detectAnomalies(timeRangeMinutes);
      } catch (error) {
        throw normalizeError(error);
      }
    },
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
