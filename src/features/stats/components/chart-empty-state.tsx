import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

type ChartEmptyStateVariant = "light" | "dark";

interface ChartEmptyStateProps {
  title: string;
  description: string;
  icon?: ReactNode;
  variant?: ChartEmptyStateVariant;
}

export function ChartEmptyState({
  title,
  description,
  icon,
  variant = "light",
}: ChartEmptyStateProps) {
  const isDark = variant === "dark";

  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
      {icon ? (
        <div
          className={cn(
            "rounded-full p-3",
            isDark
              ? "border border-white/10 bg-white/5 text-slate-200"
              : "border border-border bg-muted/50 text-muted-foreground",
          )}
        >
          {icon}
        </div>
      ) : null}
      <div>
        <p className={cn("text-base font-medium", isDark ? "text-white" : "text-foreground")}>
          {title}
        </p>
        <p
          className={cn(
            "mt-1 text-sm",
            isDark ? "text-slate-300/80" : "text-muted-foreground",
          )}
        >
          {description}
        </p>
      </div>
    </div>
  );
}
