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
