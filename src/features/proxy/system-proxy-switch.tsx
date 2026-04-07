import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Shield, ShieldOff } from "lucide-react";
import { api } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/app-store";

const SYSPROXY_KEY = ["system-proxy"] as const;

function useSystemProxy() {
  return useQuery({
    queryKey: SYSPROXY_KEY,
    queryFn: api.system.getProxy,
  });
}

function useToggleSystemProxy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ enable, port }: { enable: boolean; port: number }) =>
      api.system.setProxy(enable, port),
    onSuccess: () => qc.invalidateQueries({ queryKey: SYSPROXY_KEY }),
  });
}

export function SystemProxySwitch() {
  const { data } = useSystemProxy();
  const toggle = useToggleSystemProxy();
  const httpPort = useAppStore((s) => s.httpPort);

  const enabled = data?.enable ?? false;
  const port = httpPort || 7890;

  return (
    <button
      onClick={() => toggle.mutate({ enable: !enabled, port })}
      className={cn(
        "ml-auto flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-sm font-medium transition-colors",
        enabled
          ? "border-primary/30 bg-primary/10 text-primary shadow-[0_4px_16px_-8px_var(--color-primary)]"
          : "border-border text-muted-foreground hover:text-foreground",
      )}
    >
      <span
        className={cn(
          "size-2 rounded-full",
          enabled ? "bg-primary/80" : "bg-muted-foreground/40",
        )}
      />
      {enabled ? <Shield className="size-3.5" /> : <ShieldOff className="size-3.5" />}
      {enabled ? "系统代理已开启" : "系统代理已关闭"}
    </button>
  );
}
