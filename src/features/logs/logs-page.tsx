import { useState, useMemo, useRef, useEffect } from "react";
import { ScrollText, Trash2, Pause, Play } from "lucide-react";
import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import {
  PageHeader,
  SectionCard,
  SearchInput,
  ActionButton,
} from "@/components/ui";
import { useLogs, type LogEntry } from "./hooks/use-logs";

const LEVELS = ["all", "info", "warning", "error", "debug"] as const;

const LEVEL_COLORS: Record<string, string> = {
  info: "text-primary",
  warning: "text-warning",
  error: "text-destructive",
  debug: "text-muted-foreground",
};

const LEVEL_LABELS: Record<string, string> = {
  all: "全部",
  info: "info",
  warning: "warning",
  error: "error",
  debug: "debug",
};

function formatTime(ts: number) {
  const d = new Date(ts);
  return d.toLocaleTimeString("zh-CN", { hour12: false });
}

function LevelPillFilter({
  value,
  onChange,
}: {
  value: string;
  onChange: (level: string) => void;
}) {
  return (
    <div className="inline-flex items-center rounded-full border border-border/70 bg-muted/30 p-1">
      {LEVELS.map((l) => (
        <button
          key={l}
          type="button"
          onClick={() => onChange(l)}
          className={cn(
            "rounded-full px-3 py-1 text-xs font-medium transition-all",
            value === l
              ? "bg-primary text-primary-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          {LEVEL_LABELS[l]}
        </button>
      ))}
    </div>
  );
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
    <motion.section initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.24, ease: "easeOut" }} className="flex h-full flex-col gap-6">
      <PageHeader
        eyebrow="Event Stream"
        eyebrowIcon={ScrollText}
        title="日志"
        description="实时查看 Mihomo 内核事件与调试信息"
        actions={
          <span
            className={cn(
              "inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-medium",
              paused
                ? "border-amber-500/20 bg-amber-500/10 text-amber-400"
                : "border-primary/20 bg-primary/10 text-primary",
            )}
          >
            {paused ? (
              <Pause className="size-3" />
            ) : (
              <Play className="size-3" />
            )}
            {paused ? "已暂停" : "实时监听中"}
          </span>
        }
      />

      <div className="flex flex-wrap items-center gap-3">
        <LevelPillFilter value={level} onChange={setLevel} />
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder="搜索日志..."
          className="max-w-xs flex-1"
        />
        <ActionButton
          tone={paused ? "primary" : "ghost"}
          onClick={togglePause}
        >
          {paused ? (
            <Play className="size-3.5" />
          ) : (
            <Pause className="size-3.5" />
          )}
          {paused ? "继续" : "暂停"}
        </ActionButton>
        <ActionButton tone="destructive" onClick={clear}>
          <Trash2 className="size-3.5" />
          清空
        </ActionButton>
      </div>

      <SectionCard className="min-h-0 flex-1 overflow-hidden p-0">
        <div className="h-full overflow-auto p-4 font-mono text-xs">
          {filtered.length === 0 ? (
            <div className="flex h-full items-center justify-center text-muted-foreground">
              暂无日志
            </div>
          ) : (
            filtered.map((log, i) => (
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
            ))
          )}
          <div ref={bottomRef} />
        </div>
      </SectionCard>
    </motion.section>
  );
}
