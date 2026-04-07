import { cn } from "@/lib/utils";

export interface ActionButtonProps {
  tone?: "primary" | "secondary" | "ghost" | "destructive";
  disabled?: boolean;
  onClick?: () => void;
  children: React.ReactNode;
  className?: string;
}

export function ActionButton({
  tone = "primary",
  disabled,
  onClick,
  children,
  className,
}: ActionButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-full border px-4 py-2.5 text-sm font-medium transition-all active:translate-y-0 active:scale-[0.98]",
        tone === "primary" &&
          "border-primary/20 bg-primary text-primary-foreground shadow-[0_18px_42px_-24px_var(--color-primary)] hover:translate-y-[-1px] hover:bg-primary/92",
        tone === "secondary" &&
          "border-border/70 bg-background/80 text-foreground hover:border-primary/20 hover:bg-primary/5",
        tone === "ghost" &&
          "border-border/70 bg-transparent text-muted-foreground hover:border-primary/20 hover:text-foreground",
        tone === "destructive" &&
          "border-destructive/20 bg-destructive/5 text-destructive hover:bg-destructive/10",
        "disabled:cursor-not-allowed disabled:opacity-55 disabled:hover:translate-y-0",
        className,
      )}
    >
      {children}
    </button>
  );
}
