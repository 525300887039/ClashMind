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
        "flex flex-col gap-1 rounded-md border px-3 py-2 text-left text-sm transition-colors",
        selected
          ? "border-primary bg-accent ring-1 ring-primary"
          : "border-border bg-muted/30 hover:bg-muted/60",
      )}
    >
      <span className="truncate font-medium">{name}</span>
      <div className="flex items-center justify-between">
        <span className="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
          {type}
        </span>
        <span className={cn("text-xs font-mono", getDelayColor(delay))}>
          {getDelayText(delay)}
        </span>
      </div>
    </button>
  );
}
