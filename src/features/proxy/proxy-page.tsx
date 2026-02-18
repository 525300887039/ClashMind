import { useProxies } from "./hooks/use-proxies";
import { ProxyGroupList } from "./proxy-group-list";

export function ProxyPage() {
  const { data, isLoading, error } = useProxies();

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

  if (!data) return null;

  return <ProxyGroupList data={data} />;
}
