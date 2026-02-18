import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/tauri-api";

const RULES_KEYS = { all: ["rules"] as const };

export function useRules() {
  return useQuery({ queryKey: RULES_KEYS.all, queryFn: api.rule.getAll });
}
