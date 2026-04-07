import { cn } from "@/lib/utils";

export interface FieldShellProps {
  label: string;
  hint?: string;
  children: React.ReactNode;
}

export function FieldShell({ label, hint, children }: FieldShellProps) {
  return (
    <label className="block rounded-[1.45rem] border border-border/70 bg-background/68 p-4">
      <div className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
        {label}
      </div>
      {hint ? (
        <p className="mt-2 text-sm leading-6 text-muted-foreground">{hint}</p>
      ) : null}
      <div className={cn(hint ? "mt-4" : "mt-3")}>{children}</div>
    </label>
  );
}
