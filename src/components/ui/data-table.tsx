import { useState, useMemo } from "react";
import { ChevronUp, ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";

export interface Column<T> {
  key: string;
  label: string;
  align?: "left" | "right";
  sortable?: boolean;
  render?: (row: T, index: number) => React.ReactNode;
  className?: string;
}

export interface DataTableProps<T> {
  columns: Column<T>[];
  data: T[];
  getRowKey: (row: T) => string;
  defaultSort?: { key: string; direction: "asc" | "desc" };
  rowActions?: (row: T) => React.ReactNode;
  emptyText?: string;
}

export function DataTable<T>({
  columns,
  data,
  getRowKey,
  defaultSort,
  rowActions,
  emptyText = "暂无数据",
}: DataTableProps<T>) {
  const [sortKey, setSortKey] = useState(defaultSort?.key ?? "");
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">(
    defaultSort?.direction ?? "desc",
  );

  function handleSort(key: string) {
    if (sortKey === key) {
      setSortDirection((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDirection("desc");
    }
  }

  const sorted = useMemo(() => {
    if (!sortKey) return data;
    return [...data].sort((a, b) => {
      const av = (a as Record<string, unknown>)[sortKey];
      const bv = (b as Record<string, unknown>)[sortKey];
      let cmp = 0;
      if (typeof av === "number" && typeof bv === "number") {
        cmp = av - bv;
      } else {
        cmp = String(av ?? "").localeCompare(String(bv ?? ""));
      }
      return sortDirection === "asc" ? cmp : -cmp;
    });
  }, [data, sortKey, sortDirection]);

  if (data.length === 0) {
    return (
      <div className="px-5 py-10 text-center text-sm text-muted-foreground">
        {emptyText}
      </div>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border/70 bg-muted/45">
            {columns.map((col) => (
              <th
                key={col.key}
                className={cn(
                  "px-5 py-3 text-xs font-medium tracking-wide text-muted-foreground uppercase",
                  col.align === "right" ? "text-right" : "text-left",
                  col.sortable && "cursor-pointer select-none",
                  col.className,
                )}
                onClick={col.sortable ? () => handleSort(col.key) : undefined}
              >
                <span className="inline-flex items-center gap-1">
                  {col.label}
                  {col.sortable && sortKey === col.key ? (
                    sortDirection === "asc" ? (
                      <ChevronUp className="size-3.5" />
                    ) : (
                      <ChevronDown className="size-3.5" />
                    )
                  ) : null}
                </span>
              </th>
            ))}
            {rowActions ? <th className="px-5 py-3" /> : null}
          </tr>
        </thead>
        <tbody>
          {sorted.map((row, i) => (
            <tr
              key={getRowKey(row)}
              className="border-b border-border/60 even:bg-muted/20 hover:bg-muted/35 transition-colors"
            >
              {columns.map((col) => (
                <td
                  key={col.key}
                  className={cn(
                    "px-5 py-3",
                    col.align === "right" && "text-right",
                    col.className,
                  )}
                >
                  {col.render
                    ? col.render(row, i)
                    : String(
                        (row as Record<string, unknown>)[col.key] ?? "",
                      )}
                </td>
              ))}
              {rowActions ? (
                <td className="px-5 py-3 text-right">{rowActions(row)}</td>
              ) : null}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
