import { z } from "zod";

export const providerIdSchema = z.enum(["openai", "openai_compatible", "claude", "gemini"]);

export type ProviderId = z.infer<typeof providerIdSchema>;

export const providerSettingsSchema = z
  .object({
    provider: providerIdSchema,
    model: z.string().min(1),
    apiKey: z.string().min(1).optional(),
    baseUrl: z.string().min(1).optional(),
    temperature: z.number().finite().optional(),
    maxTokens: z.number().int().positive().optional(),
  })
  .strict();

export interface ProviderSettings extends z.infer<typeof providerSettingsSchema> {}

export const modelCatalogSettingsSchema = z
  .object({
    provider: providerIdSchema,
    apiKey: z.string().min(1).optional(),
    baseUrl: z.string().min(1).optional(),
  })
  .strict();

export interface ModelCatalogSettings extends z.infer<typeof modelCatalogSettingsSchema> {}

export const chatMessageSchema = z
  .object({
    role: z.enum(["user", "assistant", "system"]),
    content: z.string(),
  })
  .strict();

export interface ChatMessage extends z.infer<typeof chatMessageSchema> {}

export const chatContextSchema = z
  .object({
    currentConfig: z.string().optional(),
    recentStats: z.record(z.string(), z.unknown()).optional(),
    availableProxies: z.array(z.string()).optional(),
  })
  .strict();

export interface ChatContext extends z.infer<typeof chatContextSchema> {}

export const chatParamsSchema = z
  .object({
    messages: z.array(chatMessageSchema).min(1),
    context: chatContextSchema.optional(),
    settings: providerSettingsSchema,
  })
  .strict();

export interface ChatParams extends z.infer<typeof chatParamsSchema> {}

export const errorCategoryCountSchema = z
  .object({
    category: z.string().min(1),
    count: z.number().int().nonnegative(),
  })
  .strict();

export interface ErrorCategoryCount extends z.infer<typeof errorCategoryCountSchema> {}

export const proxyErrorCountSchema = z
  .object({
    proxyNode: z.string().min(1),
    count: z.number().int().nonnegative(),
  })
  .strict();

export interface ProxyErrorCount extends z.infer<typeof proxyErrorCountSchema> {}

export const hostFailureRateSchema = z
  .object({
    host: z.string().min(1),
    failureCount: z.number().int().nonnegative(),
    totalCount: z.number().int().nonnegative(),
    failureRate: z.number().finite().nonnegative(),
  })
  .strict();

export interface HostFailureRate extends z.infer<typeof hostFailureRateSchema> {}

export const diagnosisSummarySchema = z
  .object({
    timeRangeMinutes: z.number().int().min(1),
    errorStats: z.array(errorCategoryCountSchema),
    topErrorNodes: z.array(proxyErrorCountSchema),
    topFailureHosts: z.array(hostFailureRateSchema),
    dnsErrorCount: z.number().int().nonnegative(),
    matchFallbackCount: z.number().int().nonnegative(),
    totalConnections: z.number().int().nonnegative(),
    generatedAt: z.string().min(1),
  })
  .strict();

export interface DiagnosisSummary extends z.infer<typeof diagnosisSummarySchema> {}

export const alertSeveritySchema = z.enum(["critical", "warning", "info"]);

export type AlertSeverity = z.infer<typeof alertSeveritySchema>;

export const alertTypeSchema = z.enum([
  "high_timeout_rate",
  "traffic_surge",
  "traffic_drop",
  "high_match_fallback",
  "dns_failure_cluster",
]);

export type AlertType = z.infer<typeof alertTypeSchema>;

export const anomalyAlertSchema = z
  .object({
    id: z.string().min(1),
    severity: alertSeveritySchema,
    alertType: alertTypeSchema,
    title: z.string().min(1),
    description: z.string().min(1),
    context: z.record(z.string(), z.unknown()),
    detectedAt: z.string().min(1),
  })
  .strict();

export interface AnomalyAlert extends z.infer<typeof anomalyAlertSchema> {}

export const diagnosisToolParamsSchema = z
  .object({
    timeRangeMinutes: z.number().int().min(5).max(1_440).default(30),
  })
  .strict();

export interface DiagnosisToolParams extends z.infer<typeof diagnosisToolParamsSchema> {}

export const diagnosisParamsSchema = z
  .object({
    timeRangeMinutes: z.number().int().min(5).max(1_440).default(30),
    settings: providerSettingsSchema,
  })
  .strict();

export interface DiagnosisParams extends z.infer<typeof diagnosisParamsSchema> {}

export const diagnosisPayloadSchema = z
  .object({
    summary: diagnosisSummarySchema,
    alerts: z.array(anomalyAlertSchema),
  })
  .strict();

export interface DiagnosisPayload extends z.infer<typeof diagnosisPayloadSchema> {}

export const diagnosisResultSchema = z
  .object({
    report: z.string().min(1),
    summary: diagnosisSummarySchema,
    alerts: z.array(anomalyAlertSchema),
    generatedAt: z.string().min(1),
  })
  .strict();

export interface DiagnosisResult extends z.infer<typeof diagnosisResultSchema> {}

export const reportTypeSchema = z.enum(["daily", "weekly"]);

export type ReportType = z.infer<typeof reportTypeSchema>;

export const reportPeriodSchema = z
  .object({
    start: z.string().min(1),
    end: z.string().min(1),
  })
  .strict();

export interface ReportPeriod extends z.infer<typeof reportPeriodSchema> {}

export const reportTrafficSummarySchema = z
  .object({
    upload: z.number().int().nonnegative(),
    download: z.number().int().nonnegative(),
  })
  .strict();

export interface ReportTrafficSummary extends z.infer<typeof reportTrafficSummarySchema> {}

export const reportDomainStatSchema = z
  .object({
    domain: z.string().min(1),
    traffic: z.number().int().nonnegative(),
  })
  .strict();

export interface ReportDomainStat extends z.infer<typeof reportDomainStatSchema> {}

export const reportRuleStatSchema = z
  .object({
    rule: z.string().min(1),
    hitCount: z.number().int().nonnegative(),
  })
  .strict();

export interface ReportRuleStat extends z.infer<typeof reportRuleStatSchema> {}

export const reportComparisonSchema = z
  .object({
    trafficChange: z.number().finite(),
    connectionChange: z.number().finite(),
  })
  .strict();

export interface ReportComparison extends z.infer<typeof reportComparisonSchema> {}

export const reportDailyTrendPointSchema = z
  .object({
    date: z.string().min(1),
    upload: z.number().int().nonnegative(),
    download: z.number().int().nonnegative(),
    totalTraffic: z.number().int().nonnegative(),
    connCount: z.number().int().nonnegative(),
  })
  .strict();

export interface ReportDailyTrendPoint extends z.infer<typeof reportDailyTrendPointSchema> {}

function nullToUndefined<TSchema extends z.ZodTypeAny>(schema: TSchema) {
  return z.preprocess(
    (value) => (value === null ? undefined : value),
    schema.optional(),
  );
}

export const reportStatsSchema = z
  .object({
    totalTraffic: reportTrafficSummarySchema,
    totalConnections: z.number().int().nonnegative(),
    topDomains: z.array(reportDomainStatSchema),
    topRules: z.array(reportRuleStatSchema),
    peakHour: nullToUndefined(z.string().min(1)),
    comparison: nullToUndefined(reportComparisonSchema),
    dailyTrend: nullToUndefined(z.array(reportDailyTrendPointSchema)),
    matchRate: nullToUndefined(z.number().finite()),
  })
  .strict();

export interface ReportStats extends z.infer<typeof reportStatsSchema> {}

export const reportStatsPayloadSchema = z
  .object({
    period: reportPeriodSchema,
    stats: reportStatsSchema,
  })
  .strict();

export interface ReportStatsPayload extends z.infer<typeof reportStatsPayloadSchema> {}

const reportDateSchema = z
  .string()
  .regex(/^\d{4}-\d{2}-\d{2}$/, "date must match YYYY-MM-DD");

export const reportParamsSchema = z
  .object({
    type: reportTypeSchema,
    date: reportDateSchema.optional(),
    settings: providerSettingsSchema,
  })
  .strict();

export interface ReportParams extends z.infer<typeof reportParamsSchema> {}

export const reportResultSchema = z
  .object({
    type: reportTypeSchema,
    period: reportPeriodSchema,
    content: z.string(),
    stats: reportStatsSchema,
    generatedAt: z.string().min(1),
  })
  .strict();

export interface ReportResult extends z.infer<typeof reportResultSchema> {}

export const connectionTestParamsSchema = z
  .object({
    settings: providerSettingsSchema,
  })
  .strict();

export interface ConnectionTestParams extends z.infer<typeof connectionTestParamsSchema> {}

export const connectionTestResultSchema = z
  .object({
    success: z.boolean(),
    latencyMs: z.number().int().nonnegative(),
    message: z.string().min(1),
  })
  .strict();

export interface ConnectionTestResult extends z.infer<typeof connectionTestResultSchema> {}

export const modelCatalogParamsSchema = z
  .object({
    settings: modelCatalogSettingsSchema,
  })
  .strict();

export interface ModelCatalogParams extends z.infer<typeof modelCatalogParamsSchema> {}

export const modelCatalogResultSchema = z
  .object({
    models: z.array(z.string().min(1)),
    source: z.enum(["remote", "fallback", "empty"]),
    message: z.string().min(1),
  })
  .strict();

export interface ModelCatalogResult extends z.infer<typeof modelCatalogResultSchema> {}

const textDeltaEventSchema = z
  .object({
    type: z.literal("text_delta"),
    content: z.string(),
  })
  .strict();

const toolCallEventSchema = z
  .object({
    type: z.literal("tool_call"),
    name: z.string().min(1),
    id: z.string().min(1),
    input: z.record(z.string(), z.unknown()),
  })
  .strict();

const toolResultEventSchema = z
  .object({
    type: z.literal("tool_result"),
    id: z.string().min(1),
    content: z.unknown(),
  })
  .strict();

const errorEventSchema = z
  .object({
    type: z.literal("error"),
    message: z.string().min(1),
  })
  .strict();

const doneEventSchema = z
  .object({
    type: z.literal("done"),
    tokensUsed: z.number().int().nonnegative().optional(),
  })
  .strict();

export const streamEventSchema = z.discriminatedUnion("type", [
  textDeltaEventSchema,
  toolCallEventSchema,
  toolResultEventSchema,
  errorEventSchema,
  doneEventSchema,
]);

export type StreamEvent = z.infer<typeof streamEventSchema>;

export function isStreamEvent(value: unknown): value is StreamEvent {
  return streamEventSchema.safeParse(value).success;
}

export function isTerminalStreamEvent(event: StreamEvent): boolean {
  return event.type === "done" || event.type === "error";
}
