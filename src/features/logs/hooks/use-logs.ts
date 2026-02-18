import { useState, useEffect, useRef, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";

export interface LogEntry {
  type: string;
  payload: string;
  time: number;
}

const MAX_LOGS = 1000;

export function useLogs() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const pausedRef = useRef(false);
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    const unlisten = listen<{ type: string; payload: string }>(
      "log-update",
      (e) => {
        if (pausedRef.current) return;
        setLogs((prev) => {
          const next = [...prev, { ...e.payload, time: Date.now() }];
          return next.length > MAX_LOGS ? next.slice(-MAX_LOGS) : next;
        });
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const clear = useCallback(() => setLogs([]), []);

  const togglePause = useCallback(() => {
    pausedRef.current = !pausedRef.current;
    setPaused(pausedRef.current);
  }, []);

  return { logs, paused, clear, togglePause };
}
