import { RefreshCw } from "lucide-react";
import { cn } from "@/lib/utils";

interface StatusBadgeProps {
  busy: boolean;
  idleLabel?: string;
}

export function StatusBadge({
  busy,
  idleLabel = "自动刷新 60s",
}: StatusBadgeProps) {
  return (
    <div
      className={cn(
        "inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-medium",
        busy
          ? "border-primary/20 bg-primary/10 text-primary"
          : "border-border bg-muted/45 text-muted-foreground",
      )}
    >
      <RefreshCw className={cn("size-3.5", busy && "animate-spin")} />
      {busy ? "刷新中" : idleLabel}
    </div>
  );
}
