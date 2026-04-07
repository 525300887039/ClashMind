import { Zap } from "lucide-react";
import { cn } from "@/lib/utils";

interface DelayTestButtonProps {
  onClick: () => void;
  loading: boolean;
}

export function DelayTestButton({ onClick, loading }: DelayTestButtonProps) {
  return (
    <button
      type="button"
      disabled={loading}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      className={cn(
        "rounded-full p-1.5 text-muted-foreground hover:bg-primary/10 hover:text-primary",
        loading && "animate-pulse",
      )}
    >
      <Zap className="size-4" />
    </button>
  );
}
