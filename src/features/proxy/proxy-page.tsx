import { useProxies } from "./hooks/use-proxies";
import { ProxyGroupList } from "./proxy-group-list";
import { ProxyModeSwitch } from "./proxy-mode-switch";
import { SystemProxySwitch } from "./system-proxy-switch";

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

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-3">
        <ProxyModeSwitch />
        <SystemProxySwitch />
      </div>
      <ProxyGroupList data={data} />
    </div>
  );
}
