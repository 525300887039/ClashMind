import { z } from "zod";

export const providerIdSchema = z.enum(["openai", "claude", "deepseek", "ollama"]);

export type ProviderId = z.infer<typeof providerIdSchema>;

export const providerSettingsSchema = z
  .object({
    provider: providerIdSchema,
    model: z.string().min(1),
    apiKey: z.string().min(1).optional(),
    baseUrl: z.string().min(1).optional(),
    temperature: z.number().finite().optional(),
  })
  .strict();

export interface ProviderSettings extends z.infer<typeof providerSettingsSchema> {}

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
