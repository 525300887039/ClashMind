import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, refetchOnWindowFocus: false },
  },
});

/** Invalidate runtime config/proxy/rule/snapshot queries after config changes. */
export async function invalidateRuntimeQueries(qc: QueryClient) {
  await Promise.all([
    qc.invalidateQueries({ queryKey: ["config"] }),
    qc.invalidateQueries({ queryKey: ["configs"] }),
    qc.invalidateQueries({ queryKey: ["proxies"] }),
    qc.invalidateQueries({ queryKey: ["rules"] }),
    qc.invalidateQueries({ queryKey: ["snapshots"] }),
  ]);
}
