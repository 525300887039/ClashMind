import { z } from "zod";

const CALLBACK_TIMEOUT_MS = 10_000;

const jsonRpcIdSchema = z.union([z.string().min(1), z.number().int()]);

const rustCallbackResponseSchema = z
  .union([
    z
      .object({
        jsonrpc: z.literal("2.0"),
        id: jsonRpcIdSchema,
        result: z.unknown(),
      })
      .strict(),
    z
      .object({
        jsonrpc: z.literal("2.0"),
        id: jsonRpcIdSchema,
        error: z
          .object({
            code: z.number().int(),
            message: z.string().min(1),
            data: z.unknown().optional(),
          })
          .strict(),
      })
      .strict(),
  ])
  .readonly();

type RustCallbackResponse = z.infer<typeof rustCallbackResponseSchema>;

type RustCallbackParams = Record<string, unknown>;

interface PendingRustCallback {
  method: string;
  timeout: NodeJS.Timeout;
  resolve(result: unknown): void;
  reject(error: Error): void;
}

const pendingCallbacks = new Map<string, PendingRustCallback>();

export interface PendingConfirmationResult<
  TAction extends string,
  TParams extends RustCallbackParams,
> {
  action: TAction;
  params: TParams;
  status: "pending_confirmation";
}

function createCallbackError(
  method: string,
  message: string,
  data?: unknown,
): Error {
  const details =
    data === undefined ? "" : ` (${JSON.stringify(data)})`;

  return new Error(`Rust callback "${method}" failed: ${message}${details}`);
}

function normalizeCallbackId(id: RustCallbackResponse["id"]): string {
  return typeof id === "string" ? id : id.toString();
}

function consumeCallbackResponse(response: RustCallbackResponse): void {
  const callbackId = normalizeCallbackId(response.id);
  const pendingCallback = pendingCallbacks.get(callbackId);

  if (pendingCallback === undefined) {
    process.stderr.write(
      `[ai-service] received callback response for unknown id: ${callbackId}\n`,
    );
    return;
  }

  pendingCallbacks.delete(callbackId);
  clearTimeout(pendingCallback.timeout);

  if ("error" in response) {
    pendingCallback.reject(
      createCallbackError(
        pendingCallback.method,
        response.error.message,
        response.error.data,
      ),
    );
    return;
  }

  pendingCallback.resolve(response.result);
}

export function handleRustCallbackResponse(message: unknown): boolean {
  const parsedResponse = rustCallbackResponseSchema.safeParse(message);

  if (!parsedResponse.success) {
    return false;
  }

  consumeCallbackResponse(parsedResponse.data);
  return true;
}

export function requestFromRust<TResult = unknown>(
  method: string,
  params: RustCallbackParams = {},
): Promise<TResult> {
  const callbackId = crypto.randomUUID();

  return new Promise<TResult>((resolve, reject) => {
    const timeout = setTimeout(() => {
      pendingCallbacks.delete(callbackId);
      reject(
        createCallbackError(
          method,
          `timed out after ${CALLBACK_TIMEOUT_MS}ms`,
        ),
      );
    }, CALLBACK_TIMEOUT_MS);

    pendingCallbacks.set(callbackId, {
      method,
      timeout,
      resolve(result) {
        resolve(result as TResult);
      },
      reject,
    });

    process.stdout.write(
      `${JSON.stringify({
        jsonrpc: "2.0",
        id: callbackId,
        method: "callback",
        params: {
          callbackId,
          method,
          params,
        },
      })}\n`,
    );
  });
}

export function pendingConfirmation<
  TAction extends string,
  TParams extends RustCallbackParams,
>(
  action: TAction,
  params: TParams,
): PendingConfirmationResult<TAction, TParams> {
  return {
    action,
    params,
    status: "pending_confirmation",
  };
}
