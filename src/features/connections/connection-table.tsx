import { useState, useMemo } from "react";
import { X, ChevronUp, ChevronDown } from "lucide-react";
import type { Connection } from "@/lib/tauri-api";
import { cn, formatBytes } from "@/lib/utils";
import { SectionCard } from "@/components/ui/section-card";
import { useCloseConnection } from "./hooks/use-connections";

type SortKey = "host" | "network" | "type" | "chains" | "rule" | "download" | "upload" | "start";
type SortDir = "asc" | "desc";

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

interface ConnectionTableProps {
  connections: Connection[];
  search: string;
}

export function ConnectionTable({ connections, search }: ConnectionTableProps) {
  const [sortKey, setSortKey] = useState<SortKey>("start");
  const [sortDir, setSortDir] = useState<SortDir>("desc");
  const closeConn = useCloseConnection();

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

  if (filtered.length === 0) {
    return (
      <SectionCard>
        <div className="px-5 py-10 text-center text-sm text-muted-foreground">
          {search ? "没有匹配的连接" : "暂无活跃连接"}
        </div>
      </SectionCard>
    );
  }

  return (
    <SectionCard className="p-0">
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border/70 bg-muted/45">
              {COLUMNS.map((col) => (
                <th
                  key={col.key}
                  className="cursor-pointer select-none px-5 py-3 text-left text-xs font-medium tracking-wide text-muted-foreground uppercase"
                  onClick={() => handleSort(col.key)}
                >
                  <span className="inline-flex items-center gap-1">
                    {col.label}
                    {sortKey === col.key ? (
                      sortDir === "asc" ? (
                        <ChevronUp className="size-3.5" />
                      ) : (
                        <ChevronDown className="size-3.5" />
                      )
                    ) : null}
                  </span>
                </th>
              ))}
              <th className="px-5 py-3" />
            </tr>
          </thead>
          <tbody>
            {filtered.map((conn) => (
              <tr
                key={conn.id}
                className="border-b border-border/60 transition-colors even:bg-muted/20 hover:bg-muted/35"
              >
                <td className="max-w-[200px] truncate px-5 py-3">
                  {conn.metadata.host || conn.metadata.destinationIP}
                </td>
                <td className="px-5 py-3">{conn.metadata.network}</td>
                <td className="px-5 py-3">{conn.metadata.type}</td>
                <td className="max-w-[150px] truncate px-5 py-3">{conn.chains.join(" > ")}</td>
                <td className="px-5 py-3">{conn.rule}</td>
                <td className="px-5 py-3 text-success">{formatBytes(conn.download)}</td>
                <td className="px-5 py-3 text-warning">{formatBytes(conn.upload)}</td>
                <td className="px-5 py-3">{formatDuration(conn.start)}</td>
                <td className="px-5 py-3">
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
    </SectionCard>
  );
}
