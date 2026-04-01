import { cn } from "@/lib/utils";

interface SummaryCardProps {
  label: string;
  value: string | number;
  caption: string;
  truncate?: boolean;
  className?: string;
}

export function SummaryCard({
  label,
  value,
  caption,
  truncate = false,
  className,
}: SummaryCardProps) {
  return (
    <div
      className={cn(
        "rounded-[1.25rem] border border-border/70 p-4",
        className ?? "bg-background/82",
      )}
    >
      <div className="text-xs font-medium tracking-[0.16em] text-muted-foreground uppercase">
        {label}
      </div>
      <div
        className={cn(
          "mt-3 text-2xl font-semibold tracking-tight text-foreground",
          truncate && "truncate",
        )}
      >
        {value}
      </div>
      <p className="mt-2 text-sm text-muted-foreground">{caption}</p>
    </div>
  );
}
