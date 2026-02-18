import { useConnections } from "./hooks/use-connections";
import { ConnectionTable } from "./connection-table";

export function ConnectionsPage() {
  const { data, isLoading, error } = useConnections();

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

  return <ConnectionTable connections={data.connections ?? []} />;
}
