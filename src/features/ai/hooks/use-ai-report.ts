import { useMutation } from "@tanstack/react-query";
import { api, type ReportResult, type ReportType } from "@/lib/tauri-api";
import { resolveAiSettings } from "../ai-settings";

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
      const settingsResult = resolveAiSettings();
      if (!settingsResult.ok) {
        throw new Error(settingsResult.message);
      }

      try {
        return await api.ai.generateReport(type, date, settingsResult.settings);
      } catch (error) {
        throw normalizeError(error);
      }
    },
  });
}
