import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

interface HighlightCardProps {
  label: string;
  value: string;
  description: string;
  icon?: ReactNode;
  truncateValue?: boolean;
}

export function HighlightCard({
  label,
  value,
  description,
  icon,
  truncateValue = false,
}: HighlightCardProps) {
  return (
    <article className="rounded-[1.5rem] border border-border/70 bg-muted/20 p-5">
      <div className="flex items-center justify-between gap-3">
        <div className="text-xs font-medium tracking-[0.16em] text-muted-foreground uppercase">
          {label}
        </div>
        {icon ? (
          <div className="rounded-full border border-border/70 bg-background/75 p-2 text-muted-foreground">
            {icon}
          </div>
        ) : null}
      </div>
      <div
        className={cn(
          "mt-3 text-xl font-semibold tracking-tight text-foreground",
          truncateValue && "truncate",
        )}
      >
        {value}
      </div>
      <p className="mt-2 text-sm text-muted-foreground">{description}</p>
    </article>
  );
}
