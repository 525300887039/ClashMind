import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "@/lib/tauri-api";
import { normalizeErrorMessage } from "@/lib/error";
import { invalidateRuntimeQueries } from "@/lib/query-client";
import { useAiStore } from "@/stores/ai-store";

export function useConfigApply(toolCallId: string, confirmationBatchId?: string) {
  const queryClient = useQueryClient();
  const setToolCallStatus = useAiStore((state) => state.setToolCallStatus);
  const setConfigConfirmationBatchStatus = useAiStore(
    (state) => state.setConfigConfirmationBatchStatus,
  );
  const getConfigApplyPayload = useAiStore((state) => state.getConfigApplyPayload);
  const clearConfigApplyPayload = useAiStore((state) => state.clearConfigApplyPayload);

  const applyMutation = useMutation<void, Error, void>({
    mutationFn: async () => {
      if (confirmationBatchId === undefined) {
        throw new Error("缺少配置确认批次 ID，无法应用本次变更");
      }

      const payload = getConfigApplyPayload(confirmationBatchId);
      if (payload === null) {
        throw new Error("未找到待应用的配置内容，请重新生成 Diff");
      }

      await api.ai.applyConfigChange(payload);
    },
    onSuccess: async () => {
      if (confirmationBatchId !== undefined) {
        setConfigConfirmationBatchStatus(confirmationBatchId, "applied");
        clearConfigApplyPayload(confirmationBatchId);
      } else {
        setToolCallStatus(toolCallId, "applied");
      }
      await invalidateRuntimeQueries(queryClient, { includeSnapshots: true });
      toast.success("配置已写入并完成热重载");
    },
    onError: (error) => {
      toast.error(`应用配置失败: ${normalizeErrorMessage(error)}`);
    },
  });

  const rejectMutation = useMutation<void, Error, void>({
    mutationFn: async () => {
      await api.ai.rejectConfigChange();
    },
    onSuccess: () => {
      if (confirmationBatchId !== undefined) {
        setConfigConfirmationBatchStatus(confirmationBatchId, "rejected");
        clearConfigApplyPayload(confirmationBatchId);
      } else {
        setToolCallStatus(toolCallId, "rejected");
      }
      toast.success("已丢弃本次配置变更");
    },
    onError: (error) => {
      toast.error(`取消配置变更失败: ${normalizeErrorMessage(error)}`);
    },
  });

  const apply = () => {
    rejectMutation.reset();
    applyMutation.mutate();
  };

  const reject = () => {
    applyMutation.reset();
    rejectMutation.mutate();
  };

  return {
    apply,
    reject,
    isApplying: applyMutation.isPending,
    isRejecting: rejectMutation.isPending,
    error: applyMutation.error?.message ?? rejectMutation.error?.message ?? null,
  };
}
