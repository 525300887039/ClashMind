import { cn } from "@/lib/utils";

export interface SectionCardProps {
  children: React.ReactNode;
  className?: string;
  glow?: boolean;
  hoverable?: boolean;
}

export function SectionCard({ children, className, glow, hoverable }: SectionCardProps) {
  return (
    <div
      className={cn(
        "relative overflow-hidden rounded-[1.5rem] border border-border/70 bg-background/95 p-5 shadow-md",
        hoverable && "transition-all duration-200 hover:-translate-y-0.5 hover:shadow-lg hover:border-primary/20",
        className,
      )}
    >
      {glow ? (
        <div className="pointer-events-none absolute -right-8 top-0 size-28 rounded-full bg-primary/10 blur-3xl" />
      ) : null}
      <div className="relative h-full">{children}</div>
    </div>
  );
}
