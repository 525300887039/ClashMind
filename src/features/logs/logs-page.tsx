import { useState, useMemo, useRef, useEffect } from "react";
import { Search, Trash2, Pause, Play } from "lucide-react";
import { cn } from "@/lib/utils";
import { useLogs, type LogEntry } from "./hooks/use-logs";

const LEVELS = ["all", "info", "warning", "error", "debug"] as const;

const LEVEL_COLORS: Record<string, string> = {
  info: "text-primary",
  warning: "text-warning",
  error: "text-destructive",
  debug: "text-muted-foreground",
};

function formatTime(ts: number) {
  const d = new Date(ts);
  return d.toLocaleTimeString("zh-CN", { hour12: false });
}

export function LogsPage() {
  const { logs, paused, clear, togglePause } = useLogs();
  const [level, setLevel] = useState<string>("all");
  const [search, setSearch] = useState("");
  const bottomRef = useRef<HTMLDivElement>(null);

  const filtered = useMemo(() => {
    let list: LogEntry[] = logs;
    if (level !== "all") {
      list = list.filter((l) => l.type === level);
    }
    if (search) {
      const q = search.toLowerCase();
      list = list.filter((l) => l.payload.toLowerCase().includes(q));
    }
    return list;
  }, [logs, level, search]);

  useEffect(() => {
    if (!paused) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [filtered.length, paused]);

  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex items-center gap-2">
        <select
          className="h-8 rounded-md border border-border bg-background px-2 text-sm outline-none"
          value={level}
          onChange={(e) => setLevel(e.target.value)}
        >
          {LEVELS.map((l) => (
            <option key={l} value={l}>
              {l === "all" ? "全部级别" : l}
            </option>
          ))}
        </select>
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <input
            className="h-8 w-full rounded-md border border-border bg-background pl-8 pr-3 text-sm outline-none focus:ring-1 focus:ring-ring"
            placeholder="搜索日志..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <button
          className="flex h-8 items-center gap-1.5 rounded-md border border-border px-3 text-sm hover:bg-muted"
          onClick={togglePause}
        >
          {paused ? <Play className="size-3.5" /> : <Pause className="size-3.5" />}
          {paused ? "继续" : "暂停"}
        </button>
        <button
          className="flex h-8 items-center gap-1.5 rounded-md border border-border px-3 text-sm text-destructive hover:bg-destructive/10"
          onClick={clear}
        >
          <Trash2 className="size-3.5" />
          清空
        </button>
      </div>

      <div className="flex-1 overflow-auto rounded-lg border border-border bg-background p-2 font-mono text-xs">
        {filtered.map((log, i) => (
          <div key={i} className="flex gap-2 py-0.5">
            <span className="shrink-0 text-muted-foreground">
              {formatTime(log.time)}
            </span>
            <span
              className={cn(
                "shrink-0 w-14 text-right",
                LEVEL_COLORS[log.type] ?? "text-foreground",
              )}
            >
              [{log.type}]
            </span>
            <span className="break-all">{log.payload}</span>
          </div>
        ))}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
