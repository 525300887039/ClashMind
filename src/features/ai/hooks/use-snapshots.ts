import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "@/lib/tauri-api";
import { normalizeErrorMessage } from "@/lib/error";
import { invalidateRuntimeQueries } from "@/lib/query-client";

const SNAPSHOT_KEYS = {
  all: ["snapshots"] as const,
  list: (limit: number) => ["snapshots", limit] as const,
};

export function useSnapshots(limit = 20) {
  return useQuery({
    queryKey: SNAPSHOT_KEYS.list(limit),
    queryFn: () => api.ai.listSnapshots(limit),
  });
}

export function useCreateSnapshot() {
  const queryClient = useQueryClient();

  return useMutation<number, Error, string | undefined>({
    mutationFn: (description) => api.ai.createSnapshot(description),
    onSuccess: async () => {
      await invalidateRuntimeQueries(queryClient);
      toast.success("配置快照已创建");
    },
    onError: (error) => {
      toast.error(`创建快照失败: ${normalizeErrorMessage(error)}`);
    },
  });
}

export function useRestoreSnapshot() {
  const queryClient = useQueryClient();

  return useMutation<void, Error, number>({
    mutationFn: (id) => api.ai.restoreSnapshot(id),
    onSuccess: async () => {
      await invalidateRuntimeQueries(queryClient);
      toast.success("已恢复到所选快照并完成热重载");
    },
    onError: (error) => {
      toast.error(`恢复快照失败: ${normalizeErrorMessage(error)}`);
    },
  });
}
