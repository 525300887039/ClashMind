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
        "rounded-md p-1 text-muted-foreground hover:bg-muted hover:text-foreground",
        loading && "animate-pulse",
      )}
    >
      <Zap className="size-4" />
    </button>
  );
}
