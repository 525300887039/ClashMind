import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/tauri-api";
import { SPEED_TEST_URL, SPEED_TEST_TIMEOUT } from "@/lib/constants";

const PROXY_KEYS = { all: ["proxies"] as const };

export function useProxies() {
  return useQuery({ queryKey: PROXY_KEYS.all, queryFn: api.proxy.getAll });
}

export function useSwitchProxy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ group, name }: { group: string; name: string }) =>
      api.proxy.switch(group, name),
    onSuccess: () => qc.invalidateQueries({ queryKey: PROXY_KEYS.all }),
  });
}

export function useTestDelay() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ name }: { name: string }) =>
      api.proxy.testDelay(name, SPEED_TEST_URL, SPEED_TEST_TIMEOUT),
    onSuccess: () => qc.invalidateQueries({ queryKey: PROXY_KEYS.all }),
  });
}

export function useTestGroupDelay() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ group }: { group: string }) =>
      api.proxy.testGroupDelay(group, SPEED_TEST_URL, SPEED_TEST_TIMEOUT),
    onSuccess: () => qc.invalidateQueries({ queryKey: PROXY_KEYS.all }),
  });
}
