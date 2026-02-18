import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/tauri-api";

const CONFIG_KEYS = { read: (path: string) => ["config", path] as const };

export function useReadConfig(path: string) {
  return useQuery({
    queryKey: CONFIG_KEYS.read(path),
    queryFn: () => api.config.read(path),
    enabled: !!path,
  });
}

export function useWriteConfig() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ path, content }: { path: string; content: string }) =>
      api.config.write(path, content),
    onSuccess: (_data, vars) =>
      qc.invalidateQueries({ queryKey: CONFIG_KEYS.read(vars.path) }),
  });
}

export function useReloadConfig() {
  return useMutation({
    mutationFn: (mihomoUrl: string) => api.config.reload(mihomoUrl),
  });
}
