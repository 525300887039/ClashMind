import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/tauri-api";

const CONNECTIONS_KEYS = { all: ["connections"] as const };

export function useConnections() {
  return useQuery({
    queryKey: CONNECTIONS_KEYS.all,
    queryFn: api.connection.getAll,
    refetchInterval: 1000,
  });
}

export function useCloseConnection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.connection.close(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: CONNECTIONS_KEYS.all }),
  });
}

export function useCloseAllConnections() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.connection.closeAll(),
    onSuccess: () => qc.invalidateQueries({ queryKey: CONNECTIONS_KEYS.all }),
  });
}
