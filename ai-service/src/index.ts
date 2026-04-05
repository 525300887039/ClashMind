import { createInterface } from "node:readline";

import { handleRpcRequest } from "./rpc-handler.js";

const SERVICE_VERSION = "0.1.0";

function writeMessage(message: unknown): void {
  process.stdout.write(`${JSON.stringify(message)}\n`);
}

function writeParseError(): void {
  writeMessage({
    jsonrpc: "2.0",
    id: null,
    error: {
      code: -32700,
      message: "Parse error",
    },
  });
}

const rl = createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
  terminal: false,
});

rl.on("line", async (line: string) => {
  const trimmed = line.trim();
  if (!trimmed) {
    return;
  }

  let request: unknown;
  try {
    request = JSON.parse(trimmed);
  } catch {
    writeParseError();
    return;
  }

  try {
    const response = await handleRpcRequest(request);
    if (response !== null) {
      writeMessage(response);
    }
  } catch (error) {
    writeMessage({
      jsonrpc: "2.0",
      id: null,
      error: {
        code: -32603,
        message: error instanceof Error ? error.message : "Internal error",
      },
    });
  }
});

rl.on("close", () => {
  process.exit(0);
});

writeMessage({
  jsonrpc: "2.0",
  method: "ready",
  params: {
    version: SERVICE_VERSION,
  },
});
