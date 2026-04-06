import { tool } from "ai";
import { z } from "zod";

import { pendingConfirmation, requestFromRust } from "./rust-rpc.js";

const emptyParameters = z.object({}).strict();

const proxyTypeSchema = z
  .enum(["ss", "vmess", "vless", "trojan", "hysteria2", "tuic"])
  .describe("代理协议类型");

const proxyGroupTypeSchema = z
  .enum(["select", "url-test", "fallback", "load-balance"])
  .describe("代理组类型");

const ruleTypeSchema = z
  .enum([
    "DOMAIN",
    "DOMAIN-SUFFIX",
    "DOMAIN-KEYWORD",
    "IP-CIDR",
    "GEOIP",
    "PROCESS-NAME",
    "MATCH",
  ])
  .describe("路由规则类型");

const addProxyParameters = z
  .object({
    name: z.string().min(1).describe("节点名称"),
    type: proxyTypeSchema,
    server: z.string().min(1).describe("服务器地址"),
    port: z.number().int().min(1).max(65535).describe("端口"),
    settings: z
      .record(z.string(), z.unknown())
      .optional()
      .describe("协议特定配置"),
  })
  .strict();

const removeProxyParameters = z
  .object({
    name: z.string().min(1).describe("要删除的节点名称"),
  })
  .strict();

const addProxyGroupParameters = z
  .object({
    name: z.string().min(1).describe("代理组名称"),
    type: proxyGroupTypeSchema,
    proxies: z.array(z.string().min(1)).optional().describe("节点名称列表"),
    filter: z.string().min(1).optional().describe("正则过滤节点名称"),
    url: z
      .string()
      .min(1)
      .default("http://www.gstatic.com/generate_204")
      .describe("健康检查 URL"),
    interval: z.number().int().positive().default(300).describe("检查间隔秒数"),
  })
  .strict();

const updateProxyGroupParameters = z
  .object({
    name: z.string().min(1).describe("代理组名称"),
    updates: z
      .record(z.string(), z.unknown())
      .describe("需要更新的代理组字段"),
  })
  .strict();

const addRuleParameters = z
  .object({
    type: ruleTypeSchema,
    value: z.string().min(1).optional().describe("规则值，MATCH 类型可省略"),
    policy: z.string().min(1).describe("目标策略或代理组"),
    position: z
      .enum(["prepend", "append"])
      .default("prepend")
      .describe("插入位置"),
  })
  .strict();

const removeRuleParameters = z
  .object({
    pattern: z.string().min(1).describe("规则匹配模式"),
  })
  .strict();

const updateDnsParameters = z
  .object({
    nameserver: z.array(z.string().min(1)).optional().describe("DNS 服务器列表"),
    fallback: z.array(z.string().min(1)).optional().describe("备用 DNS 服务器"),
    fakeIpFilter: z
      .array(z.string().min(1))
      .optional()
      .describe("Fake-IP 过滤域名"),
    enhancedMode: z
      .enum(["fake-ip", "redir-host"])
      .optional()
      .describe("增强 DNS 模式"),
  })
  .strict();

const setModeParameters = z
  .object({
    mode: z.enum(["rule", "global", "direct"]).describe("运行模式"),
  })
  .strict();

export const configTools = {
  get_current_config: tool({
    description: "获取当前 Mihomo 运行时配置快照",
    inputSchema: emptyParameters,
    execute: async () => requestFromRust("get_config"),
  }),

  add_proxy: tool({
    description: "添加代理节点到 Mihomo 配置，返回待确认操作",
    inputSchema: addProxyParameters,
    execute: async (params) => pendingConfirmation("add_proxy", params),
  }),

  remove_proxy: tool({
    description: "删除指定代理节点，返回待确认操作",
    inputSchema: removeProxyParameters,
    execute: async (params) => pendingConfirmation("remove_proxy", params),
  }),

  add_proxy_group: tool({
    description: "添加代理组到 Mihomo 配置，返回待确认操作",
    inputSchema: addProxyGroupParameters,
    execute: async (params) => pendingConfirmation("add_proxy_group", params),
  }),

  update_proxy_group: tool({
    description: "更新代理组配置，返回待确认操作",
    inputSchema: updateProxyGroupParameters,
    execute: async (params) => pendingConfirmation("update_proxy_group", params),
  }),

  add_rule: tool({
    description: "添加路由规则，返回待确认操作",
    inputSchema: addRuleParameters,
    execute: async (params) => pendingConfirmation("add_rule", params),
  }),

  remove_rule: tool({
    description: "删除路由规则，返回待确认操作",
    inputSchema: removeRuleParameters,
    execute: async (params) => pendingConfirmation("remove_rule", params),
  }),

  update_dns: tool({
    description: "更新 DNS 配置，返回待确认操作",
    inputSchema: updateDnsParameters,
    execute: async (params) => pendingConfirmation("update_dns", params),
  }),

  set_mode: tool({
    description: "切换 Mihomo 运行模式，返回待确认操作",
    inputSchema: setModeParameters,
    execute: async (params) => pendingConfirmation("set_mode", params),
  }),
};
