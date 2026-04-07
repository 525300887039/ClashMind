import { useState } from "react";
import { Cable, Unplug, Trash2 } from "lucide-react";
import { motion } from "framer-motion";
import { PageHeader } from "@/components/ui/page-header";
import { EmptyState } from "@/components/ui/empty-state";
import { SearchInput } from "@/components/ui/search-input";
import { ActionButton } from "@/components/ui/action-button";
import { useConnections, useCloseAllConnections } from "./hooks/use-connections";
import { ConnectionTable } from "./connection-table";

export function ConnectionsPage() {
  const { data, isLoading, error } = useConnections();
  const [search, setSearch] = useState("");
  const closeAll = useCloseAllConnections();

  const connections = data?.connections ?? [];

  if (isLoading) {
    return (
      <motion.section initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.24, ease: "easeOut" }} className="flex flex-col gap-6">
        <PageHeader
          eyebrow="Live Connections"
          eyebrowIcon={Cable}
          title="连接"
          description="实时监控活跃连接，查看流量与代理链路详情"
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
          eyebrow="Live Connections"
          eyebrowIcon={Cable}
          title="连接"
          description="实时监控活跃连接，查看流量与代理链路详情"
        />
        <div className="rounded-[1.5rem] border border-destructive/20 bg-destructive/5 p-6 text-sm text-destructive">
          加载失败: {error.message}
        </div>
      </motion.section>
    );
  }

  if (!data) return null;

  return (
    <motion.section initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.24, ease: "easeOut" }} className="flex flex-col gap-6">
      <PageHeader
        eyebrow="Live Connections"
        eyebrowIcon={Cable}
        title="连接"
        description="实时监控活跃连接，查看流量与代理链路详情"
        actions={
          <>
            <span className="inline-flex items-center rounded-full border border-primary/15 bg-primary/10 px-3 py-1 text-xs font-medium text-primary">
              {connections.length} 个连接
            </span>
            <ActionButton
              tone="destructive"
              onClick={() => closeAll.mutate()}
              disabled={connections.length === 0}
            >
              <Trash2 className="size-3.5" />
              关闭全部
            </ActionButton>
          </>
        }
      />

      <SearchInput
        value={search}
        onChange={setSearch}
        placeholder="搜索 Host..."
        className="max-w-sm"
      />

      {connections.length === 0 ? (
        <EmptyState
          icon={Unplug}
          title="暂无活跃连接"
          description="当前没有活跃的网络连接"
        />
      ) : (
        <ConnectionTable connections={connections} search={search} />
      )}
    </motion.section>
  );
}
