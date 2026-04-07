import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, refetchOnWindowFocus: false },
  },
});

/** Invalidate runtime config/proxy/rule queries after config changes. */
export async function invalidateRuntimeQueries(
  qc: QueryClient,
  options?: { includeSnapshots?: boolean },
) {
  await Promise.all([
    qc.invalidateQueries({ queryKey: ["config"] }),
    qc.invalidateQueries({ queryKey: ["configs"] }),
    qc.invalidateQueries({ queryKey: ["proxies"] }),
    qc.invalidateQueries({ queryKey: ["rules"] }),
    ...(options?.includeSnapshots
      ? [qc.invalidateQueries({ queryKey: ["snapshots"] })]
      : []),
  ]);
}
