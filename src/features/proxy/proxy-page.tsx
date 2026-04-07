import { Globe } from "lucide-react";
import { motion } from "framer-motion";
import { PageHeader } from "@/components/ui/page-header";
import { EmptyState } from "@/components/ui/empty-state";
import { useProxies } from "./hooks/use-proxies";
import { ProxyGroupList } from "./proxy-group-list";
import { ProxyModeSwitch } from "./proxy-mode-switch";
import { SystemProxySwitch } from "./system-proxy-switch";

export function ProxyPage() {
  const { data, isLoading, error } = useProxies();

  if (isLoading) {
    return (
      <motion.section initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.24, ease: "easeOut" }} className="flex flex-col gap-6">
        <PageHeader
          eyebrow="Proxy Gateway"
          eyebrowIcon={Globe}
          title="代理"
          description="管理代理节点、切换代理模式，实时监控连接状态"
        />
        <div className="space-y-4">
          {Array.from({ length: 3 }).map((_, i) => (
            <div
              key={i}
              className="h-32 animate-pulse rounded-[1.5rem] border border-border/70 bg-muted/20"
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
          eyebrow="Proxy Gateway"
          eyebrowIcon={Globe}
          title="代理"
          description="管理代理节点、切换代理模式，实时监控连接状态"
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
        eyebrow="Proxy Gateway"
        eyebrowIcon={Globe}
        title="代理"
        description="管理代理节点、切换代理模式，实时监控连接状态"
        actions={
          <>
            <ProxyModeSwitch />
            <SystemProxySwitch />
          </>
        }
      />
      {Object.keys(data.proxies).length === 0 ? (
        <EmptyState
          icon={Globe}
          title="暂无代理数据"
          description="请先导入或订阅代理配置文件"
        />
      ) : (
        <ProxyGroupList data={data} />
      )}
    </motion.section>
  );
}
