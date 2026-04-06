import { stepCountIs, streamText, type ModelMessage } from "ai";
import { z } from "zod";

import { createModel } from "./providers/index.js";
import { allTools } from "./tools/index.js";
import { handleRustCallbackResponse } from "./tools/rust-rpc.js";
import {
  chatParamsSchema,
  type ChatContext,
  type ChatMessage,
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

function toModelMessage(message: ChatMessage): ModelMessage {
  return {
    role: message.role,
    content: message.content,
  };
}

function buildContextMessage(chatContext: ChatContext | undefined): ModelMessage | null {
  if (chatContext === undefined) {
    return null;
  }

  const sections: string[] = [];

  if (chatContext.currentConfig !== undefined) {
    sections.push(`Current Mihomo config:\n${chatContext.currentConfig}`);
  }

  if (chatContext.recentStats !== undefined) {
    sections.push(`Recent stats JSON:\n${JSON.stringify(chatContext.recentStats, null, 2)}`);
  }

  if (chatContext.availableProxies !== undefined) {
    sections.push(`Available proxies:\n${chatContext.availableProxies.join(", ")}`);
  }

  if (sections.length === 0) {
    return null;
  }

  return {
    role: "system",
    content: [
      "Trusted runtime context from the desktop app.",
      "Use it when answering configuration, proxy, or diagnosis questions.",
      ...sections,
    ].join("\n\n"),
  };
}

function buildModelMessages(chatParams: ChatParams): ModelMessage[] {
  const messages: ModelMessage[] = [];
  const contextMessage = buildContextMessage(chatParams.context);

  if (contextMessage !== null) {
    messages.push(contextMessage);
  }

  messages.push(...chatParams.messages.map(toModelMessage));
  return messages;
}

function normalizeToolInput(input: unknown): Record<string, unknown> {
  return isObjectRecord(input) ? input : { value: input };
}

registerHandler("chat", async (params, context) => {
  const parsedParams = chatParamsSchema.safeParse(params);

  if (!parsedParams.success) {
    throw new Error(parsedParams.error.issues.map((issue) => issue.message).join("; "));
  }

  const chatParams: ChatParams = parsedParams.data;
  const result = streamText({
    model: createModel(chatParams.settings),
    messages: buildModelMessages(chatParams),
    tools: allTools,
    stopWhen: stepCountIs(5),
    ...(chatParams.settings.temperature === undefined
      ? {}
      : { temperature: chatParams.settings.temperature }),
  });

  try {
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
