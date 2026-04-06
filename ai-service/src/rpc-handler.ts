import { stepCountIs, streamText } from "ai";
import { z } from "zod";

import { assemblePrompt } from "./prompts/index.js";
import { createModel } from "./providers/index.js";
import {
  type ConfigDiff,
  generateDiff,
  generateDiffFromConfigDocument,
  isPendingConfigChange,
  type ValidationError,
  type ValidationResult,
  validateBeforeApply,
} from "./safety/index.js";
import { allTools } from "./tools/index.js";
import { handleRustCallbackResponse, requestFromRust } from "./tools/rust-rpc.js";
import {
  chatParamsSchema,
  type ChatParams,
  type StreamEvent,
} from "./types.js";

const jsonRpcIdSchema = z.union([z.string(), z.number()]);
const jsonRpcParamsSchema = z.record(z.string(), z.unknown());
const jsonRpcRequestSchema = z.object({
  jsonrpc: z.literal("2.0"),
  id: jsonRpcIdSchema.optional(),
  method: z.string().min(1),
  params: jsonRpcParamsSchema.optional(),
});

export type JsonRpcId = z.infer<typeof jsonRpcIdSchema>;
type JsonRpcParams = z.infer<typeof jsonRpcParamsSchema>;
const HANDLED_EXTERNALLY = Symbol("handledExternally");

interface JsonRpcHandlerContext {
  id: JsonRpcId | null;
  writeResult(result: unknown): void;
}

type JsonRpcHandlerResult = unknown | typeof HANDLED_EXTERNALLY;
type JsonRpcHandler = (
  params: JsonRpcParams,
  context: JsonRpcHandlerContext,
) => Promise<JsonRpcHandlerResult>;

interface ConfigApplyPayload {
  originalConfig: string;
  modifiedConfig: string;
}

interface SanitizedConfigResponse {
  source: string;
  sanitized: boolean;
  config: Record<string, unknown>;
}

const REDACTED_VALUE = "<redacted>";

export interface JsonRpcError {
  code: number;
  message: string;
  data?: unknown;
}

export interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: JsonRpcId | null;
  result?: unknown;
  error?: JsonRpcError;
}

const handlers = new Map<string, JsonRpcHandler>([
  [
    "ping",
    async () => ({
      pong: true,
      timestamp: Date.now(),
    }),
  ],
  ["echo", async (params) => params],
]);

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function extractRequestId(input: unknown): JsonRpcId | null {
  if (!isObjectRecord(input) || !("id" in input)) {
    return null;
  }

  const parsedId = jsonRpcIdSchema.safeParse(input.id);
  return parsedId.success ? parsedId.data : null;
}

function createErrorResponse(
  id: JsonRpcId | null,
  code: number,
  message: string,
  data?: unknown,
): JsonRpcResponse {
  return {
    jsonrpc: "2.0",
    id,
    error: {
      code,
      message,
      ...(data === undefined ? {} : { data }),
    },
  };
}

function createSuccessResponse(id: JsonRpcId, result: unknown): JsonRpcResponse {
  return {
    jsonrpc: "2.0",
    id,
    result,
  };
}

function writeResponse(response: JsonRpcResponse): void {
  process.stdout.write(`${JSON.stringify(response)}\n`);
}

function createHandlerContext(id: JsonRpcId | null): JsonRpcHandlerContext {
  return {
    id,
    writeResult(result: unknown) {
      if (id === null) {
        return;
      }

      writeResponse(createSuccessResponse(id, result));
    },
  };
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Internal error";
}

function normalizeToolInput(input: unknown): Record<string, unknown> {
  return redactSensitiveValue(isObjectRecord(input) ? input : { value: input });
}

function normalizeConfigKey(key: string): string {
  return key
    .split("")
    .filter((character) => /[a-zA-Z0-9]/.test(character))
    .join("")
    .toLowerCase();
}

function isSensitiveConfigKey(key: string): boolean {
  return [
    "password",
    "passwd",
    "secret",
    "token",
    "uuid",
    "apikey",
    "privatekey",
    "auth",
    "authstr",
    "authorization",
    "clientsecret",
    "users",
  ].includes(normalizeConfigKey(key));
}

function redactSensitiveValue<T>(value: T): T {
  if (Array.isArray(value)) {
    return value.map((item) => redactSensitiveValue(item)) as T;
  }

  if (isObjectRecord(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([key, entryValue]) => [
        key,
        isSensitiveConfigKey(key) ? REDACTED_VALUE : redactSensitiveValue(entryValue),
      ]),
    ) as T;
  }

  return value;
}

function isSanitizedConfigResponse(value: unknown): value is SanitizedConfigResponse {
  return (
    isObjectRecord(value) &&
    typeof value.source === "string" &&
    typeof value.sanitized === "boolean" &&
    isObjectRecord(value.config)
  );
}

function formatValidationError(issue: ValidationError): string {
  return issue.path.length === 0 ? issue.message : `${issue.path}: ${issue.message}`;
}

function createValidationFailureError(validation: ValidationResult): Error {
  const detail = validation.errors.map(formatValidationError).join("; ");
  return new Error(detail.length > 0 ? `配置校验失败: ${detail}` : "配置校验失败");
}

async function buildPendingConfigPreview(
  toolResults: Parameters<typeof generateDiffFromConfigDocument>[1],
): Promise<{
  diff: ConfigDiff;
  applyPayload: ConfigApplyPayload;
  validation: ValidationResult;
}> {
  const sanitizedToolResults =
    redactSensitiveValue(structuredClone(toolResults)) as Parameters<
      typeof generateDiffFromConfigDocument
    >[1];
  const [configFileContent, sanitizedConfigResponse] = await Promise.all([
    requestFromRust<string>("read_active_config_file"),
    requestFromRust<SanitizedConfigResponse>("get_config_file"),
  ]);

  if (typeof configFileContent !== "string") {
    throw new Error("active config file response is invalid");
  }

  if (!isSanitizedConfigResponse(sanitizedConfigResponse)) {
    throw new Error("sanitized config file response is invalid");
  }

  const applyDiff = generateDiff(configFileContent, toolResults);
  const applyValidation = validateBeforeApply(
    configFileContent,
    applyDiff.modified,
  );
  if (!applyValidation.valid) {
    throw createValidationFailureError(applyValidation);
  }

  const previewDiff = generateDiffFromConfigDocument(
    sanitizedConfigResponse.config,
    sanitizedToolResults,
  );

  return {
    diff: previewDiff,
    applyPayload: {
      originalConfig: configFileContent,
      modifiedConfig: applyDiff.modified,
    },
    validation: applyValidation,
  };
}

registerHandler("chat", async (params, context) => {
  const parsedParams = chatParamsSchema.safeParse(params);

  if (!parsedParams.success) {
    throw new Error(parsedParams.error.issues.map((issue) => issue.message).join("; "));
  }

  const chatParams: ChatParams = parsedParams.data;
  const prompt = assemblePrompt(chatParams.messages, chatParams.context);
  const result = streamText({
    model: createModel(chatParams.settings),
    system: prompt.system,
    messages: prompt.messages,
    tools: allTools,
    stopWhen: stepCountIs(5),
    ...(chatParams.settings.temperature === undefined
      ? {}
      : { temperature: chatParams.settings.temperature }),
  });

  try {
    const pendingConfigChanges: Array<{
      toolCallId: string;
      change: Parameters<typeof generateDiffFromConfigDocument>[1][number];
    }> = [];
    let confirmationBatchId: string | null = null;
    for await (const chunk of result.fullStream) {
      let event: StreamEvent | null = null;

      switch (chunk.type) {
        case "text-delta":
          event = {
            type: "text_delta",
            content: chunk.text,
          };
          break;
        case "tool-call":
          event = {
            type: "tool_call",
            name: chunk.toolName,
            id: chunk.toolCallId,
            input: normalizeToolInput(chunk.input),
          };
          break;
        case "tool-result":
          if (isPendingConfigChange(chunk.output)) {
            pendingConfigChanges.push({
              toolCallId: chunk.toolCallId,
              change: chunk.output,
            });

            confirmationBatchId ??= crypto.randomUUID();
            const { diff, applyPayload, validation } = await buildPendingConfigPreview(
              pendingConfigChanges.map((item) => item.change),
            );

            for (const pendingChange of pendingConfigChanges) {
              const sanitizedPendingChange = redactSensitiveValue(
                structuredClone(pendingChange.change),
              );
              context.writeResult({
                type: "tool_result",
                id: pendingChange.toolCallId,
                content: {
                  ...sanitizedPendingChange,
                  diff,
                  applyPayload,
                  validation,
                  confirmationBatchId,
                  confirmationBatchSize: pendingConfigChanges.length,
                  isLatestInBatch: pendingChange.toolCallId === chunk.toolCallId,
                },
              } satisfies StreamEvent);
            }
            break;
          }

          event = {
            type: "tool_result",
            id: chunk.toolCallId,
            content: chunk.output,
          };
          break;
        case "error":
          event = {
            type: "error",
            message: getErrorMessage(chunk.error),
          };
          break;
        case "finish":
          event = {
            type: "done",
            tokensUsed: chunk.totalUsage.totalTokens ?? undefined,
          };
          break;
        default:
          break;
      }

      if (event !== null) {
        context.writeResult(event);
      }
    }
  } catch (error) {
    context.writeResult({
      type: "error",
      message: getErrorMessage(error),
    } satisfies StreamEvent);
  }

  return HANDLED_EXTERNALLY;
});

export function registerHandler(method: string, handler: JsonRpcHandler): void {
  handlers.set(method, handler);
}

export async function handleRpcMessage(message: unknown): Promise<JsonRpcResponse | null> {
  if (handleRustCallbackResponse(message)) {
    return null;
  }

  return handleRpcRequest(message);
}

export async function handleRpcRequest(request: unknown): Promise<JsonRpcResponse | null> {
  const parsedRequest = jsonRpcRequestSchema.safeParse(request);

  if (!parsedRequest.success) {
    return createErrorResponse(
      extractRequestId(request),
      -32600,
      "Invalid Request",
      parsedRequest.error.flatten(),
    );
  }

  const rpcRequest = parsedRequest.data;
  const handler = handlers.get(rpcRequest.method);

  if (!handler) {
    return createErrorResponse(
      rpcRequest.id ?? null,
      -32601,
      `Method not found: ${rpcRequest.method}`,
    );
  }

  try {
    const result = await handler(
      rpcRequest.params ?? {},
      createHandlerContext(rpcRequest.id ?? null),
    );

    if (result === HANDLED_EXTERNALLY) {
      return null;
    }

    if (rpcRequest.id === undefined) {
      return null;
    }

    return createSuccessResponse(rpcRequest.id, result);
  } catch (error) {
    if (rpcRequest.id === undefined) {
      return null;
    }

    return createErrorResponse(rpcRequest.id, -32603, getErrorMessage(error));
  }
}
