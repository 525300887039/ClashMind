import { cn } from "@/lib/utils";

interface ProxyNodeCardProps {
  name: string;
  type: string;
  delay: number;
  selected: boolean;
  onSelect: () => void;
}

function getDelayColor(delay: number) {
  if (delay <= 0) return "text-muted-foreground";
  if (delay < 200) return "text-success";
  if (delay < 500) return "text-warning";
  return "text-destructive";
}

function getDelayText(delay: number) {
  if (delay <= 0) return "--";
  return `${delay}ms`;
}

export function ProxyNodeCard({
  name,
  type,
  delay,
  selected,
  onSelect,
}: ProxyNodeCardProps) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        "flex flex-col gap-1 rounded-xl border px-3.5 py-2.5 text-left text-sm transition-colors",
        selected
          ? "border-primary/40 bg-primary/8 ring-1 ring-primary/20 shadow-[0_8px_24px_-12px_var(--color-primary)]"
          : "border-border/70 bg-background/80 shadow-sm hover:border-primary/20 hover:bg-muted/30",
      )}
    >
      <span className="truncate font-medium">{name}</span>
      <div className="flex items-center justify-between">
        <span className="rounded-full border border-border/60 bg-muted/40 px-1.5 py-0.5 text-xs text-muted-foreground">
          {type}
        </span>
        <span className={cn("text-xs font-mono", getDelayColor(delay))}>
          {getDelayText(delay)}
        </span>
      </div>
    </button>
  );
}
