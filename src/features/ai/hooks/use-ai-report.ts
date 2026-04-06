import { useMutation } from "@tanstack/react-query";
import { api, type ReportResult, type ReportType } from "@/lib/tauri-api";
import { resolveAiProviderSettings } from "./use-ai-settings";

export interface GenerateReportInput {
  type: ReportType;
  date?: string;
}

function normalizeError(error: unknown) {
  return error instanceof Error ? error : new Error(String(error));
}

export function useAiReport() {
  return useMutation<ReportResult, Error, GenerateReportInput>({
    mutationFn: async ({ type, date }) => {
      const settings = await resolveAiProviderSettings();

      try {
        return await api.ai.generateReport(type, date, settings);
      } catch (error) {
        throw normalizeError(error);
      }
    },
  });
}
