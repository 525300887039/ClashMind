import { z } from "zod";

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
type JsonRpcHandler = (params: JsonRpcParams) => Promise<unknown>;

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

export function registerHandler(method: string, handler: JsonRpcHandler): void {
  handlers.set(method, handler);
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
    const result = await handler(rpcRequest.params ?? {});
    if (rpcRequest.id === undefined) {
      return null;
    }
    return createSuccessResponse(rpcRequest.id, result);
  } catch (error) {
    if (rpcRequest.id === undefined) {
      return null;
    }

    return createErrorResponse(
      rpcRequest.id,
      -32603,
      error instanceof Error ? error.message : "Internal error",
    );
  }
}
