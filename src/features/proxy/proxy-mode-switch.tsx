import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Globe, Route, Zap } from "lucide-react";
import { api } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";

const MODES = [
  { value: "rule", label: "规则", icon: Route },
  { value: "global", label: "全局", icon: Globe },
  { value: "direct", label: "直连", icon: Zap },
] as const;

type ProxyMode = (typeof MODES)[number]["value"];

function useProxyMode() {
  return useQuery({
    queryKey: ["configs"],
    queryFn: api.config.get,
    select: (data) => (data.mode as string)?.toLowerCase() as ProxyMode,
  });
}

function usePatchMode() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (mode: ProxyMode) => api.config.patch({ mode }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["configs"] }),
  });
}

export function ProxyModeSwitch() {
  const { data: currentMode } = useProxyMode();
  const patchMode = usePatchMode();

  return (
    <div className="flex items-center gap-1 rounded-full border border-border/70 bg-muted/30 p-1">
      {MODES.map(({ value, label, icon: Icon }) => (
        <button
          key={value}
          onClick={() => patchMode.mutate(value)}
          className={cn(
            "flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm transition-colors",
            currentMode === value
              ? "bg-primary text-primary-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          <Icon className="size-3.5" />
          {label}
        </button>
      ))}
    </div>
  );
}
