import { useEffect, useState } from "react";
import { useAppStore } from "@/stores/app-store";

export function useTheme() {
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const [resolvedTheme, setResolvedTheme] = useState<"light" | "dark">(() => {
    if (theme !== "system") return theme;
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  });

  useEffect(() => {
    const apply = (t: "light" | "dark") => {
      setResolvedTheme(t);
      document.documentElement.classList.toggle("dark", t === "dark");
    };

    if (theme !== "system") {
      apply(theme);
      return;
    }

    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    apply(mq.matches ? "dark" : "light");
    const handler = (e: MediaQueryListEvent) =>
      apply(e.matches ? "dark" : "light");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  return { theme, setTheme, resolvedTheme };
}
