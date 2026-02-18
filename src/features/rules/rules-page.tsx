import { useState, useMemo } from "react";
import { Search } from "lucide-react";
import { useRules } from "./hooks/use-rules";

export function RulesPage() {
  const { data, isLoading, error } = useRules();
  const [search, setSearch] = useState("");

  const rules = data?.rules ?? [];

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    if (!q) return rules;
    return rules.filter(
      (r) =>
        r.payload.toLowerCase().includes(q) ||
        r.proxy.toLowerCase().includes(q),
    );
  }, [rules, search]);

  if (isLoading) {
    return <div className="text-sm text-muted-foreground">加载中...</div>;
  }

  if (error) {
    return (
      <div className="text-sm text-destructive">
        加载失败: {error.message}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <input
            className="h-8 w-full rounded-md border border-border bg-background pl-8 pr-3 text-sm outline-none focus:ring-1 focus:ring-ring"
            placeholder="搜索载荷或策略..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <span className="text-sm text-muted-foreground">
          共 {filtered.length} 条规则
        </span>
      </div>

      <div className="overflow-auto rounded-lg border border-border">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border bg-muted/50">
              <th className="whitespace-nowrap px-3 py-2 text-left font-medium text-muted-foreground w-16">#</th>
              <th className="whitespace-nowrap px-3 py-2 text-left font-medium text-muted-foreground">类型</th>
              <th className="whitespace-nowrap px-3 py-2 text-left font-medium text-muted-foreground">载荷</th>
              <th className="whitespace-nowrap px-3 py-2 text-left font-medium text-muted-foreground">策略</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((rule, i) => (
              <tr
                key={i}
                className="border-b border-border last:border-0 hover:bg-muted/30"
              >
                <td className="px-3 py-2 text-muted-foreground">{i + 1}</td>
                <td className="px-3 py-2">
                  <span className="rounded bg-accent px-1.5 py-0.5 text-xs">
                    {rule.type}
                  </span>
                </td>
                <td className="max-w-[300px] truncate px-3 py-2">{rule.payload}</td>
                <td className="px-3 py-2">{rule.proxy}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
