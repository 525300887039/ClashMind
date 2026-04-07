import { RefreshCw } from "lucide-react";
import { cn } from "@/lib/utils";

export interface StatusBadgeProps {
  busy: boolean;
  busyText?: string;
  readyText?: string;
}

export function StatusBadge({
  busy,
  busyText = "刷新中",
  readyText = "自动刷新 60s",
}: StatusBadgeProps) {
  return (
    <div
      className={cn(
        "inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-medium",
        busy
          ? "border-primary/20 bg-primary/10 text-primary"
          : "border-border bg-muted/40 text-muted-foreground",
      )}
    >
      <RefreshCw className={cn("size-3.5", busy && "animate-spin")} />
      {busy ? busyText : readyText}
    </div>
  );
}
