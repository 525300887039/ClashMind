import { invoke } from "@tauri-apps/api/core";

export interface ProxyNode {
  name: string;
  type: string;
  alive: boolean;
  delay: number;
  history: { time: string; delay: number }[];
}

export interface ProxyGroup {
  name: string;
  type: "select" | "url-test" | "fallback" | "load-balance";
  now: string;
  all: string[];
}

export interface ProxiesResponse {
  proxies: Record<string, ProxyNode | ProxyGroup>;
}

export interface Connection {
  id: string;
  metadata: {
    host: string;
    destinationIP: string;
    destinationPort: string;
    sourceIP: string;
    sourcePort: string;
    network: string;
    type: string;
  };
  upload: number;
  download: number;
  start: string;
  chains: string[];
  rule: string;
  rulePayload: string;
}

export interface ConnectionsResponse {
  downloadTotal: number;
  uploadTotal: number;
  connections: Connection[];
}

export interface Rule {
  type: string;
  payload: string;
  proxy: string;
}

export const api = {
  mihomo: {
    start: (configPath: string) => invoke("start_mihomo", { configPath }),
    stop: () => invoke("stop_mihomo"),
    restart: (configPath: string) => invoke("restart_mihomo", { configPath }),
    status: () => invoke<boolean>("get_mihomo_status"),
  },
  proxy: {
    getAll: () => invoke<ProxiesResponse>("get_proxies"),
    switch: (group: string, name: string) => invoke("switch_proxy", { group, name }),
    testDelay: (name: string, url: string, timeout: number) =>
      invoke<number>("test_delay", { name, url, timeout }),
    testGroupDelay: (group: string, url: string, timeout: number) =>
      invoke<Record<string, number>>("test_group_delay", { group, url, timeout }),
  },
  connection: {
    getAll: () => invoke<ConnectionsResponse>("get_connections"),
    close: (id: string) => invoke("close_connection", { id }),
    closeAll: () => invoke("close_all_connections"),
  },
  rule: {
    getAll: () => invoke<{ rules: Rule[] }>("get_rules"),
  },
  config: {
    read: (path: string) => invoke<string>("read_config", { path }),
    write: (path: string, content: string) => invoke("write_config", { path, content }),
    reload: (mihomoUrl: string) => invoke("reload_config", { mihomoUrl }),
    get: () => invoke<Record<string, unknown>>("get_configs"),
    patch: (payload: Record<string, unknown>) => invoke("patch_configs", { payload }),
  },
  system: {
    setProxy: (enable: boolean, port: number) => invoke("set_system_proxy", { enable, port }),
    getProxy: () => invoke<{ enable: boolean; port: number }>("get_system_proxy"),
    getVersion: () => invoke<{ version: string }>("get_version"),
  },
} as const;
