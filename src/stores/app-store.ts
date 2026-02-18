import { create } from "zustand";
import { persist } from "zustand/middleware";
import {
  MIHOMO_DEFAULT_ADDRESS,
  DEFAULT_CONFIG_DIR,
  DEFAULT_HTTP_PORT,
  DEFAULT_SOCKS_PORT,
} from "@/lib/constants";

export type Page =
  | "proxies"
  | "connections"
  | "rules"
  | "logs"
  | "config"
  | "settings";

export type Theme = "light" | "dark" | "system";

interface AppState {
  theme: Theme;
  sidebarCollapsed: boolean;
  currentPage: Page;
  mihomoConfigDir: string;
  apiAddress: string;
  apiSecret: string;
  httpPort: number;
  socksPort: number;
  autoStart: boolean;
  language: "zh-CN" | "en-US";
  setTheme: (theme: Theme) => void;
  toggleSidebar: () => void;
  setCurrentPage: (page: Page) => void;
  updateSettings: (
    settings: Partial<
      Pick<
        AppState,
        | "mihomoConfigDir"
        | "apiAddress"
        | "apiSecret"
        | "httpPort"
        | "socksPort"
        | "autoStart"
        | "language"
      >
    >,
  ) => void;
}

export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
      theme: "system",
      sidebarCollapsed: false,
      currentPage: "proxies",
      mihomoConfigDir: DEFAULT_CONFIG_DIR,
      apiAddress: MIHOMO_DEFAULT_ADDRESS,
      apiSecret: "",
      httpPort: DEFAULT_HTTP_PORT,
      socksPort: DEFAULT_SOCKS_PORT,
      autoStart: false,
      language: "zh-CN",
      setTheme: (theme) => set({ theme }),
      toggleSidebar: () =>
        set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
      setCurrentPage: (currentPage) => set({ currentPage }),
      updateSettings: (settings) => set(settings),
    }),
    {
      name: "clashmind-store",
      partialize: (state) => ({
        theme: state.theme,
        sidebarCollapsed: state.sidebarCollapsed,
        currentPage: state.currentPage,
        mihomoConfigDir: state.mihomoConfigDir,
        apiAddress: state.apiAddress,
        apiSecret: state.apiSecret,
        httpPort: state.httpPort,
        socksPort: state.socksPort,
        autoStart: state.autoStart,
        language: state.language,
      }),
    },
  ),
);
