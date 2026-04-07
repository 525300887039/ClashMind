export interface EmptyStateProps {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  description: string;
  action?: React.ReactNode;
}

export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
}: EmptyStateProps) {
  return (
    <div className="relative overflow-hidden rounded-[1.6rem] border border-dashed border-border/70 bg-muted/15 px-6 py-10 text-center flex flex-col items-center gap-4">
      <div className="pointer-events-none absolute -top-6 left-1/2 size-24 -translate-x-1/2 rounded-full bg-primary/10 blur-3xl" />
      <div className="relative rounded-[1.25rem] border border-primary/20 bg-primary/10 p-3 text-primary">
        <Icon className="size-6" />
      </div>
      <div>
        <p className="text-base font-medium text-foreground">{title}</p>
        <p className="mt-1 max-w-sm text-sm text-muted-foreground">
          {description}
        </p>
      </div>
      {action ? <div className="mt-3">{action}</div> : null}
    </div>
  );
}
