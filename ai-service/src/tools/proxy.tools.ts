import { tool } from "ai";
import { z } from "zod";

import { pendingConfirmation, requestFromRust } from "./rust-rpc.js";

const emptyParameters = z.object({}).strict();

const switchProxyParameters = z
  .object({
    group: z.string().min(1).describe("代理组名称"),
    name: z.string().min(1).describe("目标节点名称"),
  })
  .strict();

const testDelayParameters = z
  .object({
    name: z.string().min(1).describe("节点名称"),
  })
  .strict();

export const proxyTools = {
  list_proxies: tool({
    description: "列出当前所有代理组与节点",
    inputSchema: emptyParameters,
    execute: async () => requestFromRust("get_proxies"),
  }),

  switch_proxy: tool({
    description: "切换代理组的当前节点，返回待确认操作",
    inputSchema: switchProxyParameters,
    execute: async (params) => pendingConfirmation("switch_proxy", params),
  }),

  test_delay: tool({
    description: "测试指定节点延迟",
    inputSchema: testDelayParameters,
    execute: async (params) => requestFromRust("test_delay", params),
  }),
};
