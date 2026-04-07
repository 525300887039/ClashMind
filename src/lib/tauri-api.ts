import { invoke } from "@tauri-apps/api/core";
import { isRecord } from "./utils";

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

export interface DomainStat {
  domain: string;
  hitCount: number;
  upload: number;
  download: number;
}

export interface TrafficPoint {
  time: string;
  upload: number;
  download: number;
  connCount: number;
}

export interface StatsOverview {
  totalConnections: number;
  totalUpload: number;
  totalDownload: number;
  activeConnections: number;
  uniqueDomains: number;
}

export interface RuleStat {
  rule: string;
  hitCount: number;
  upload: number;
  download: number;
}

export interface GeoStat {
  countryCode: string;
  country: string;
  connCount: number;
  totalTraffic: number;
}

export type AiProviderKind = "openai" | "openai_compatible" | "claude" | "gemini";

export type AiChatRole = "user" | "assistant" | "system";

export interface AiProviderSettings {
  provider: AiProviderKind;
  model: string;
  apiKey?: string;
  baseUrl?: string;
  temperature?: number;
  maxTokens?: number;
}

export interface AiSettings {
  provider: AiProviderKind;
  model: string;
  apiKey: string;
  baseUrl: string;
  temperature: number;
  maxTokens: number;
  autoStart: boolean;
}

export interface AiChatMessage {
  role: AiChatRole;
  content: string;
}

export interface AiChatContext {
  currentConfig?: string;
  recentStats?: Record<string, unknown>;
  availableProxies?: string[];
}

export interface AiChatParams {
  messages: AiChatMessage[];
  context?: AiChatContext;
  settings: AiProviderSettings;
}

export interface AiPingResponse {
  pong: boolean;
  timestamp: number;
}

export interface AiConnectionTestResult {
  success: boolean;
  latencyMs: number;
  message: string;
}

export type AiModelCatalogSource = "remote" | "fallback" | "empty";

export interface AiModelCatalog {
  models: string[];
  source: AiModelCatalogSource;
  message: string;
}

export type ReportType = "daily" | "weekly";

export interface ReportPeriod {
  start: string;
  end: string;
}

export interface ReportTrafficSummary {
  upload: number;
  download: number;
}

export interface ReportDomainStat {
  domain: string;
  traffic: number;
}

export interface ReportRuleSummary {
  rule: string;
  hitCount: number;
}

export interface ReportComparison {
  trafficChange: number;
  connectionChange: number;
}

export interface ReportDailyTrendPoint {
  date: string;
  upload: number;
  download: number;
  totalTraffic: number;
  connCount: number;
}

export interface ReportStats {
  totalTraffic: ReportTrafficSummary;
  totalConnections: number;
  topDomains: ReportDomainStat[];
  topRules: ReportRuleSummary[];
  peakHour?: string;
  comparison?: ReportComparison;
  dailyTrend?: ReportDailyTrendPoint[];
  matchRate?: number;
}

export interface ReportResult {
  type: ReportType;
  period: ReportPeriod;
  content: string;
  stats: ReportStats;
  generatedAt: string;
}

export interface ConfigSnapshot {
  id: number;
  content: string;
  source: "manual" | "ai";
  description: string | null;
  filePath: string | null;
  createdAt: string;
}

export interface SaveConversationMessageParams {
  role: AiChatRole;
  content: string;
  toolCalls?: unknown;
  tokensUsed?: number;
  model?: string;
}

export interface ConfigApplyPayload {
  originalConfig: string;
  modifiedConfig: string;
}

export type ConfigChangeAction =
  | "add_proxy"
  | "remove_proxy"
  | "add_proxy_group"
  | "update_proxy_group"
  | "add_rule"
  | "remove_rule"
  | "update_dns"
  | "set_mode";

const CONFIG_CHANGE_ACTIONS: ConfigChangeAction[] = [
  "add_proxy",
  "remove_proxy",
  "add_proxy_group",
  "update_proxy_group",
  "add_rule",
  "remove_rule",
  "update_dns",
  "set_mode",
];

export interface DiffChange {
  type: "add" | "remove" | "modify";
  path: string;
  description: string;
}

export interface ConfigDiff {
  original: string;
  modified: string;
  unifiedDiff: string;
  summary: string;
  changes: DiffChange[];
}

export interface PendingConfigChangeResult {
  action: ConfigChangeAction;
  params: Record<string, unknown>;
  status: "pending_confirmation";
  diff: ConfigDiff;
  confirmationBatchId: string;
  confirmationBatchSize: number;
  isLatestInBatch: boolean;
}

export type AiStreamEvent =
  | { type: "text_delta"; content: string }
  | { type: "tool_call"; name: string; id: string; input: Record<string, unknown> }
  | { type: "tool_result"; id: string; content: unknown }
  | { type: "error"; message: string }
  | { type: "done"; tokensUsed?: number };

function isConfigChangeAction(value: unknown): value is ConfigChangeAction {
  return typeof value === "string" && CONFIG_CHANGE_ACTIONS.includes(value as ConfigChangeAction);
}

function isDiffChange(value: unknown): value is DiffChange {
  return (
    isRecord(value) &&
    (value.type === "add" || value.type === "remove" || value.type === "modify") &&
    typeof value.path === "string" &&
    typeof value.description === "string"
  );
}

function isConfigDiff(value: unknown): value is ConfigDiff {
  return (
    isRecord(value) &&
    typeof value.original === "string" &&
    typeof value.modified === "string" &&
    typeof value.unifiedDiff === "string" &&
    typeof value.summary === "string" &&
    Array.isArray(value.changes) &&
    value.changes.every(isDiffChange)
  );
}

export function isPendingConfigChangeResult(
  value: unknown,
): value is PendingConfigChangeResult {
  return (
    isRecord(value) &&
    value.status === "pending_confirmation" &&
    isConfigChangeAction(value.action) &&
    isRecord(value.params) &&
    isConfigDiff(value.diff) &&
    typeof value.confirmationBatchId === "string" &&
    typeof value.confirmationBatchSize === "number" &&
    Number.isInteger(value.confirmationBatchSize) &&
    value.confirmationBatchSize > 0 &&
    typeof value.isLatestInBatch === "boolean"
  );
}

export const api = {
  mihomo: {
    start: (configPath: string) => invoke("start_mihomo", { configPath }),
    stop: () => invoke("stop_mihomo"),
    restart: (configPath: string) => invoke("restart_mihomo", { configPath }),
    status: () => invoke<boolean>("get_mihomo_status"),
    checkConfig: (configPath: string) => invoke<boolean>("check_config_exists", { configPath }),
    ensureDefaultConfig: (configPath: string) => invoke("ensure_default_config", { configPath }),
  },
  ai: {
    start: () => invoke("start_ai_service"),
    stop: () => invoke("stop_ai_service"),
    status: () => invoke<boolean>("get_ai_status"),
    getSettings: () => invoke<AiSettings>("get_ai_settings"),
    setSettings: (settings: AiSettings) => invoke("set_ai_settings", { settings }),
    chat: (params: AiChatParams) => invoke("ai_chat", { params }),
    generateReport: (
      type: ReportType,
      date: string | undefined,
      settings: AiProviderSettings,
    ) =>
      invoke<ReportResult>("ai_generate_report", {
        reportType: type,
        date,
        settings,
      }),
    ping: () => invoke<AiPingResponse>("ai_ping"),
    testConnection: (settings: AiSettings) =>
      invoke<AiConnectionTestResult>("test_ai_connection", { settings }),
    fetchModels: (settings: AiSettings) =>
      invoke<AiModelCatalog>("fetch_ai_models", { settings }),
    listSnapshots: (limit: number) =>
      invoke<ConfigSnapshot[]>("list_snapshots", { limit }),
    restoreSnapshot: (id: number) => invoke<void>("restore_snapshot", { id }),
    createSnapshot: (description?: string, filePath?: string) =>
      invoke<number>("create_snapshot", { description, filePath }),
    saveConversationMessage: (params: SaveConversationMessageParams) =>
      invoke<number>("save_conversation_message", { params }),
    applyConfigChange: (payload: ConfigApplyPayload) =>
      invoke("apply_config_change", {
        originalConfig: payload.originalConfig,
        modifiedConfig: payload.modifiedConfig,
      }),
    rejectConfigChange: () => invoke("reject_config_change"),
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
  stats: {
    domains: (days: number, limit: number) =>
      invoke<DomainStat[]>("get_domain_stats", { days, limit }),
    trafficHourly: (start: string, end: string) =>
      invoke<TrafficPoint[]>("get_traffic_hourly", { start, end }),
    trafficDaily: (start: string, end: string) =>
      invoke<TrafficPoint[]>("get_traffic_daily", { start, end }),
    overview: (days: number) => invoke<StatsOverview>("get_stats_overview", { days }),
    rules: (days: number, limit: number) =>
      invoke<RuleStat[]>("get_rule_stats", { days, limit }),
    geo: (days: number) => invoke<GeoStat[]>("get_geo_stats", { days }),
  },
  config: {
    read: (path: string) => invoke<string>("read_config", { path }),
    write: (path: string, content: string) => invoke("write_config", { path, content }),
    reload: () => invoke("reload_config"),
    get: () => invoke<Record<string, unknown>>("get_configs"),
    patch: (payload: Record<string, unknown>) => invoke("patch_configs", { payload }),
  },
  system: {
    setProxy: (enable: boolean, port: number) => invoke("set_system_proxy", { enable, port }),
    getProxy: () => invoke<{ enable: boolean; port: number }>("get_system_proxy"),
    getVersion: () => invoke<{ version: string }>("get_version"),
    updateMihomoClient: (baseUrl: string, secret: string) =>
      invoke("update_mihomo_client", { baseUrl, secret }),
  },
  collector: {
    start: () => invoke("start_collector"),
    stop: () => invoke("stop_collector"),
    status: () => invoke<boolean>("get_collector_status"),
  },
} as const;
