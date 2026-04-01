const COLUMN_TEMPLATES: Record<number, string> = {
  5: "5rem minmax(14rem,1fr) 8rem 9rem 7rem",
  6: "5rem minmax(14rem,1fr) 8rem 8rem 8rem 8rem",
};

interface TableSkeletonProps {
  rows?: number;
  columns?: number;
}

export function TableSkeleton({
  rows = 6,
  columns = 5,
}: TableSkeletonProps) {
  const templateColumns =
    COLUMN_TEMPLATES[columns] ?? `repeat(${columns}, minmax(0, 1fr))`;

  return (
    <div className="space-y-3 px-5 py-5">
      {Array.from({ length: rows }, (_, rowIndex) => (
        <div
          key={rowIndex}
          className="grid animate-pulse gap-3 rounded-[1rem] border border-border/60 bg-muted/20 px-4 py-3"
          style={{ gridTemplateColumns: templateColumns }}
        >
          {Array.from({ length: columns }, (_, columnIndex) => (
            <div key={columnIndex} className="h-4 rounded-full bg-muted" />
          ))}
        </div>
      ))}
    </div>
  );
}
