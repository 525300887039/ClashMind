import { useState, useMemo } from "react";
import { X, Trash2, Search, ArrowUpDown } from "lucide-react";
import type { Connection } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";
import { useCloseConnection, useCloseAllConnections } from "./hooks/use-connections";

type SortKey = "host" | "network" | "type" | "chains" | "rule" | "download" | "upload" | "start";
type SortDir = "asc" | "desc";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1048576).toFixed(1)} MB`;
}

function formatDuration(start: string): string {
  const ms = Date.now() - new Date(start).getTime();
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m${s % 60}s`;
  return `${Math.floor(s / 3600)}h${Math.floor((s % 3600) / 60)}m`;
}

function getSortValue(conn: Connection, key: SortKey): string | number {
  switch (key) {
    case "host": return conn.metadata.host || conn.metadata.destinationIP;
    case "network": return conn.metadata.network;
    case "type": return conn.metadata.type;
    case "chains": return conn.chains.join(" > ");
    case "rule": return conn.rule;
    case "download": return conn.download;
    case "upload": return conn.upload;
    case "start": return new Date(conn.start).getTime();
  }
}

const COLUMNS: { key: SortKey; label: string }[] = [
  { key: "host", label: "Host" },
  { key: "network", label: "网络类型" },
  { key: "type", label: "类型" },
  { key: "chains", label: "代理链" },
  { key: "rule", label: "规则" },
  { key: "download", label: "下载↓" },
  { key: "upload", label: "上传↑" },
  { key: "start", label: "耗时" },
];

export function ConnectionTable({ connections }: { connections: Connection[] }) {
  const [search, setSearch] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("start");
  const [sortDir, setSortDir] = useState<SortDir>("desc");
  const closeConn = useCloseConnection();
  const closeAll = useCloseAllConnections();

  const handleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDir("desc");
    }
  };

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    const list = q
      ? connections.filter((c) =>
          (c.metadata.host || c.metadata.destinationIP).toLowerCase().includes(q),
        )
      : connections;

    return [...list].sort((a, b) => {
      const va = getSortValue(a, sortKey);
      const vb = getSortValue(b, sortKey);
      const cmp = va < vb ? -1 : va > vb ? 1 : 0;
      return sortDir === "asc" ? cmp : -cmp;
    });
  }, [connections, search, sortKey, sortDir]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <input
            className="h-8 w-full rounded-md border border-border bg-background pl-8 pr-3 text-sm outline-none focus:ring-1 focus:ring-ring"
            placeholder="搜索 Host..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <button
          className="flex h-8 items-center gap-1.5 rounded-md border border-border px-3 text-sm text-destructive hover:bg-destructive/10"
          onClick={() => closeAll.mutate()}
        >
          <Trash2 className="size-3.5" />
          关闭全部
        </button>
      </div>

      <div className="overflow-auto rounded-lg border border-border">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border bg-muted/50">
              {COLUMNS.map((col) => (
                <th
                  key={col.key}
                  className="cursor-pointer whitespace-nowrap px-3 py-2 text-left font-medium text-muted-foreground hover:text-foreground"
                  onClick={() => handleSort(col.key)}
                >
                  <span className="inline-flex items-center gap-1">
                    {col.label}
                    {sortKey === col.key && (
                      <ArrowUpDown className="size-3" />
                    )}
                  </span>
                </th>
              ))}
              <th className="px-3 py-2 text-left font-medium text-muted-foreground">操作</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((conn) => (
              <tr
                key={conn.id}
                className="border-b border-border last:border-0 hover:bg-muted/30"
              >
                <td className="max-w-[200px] truncate px-3 py-2">
                  {conn.metadata.host || conn.metadata.destinationIP}
                </td>
                <td className="px-3 py-2">{conn.metadata.network}</td>
                <td className="px-3 py-2">{conn.metadata.type}</td>
                <td className="max-w-[150px] truncate px-3 py-2">{conn.chains.join(" > ")}</td>
                <td className="px-3 py-2">{conn.rule}</td>
                <td className="px-3 py-2 text-success">{formatBytes(conn.download)}</td>
                <td className="px-3 py-2 text-warning">{formatBytes(conn.upload)}</td>
                <td className="px-3 py-2">{formatDuration(conn.start)}</td>
                <td className="px-3 py-2">
                  <button
                    className={cn(
                      "rounded p-1 text-muted-foreground hover:bg-destructive/10 hover:text-destructive",
                    )}
                    onClick={() => closeConn.mutate(conn.id)}
                  >
                    <X className="size-3.5" />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
