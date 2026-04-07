import { useState, useMemo } from "react";
import { Shield } from "lucide-react";
import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import {
  PageHeader,
  SectionCard,
  SearchInput,
  DataTable,
  type Column,
} from "@/components/ui";
import type { Rule } from "@/lib/tauri-api";
import { useRules } from "./hooks/use-rules";

function getRuleTypeBadgeClass(type: string): string {
  switch (type) {
    case "DOMAIN":
    case "DOMAIN-SUFFIX":
    case "DOMAIN-KEYWORD":
      return "border-blue-500/20 bg-blue-500/10 text-blue-400";
    case "GEOIP":
    case "IP-CIDR":
    case "IP-CIDR6":
      return "border-emerald-500/20 bg-emerald-500/10 text-emerald-400";
    case "MATCH":
      return "border-amber-500/20 bg-amber-500/10 text-amber-400";
    default:
      return "border-border/60 bg-muted text-muted-foreground";
  }
}

const columns: Column<Rule>[] = [
  {
    key: "index",
    label: "#",
    className: "w-16",
    render: (_row, index) => (
      <span className="text-muted-foreground">{index + 1}</span>
    ),
  },
  {
    key: "type",
    label: "类型",
    render: (row) => (
      <span
        className={cn(
          "inline-block rounded-full border px-2 py-0.5 text-xs font-medium",
          getRuleTypeBadgeClass(row.type),
        )}
      >
        {row.type}
      </span>
    ),
  },
  {
    key: "payload",
    label: "载荷",
    className: "max-w-[300px] truncate",
  },
  {
    key: "proxy",
    label: "策略",
  },
];

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
    return (
      <motion.section initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.24, ease: "easeOut" }} className="flex flex-col gap-6">
        <PageHeader
          eyebrow="Rule Engine"
          eyebrowIcon={Shield}
          title="规则"
          description="查看当前配置中的所有代理规则"
        />
        <div className="space-y-3">
          {Array.from({ length: 5 }).map((_, i) => (
            <div
              key={i}
              className="h-10 animate-pulse rounded-xl border border-border/70 bg-muted/20"
            />
          ))}
        </div>
      </motion.section>
    );
  }

  if (error) {
    return (
      <motion.section initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.24, ease: "easeOut" }} className="flex flex-col gap-6">
        <PageHeader
          eyebrow="Rule Engine"
          eyebrowIcon={Shield}
          title="规则"
          description="查看当前配置中的所有代理规则"
        />
        <div className="rounded-[1.5rem] border border-destructive/20 bg-destructive/5 p-6 text-sm text-destructive">
          加载失败: {error.message}
        </div>
      </motion.section>
    );
  }

  return (
    <motion.section initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.24, ease: "easeOut" }} className="flex flex-col gap-6">
      <PageHeader
        eyebrow="Rule Engine"
        eyebrowIcon={Shield}
        title="规则"
        description="查看当前配置中的所有代理规则"
        actions={
          <span className="inline-flex items-center rounded-full border border-primary/15 bg-primary/10 px-3 py-1 text-xs font-medium text-primary">
            {rules.length} 条规则
          </span>
        }
      />

      <SectionCard className="p-0">
        <div className="p-4 pb-0">
          <SearchInput
            value={search}
            onChange={setSearch}
            placeholder="搜索载荷或策略..."
            className="max-w-sm"
          />
        </div>
        <div className="mt-4">
          <DataTable
            columns={columns}
            data={filtered}
            getRowKey={(row) => `${row.type}:${row.payload}:${row.proxy}`}
            emptyText="没有匹配的规则"
          />
        </div>
      </SectionCard>
    </motion.section>
  );
}
