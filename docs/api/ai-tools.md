# AI 工具参考

本文档整理 `ai-service/src/tools/*.ts` 中实际注册的 18 个工具，覆盖配置修改、代理操作、统计查询和运行诊断。

:::tip 阅读约定
- 工具来源仅包括 `config.tools.ts`、`proxy.tools.ts`、`stats.tools.ts`、`diagnosis.tools.ts`。
- `index.ts` 仅负责合并 `allTools`，`rust-rpc.ts` 仅提供 `requestFromRust(...)` 与 `pendingConfirmation(...)` 等通用辅助，不单独算作工具。
- 参数表严格按 `inputSchema` 的 Zod 定义记录：类型、可选性、枚举值、默认值、范围约束和 `.describe(...)`。
- 返回值按 `execute` 的真实路径记录：要么是 callback JSON，要么是 `pending_confirmation`，要么是 `validation_error`。
- 下文示例默认已从 `ai-service/src/tools/*.ts` 所在上下文导入 `tool`、`z`、`requestFromRust` 等符号。
:::

## 模块总览

| 模块 | 源文件 | 工具数 | 说明 |
| --- | --- | ---: | --- |
| 配置工具 | `ai-service/src/tools/config.tools.ts` | 9 | 获取当前配置、生成待确认的配置修改动作 |
| 代理工具 | `ai-service/src/tools/proxy.tools.ts` | 3 | 列出代理、切换代理、测试节点延迟 |
| 统计工具 | `ai-service/src/tools/stats.tools.ts` | 3 | 获取流量摘要、域名排行、趋势数据 |
| 诊断工具 | `ai-service/src/tools/diagnosis.tools.ts` | 3 | 健康检查、近期问题摘要、规则命中统计 |

## 通用返回结构

### PendingConfirmationResult

由 `pendingConfirmation(action, params)` 生成，结构固定如下：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `action` | `string` | 当前待确认动作名，例如 `add_proxy` |
| `params` | `Record<string, unknown>` | 原始输入参数；字段与当前工具输入 schema 一致 |
| `status` | `"pending_confirmation"` | 固定字面量 |

### ValidationErrorResult

由 `createValidationErrorResult(validation)` 生成，结构固定如下：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `action` | `"validation_error"` | 固定字面量 |
| `errors` | `ValidationError[]` | 校验错误列表 |
| `warnings` | `ValidationWarning[]` | 校验警告列表 |

`ValidationError` 与 `ValidationWarning` 的字段相同：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `path` | `string` | 出错字段路径 |
| `message` | `string` | 说明消息 |

### SanitizedConfigResponse

`get_current_config` 返回的结构如下：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `source` | `"mihomo_runtime" \| "config_file"` | 实际读取来源 |
| `sanitized` | `boolean` | 当前实现始终为 `true` |
| `config` | `Record<string, unknown>` | 已脱敏配置对象 |

### DelayTestResult

`test_delay` 返回的结构如下：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `name` | `string` | 测试节点名称 |
| `url` | `string` | 实际测试 URL，当前固定为 `http://www.gstatic.com/generate_204` |
| `timeout` | `number` | 实际超时毫秒数，当前固定为 `5000` |
| `delay` | `number` | 延迟测试结果，单位毫秒 |

### TrafficSummaryResult

`get_traffic_summary` 返回：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `days` | `number` | 实际统计天数 |
| `summary` | `StatsOverview` | 结构见 [Tauri IPC 文档](./tauri-ipc.md#statsoverview) |

### TopDomainsResult

`get_top_domains` 返回：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `days` | `number` | 实际统计天数 |
| `limit` | `number` | 实际返回数量 |
| `domains` | `DomainStat[]` | 结构见 [Tauri IPC 文档](./tauri-ipc.md#domainstat) |

### TrafficTrendResult

`get_traffic_trend` 返回：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `granularity` | `"hourly" \| "daily"` | 实际粒度 |
| `days` | `number` | 实际统计天数 |
| `start` | `string` | 查询窗口起始时间 |
| `end` | `string` | 查询窗口结束时间 |
| `points` | `TrafficPoint[]` | 结构见 [Tauri IPC 文档](./tauri-ipc.md#trafficpoint) |

### ConnectivitySummary

`check_connectivity` 返回：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `reachable` | `boolean` | `version` 和 `proxies` 两个 API 是否都可用 |
| `apiAddress` | `string` | 当前 Mihomo API 地址 |
| `collectorRunning` | `boolean` | 采集器是否运行中 |
| `activeConnections` | `number` | 当前活动连接数 |
| `proxyCount` | `number` | 代理项数量 |
| `selectedGroups` | `SelectedProxyGroup[]` | 当前有选中节点的代理组 |
| `version` | `unknown \| null` | Mihomo 版本 JSON；失败时为 `null` |
| `issues` | `string[]` | 诊断问题列表 |

`SelectedProxyGroup` 字段如下：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `group` | `string` | 代理组名称 |
| `current` | `string` | 当前选中的节点名 |

### RecentErrorsSummary

`get_recent_errors` 返回：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `windowMinutes` | `number` | 实际查询窗口分钟数 |
| `issues` | `RuntimeIssue[]` | 当前运行时健康问题摘要 |
| `note` | `string` | 附加说明 |

`RuntimeIssue` 字段如下：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `source` | `string` | 问题来源，例如 `mihomo_api`、`collector`、`runtime` |
| `severity` | `string` | 严重级别，例如 `error`、`warning`、`info` |
| `message` | `string` | 诊断消息 |

### RuleMatchStatsSummary

`get_rule_match_stats` 返回：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `days` | `number` | 实际统计天数 |
| `totalHits` | `number` | 总规则命中数 |
| `matchHits` | `number` | `MATCH` 规则命中数 |
| `matchRate` | `number` | `matchHits / totalHits`，范围 `0..=1` |
| `rules` | `RuleStat[]` | 结构见 [Tauri IPC 文档](./tauri-ipc.md#rulestat) |

## 配置工具

源文件：`ai-service/src/tools/config.tools.ts`

::: details get_current_config
**名称** `get_current_config`

**参数**

无。`inputSchema` 为 `z.object({}).strict()`。

**返回值**

返回 `SanitizedConfigResponse`。

**说明**

通过 `requestFromRust("get_config")` 获取当前 Mihomo 配置快照。Rust 会优先读取运行时配置，失败时回退到活动配置文件，并对敏感字段做脱敏。

**调用示例**

::: code-group
```ts [Definition]
get_current_config: tool({
  description: "获取当前 Mihomo 运行时配置快照",
  inputSchema: z.object({}).strict(),
  execute: async () => requestFromRust("get_config"),
})
```

```ts [TypeScript]
const input = {};
// execute -> requestFromRust("get_config")
```
:::
:::

::: details add_proxy
**名称** `add_proxy`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `name` | `string` | 是 | `min(1)` | 节点名称 |
| `type` | `"ss" \| "vmess" \| "vless" \| "trojan" \| "hysteria2" \| "tuic"` | 是 | 枚举 | 代理协议类型 |
| `server` | `string` | 是 | `min(1)` | 服务器地址 |
| `port` | `number` | 是 | `int`，`1..=65535` | 端口 |
| `settings` | `Record<string, unknown>` | 否 | 无默认值 | 协议特定配置 |

**返回值**

校验成功时返回 `PendingConfirmationResult`：

```ts
{
  action: "add_proxy",
  params: AddProxyParameters,
  status: "pending_confirmation"
}
```

校验失败时返回 `ValidationErrorResult`。

**说明**

先把输入拼成代理片段，再调用 `validateProxyFragment(...)` 做 schema 校验；通过后只返回“待确认操作”，不会直接修改配置。

**调用示例**

::: code-group
```ts [Definition]
add_proxy: tool({
  description: "添加代理节点到 Mihomo 配置，返回待确认操作",
  inputSchema: addProxyParameters,
  execute: async (params) =>
    finalizeValidatedChange(
      "add_proxy",
      params,
      validateProxyFragment(buildProxyFragment(params)),
    ),
})
```

```ts [TypeScript]
const input = {
  name: "HK-01",
  type: "ss",
  server: "hk.example.com",
  port: 443,
};
// 典型返回：{ action: "add_proxy", params: input, status: "pending_confirmation" }
```
:::
:::

::: details remove_proxy
**名称** `remove_proxy`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `name` | `string` | 是 | `min(1)` | 要删除的节点名称 |

**返回值**

返回 `PendingConfirmationResult`：

```ts
{
  action: "remove_proxy",
  params: RemoveProxyParameters,
  status: "pending_confirmation"
}
```

**说明**

不做额外 schema 校验，直接返回待确认动作；真正删除动作由后续 diff 预览和 Rust 端确认链路完成。

**调用示例**

::: code-group
```ts [Definition]
remove_proxy: tool({
  description: "删除指定代理节点，返回待确认操作",
  inputSchema: removeProxyParameters,
  execute: async (params) => pendingConfirmation("remove_proxy", params),
})
```

```ts [TypeScript]
const input = { name: "HK-01" };
// 返回：{ action: "remove_proxy", params: input, status: "pending_confirmation" }
```
:::
:::

::: details add_proxy_group
**名称** `add_proxy_group`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `name` | `string` | 是 | `min(1)` | 代理组名称 |
| `type` | `"select" \| "url-test" \| "fallback" \| "load-balance"` | 是 | 枚举 | 代理组类型 |
| `proxies` | `string[]` | 否 | 元素 `min(1)` | 节点名称列表 |
| `filter` | `string` | 否 | `min(1)` | 正则过滤节点名称 |
| `url` | `string` | 否 | 默认 `"http://www.gstatic.com/generate_204"` | 健康检查 URL |
| `interval` | `number` | 否 | `int`、`> 0`、默认 `300` | 检查间隔秒数 |

**返回值**

校验成功时返回 `PendingConfirmationResult`：

```ts
{
  action: "add_proxy_group",
  params: AddProxyGroupParameters,
  status: "pending_confirmation"
}
```

校验失败时返回 `ValidationErrorResult`。

**说明**

内部调用 `validateProxyGroupFragment(params)`。虽然 `proxies` 和 `filter` 都是可选字段，但如果两者都不传，校验阶段会返回错误。

**调用示例**

::: code-group
```ts [Definition]
add_proxy_group: tool({
  description: "添加代理组到 Mihomo 配置，返回待确认操作",
  inputSchema: addProxyGroupParameters,
  execute: async (params) =>
    finalizeValidatedChange(
      "add_proxy_group",
      params,
      validateProxyGroupFragment(params),
    ),
})
```

```ts [TypeScript]
const input = {
  name: "Auto",
  type: "url-test",
  proxies: ["HK-01", "JP-01"],
  url: "http://www.gstatic.com/generate_204",
  interval: 300,
};
```
:::
:::

::: details update_proxy_group
**名称** `update_proxy_group`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `name` | `string` | 是 | `min(1)` | 代理组名称 |
| `updates` | `Record<string, unknown>` | 是 | 无默认值 | 需要更新的代理组字段 |

**返回值**

校验成功时返回 `PendingConfirmationResult`，失败时返回 `ValidationErrorResult`。

**说明**

内部调用 `validateProxyGroupUpdateFragment(params.updates)` 校验更新片段。

**调用示例**

::: code-group
```ts [Definition]
update_proxy_group: tool({
  description: "更新代理组配置，返回待确认操作",
  inputSchema: updateProxyGroupParameters,
  execute: async (params) =>
    finalizeValidatedChange(
      "update_proxy_group",
      params,
      validateProxyGroupUpdateFragment(params.updates),
    ),
})
```

```ts [TypeScript]
const input = {
  name: "Auto",
  updates: {
    interval: 600,
  },
};
```
:::
:::

::: details add_rule
**名称** `add_rule`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `type` | `"DOMAIN" \| "DOMAIN-SUFFIX" \| "DOMAIN-KEYWORD" \| "IP-CIDR" \| "GEOIP" \| "PROCESS-NAME" \| "MATCH"` | 是 | 枚举 | 路由规则类型 |
| `value` | `string` | 否 | `min(1)` | 规则值，`MATCH` 类型可省略 |
| `policy` | `string` | 是 | `min(1)` | 目标策略或代理组 |
| `position` | `"prepend" \| "append"` | 否 | 默认 `"prepend"` | 插入位置 |

**返回值**

校验成功时返回 `PendingConfirmationResult`，失败时返回 `ValidationErrorResult`。

**说明**

内部会先做一层规则参数校验：当 `type !== "MATCH"` 时，`value` 不能为空；随后再把规则字符串交给 `validateRuleFragment(...)`。

**调用示例**

::: code-group
```ts [Definition]
add_rule: tool({
  description: "添加路由规则，返回待确认操作",
  inputSchema: addRuleParameters,
  execute: async (params) =>
    finalizeValidatedChange("add_rule", params, validateRuleParameters(params)),
})
```

```ts [TypeScript]
const input = {
  type: "DOMAIN-SUFFIX",
  value: "example.com",
  policy: "Proxy",
  position: "prepend",
};
```
:::
:::

::: details remove_rule
**名称** `remove_rule`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `pattern` | `string` | 是 | `min(1)` | 规则匹配模式 |

**返回值**

返回 `PendingConfirmationResult`：

```ts
{
  action: "remove_rule",
  params: RemoveRuleParameters,
  status: "pending_confirmation"
}
```

**说明**

只生成“删除规则”的待确认动作，真正按模式查找并删除规则发生在 diff 生成阶段。

**调用示例**

::: code-group
```ts [Definition]
remove_rule: tool({
  description: "删除路由规则，返回待确认操作",
  inputSchema: removeRuleParameters,
  execute: async (params) => pendingConfirmation("remove_rule", params),
})
```

```ts [TypeScript]
const input = { pattern: "example.com" };
```
:::
:::

::: details update_dns
**名称** `update_dns`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `nameserver` | `string[]` | 否 | 元素 `min(1)` | DNS 服务器列表 |
| `fallback` | `string[]` | 否 | 元素 `min(1)` | 备用 DNS 服务器 |
| `fakeIpFilter` | `string[]` | 否 | 元素 `min(1)` | Fake-IP 过滤域名 |
| `enhancedMode` | `"fake-ip" \| "redir-host"` | 否 | 枚举 | 增强 DNS 模式 |

**返回值**

校验成功时返回 `PendingConfirmationResult`，失败时返回 `ValidationErrorResult`。

**说明**

先把 camelCase 输入字段转换成 Mihomo 配置片段中的 `fake-ip-filter` / `enhanced-mode` 键，再交给 `validateDnsFragment(...)` 校验。

**调用示例**

::: code-group
```ts [Definition]
update_dns: tool({
  description: "更新 DNS 配置，返回待确认操作",
  inputSchema: updateDnsParameters,
  execute: async (params) =>
    finalizeValidatedChange(
      "update_dns",
      params,
      validateDnsFragment(buildDnsFragment(params)),
    ),
})
```

```ts [TypeScript]
const input = {
  nameserver: ["1.1.1.1", "8.8.8.8"],
  enhancedMode: "fake-ip",
};
```
:::
:::

::: details set_mode
**名称** `set_mode`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `mode` | `"rule" \| "global" \| "direct"` | 是 | 枚举 | 运行模式 |

**返回值**

校验成功时返回 `PendingConfirmationResult`，失败时返回 `ValidationErrorResult`。

**说明**

通过 `validateModeFragment(params.mode)` 校验模式值是否有效。

**调用示例**

::: code-group
```ts [Definition]
set_mode: tool({
  description: "切换 Mihomo 运行模式，返回待确认操作",
  inputSchema: setModeParameters,
  execute: async (params) =>
    finalizeValidatedChange("set_mode", params, validateModeFragment(params.mode)),
})
```

```ts [TypeScript]
const input = { mode: "rule" };
```
:::
:::

## 代理工具

源文件：`ai-service/src/tools/proxy.tools.ts`

::: details list_proxies
**名称** `list_proxies`

**参数**

无。`inputSchema` 为 `z.object({}).strict()`。

**返回值**

返回 `requestFromRust("get_proxies")` 的原始 JSON 结果，即 Mihomo `/proxies` 响应；当前工具层没有额外包裹返回结构。

**说明**

列出当前所有代理组与节点。

**调用示例**

::: code-group
```ts [Definition]
list_proxies: tool({
  description: "列出当前所有代理组与节点",
  inputSchema: z.object({}).strict(),
  execute: async () => requestFromRust("get_proxies"),
})
```

```ts [TypeScript]
const input = {};
// execute -> requestFromRust("get_proxies")
```
:::
:::

::: details switch_proxy
**名称** `switch_proxy`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `group` | `string` | 是 | `min(1)` | 代理组名称 |
| `name` | `string` | 是 | `min(1)` | 目标节点名称 |

**返回值**

返回 `PendingConfirmationResult`：

```ts
{
  action: "switch_proxy",
  params: SwitchProxyParameters,
  status: "pending_confirmation"
}
```

**说明**

和配置工具不同，这里不直接调用 Rust 切换节点，而是先要求用户确认。

**调用示例**

::: code-group
```ts [Definition]
switch_proxy: tool({
  description: "切换代理组的当前节点，返回待确认操作",
  inputSchema: switchProxyParameters,
  execute: async (params) => pendingConfirmation("switch_proxy", params),
})
```

```ts [TypeScript]
const input = {
  group: "Proxy",
  name: "HK-01",
};
```
:::
:::

::: details test_delay
**名称** `test_delay`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `name` | `string` | 是 | `min(1)` | 节点名称 |

**返回值**

返回 `DelayTestResult`。

**说明**

工具输入只有节点名；真正的 URL 与 timeout 由 Rust callback 固定为 `http://www.gstatic.com/generate_204` 和 `5000ms`。

**调用示例**

::: code-group
```ts [Definition]
test_delay: tool({
  description: "测试指定节点延迟",
  inputSchema: testDelayParameters,
  execute: async (params) => requestFromRust("test_delay", params),
})
```

```ts [TypeScript]
const input = { name: "HK-01" };
// execute -> requestFromRust("test_delay", input)
```
:::
:::

## 统计工具

源文件：`ai-service/src/tools/stats.tools.ts`

::: details get_traffic_summary
**名称** `get_traffic_summary`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `days` | `number` | 否 | `int`、`1..=365`、默认 `7` | 统计天数 |

**返回值**

返回 `TrafficSummaryResult`。

**说明**

内部调用 `requestFromRust("get_stats_overview", params)`。Rust callback 会再次对 `days` 做边界裁剪校验。

**调用示例**

::: code-group
```ts [Definition]
get_traffic_summary: tool({
  description: "获取指定时间窗口的流量摘要",
  inputSchema: trafficSummaryParameters,
  execute: async (params) => requestFromRust("get_stats_overview", params),
})
```

```ts [TypeScript]
const input = { days: 7 };
// execute -> requestFromRust("get_stats_overview", input)
```
:::
:::

::: details get_top_domains
**名称** `get_top_domains`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `days` | `number` | 否 | `int`、`1..=365`、默认 `7` | 统计天数 |
| `limit` | `number` | 否 | `int`、`1..=100`、默认 `20` | 返回数量 |

**返回值**

返回 `TopDomainsResult`。

**说明**

内部调用 `requestFromRust("get_domain_stats", params)`，Rust callback 会把最终 `days` 与 `limit` 原样回显到结果中。

**调用示例**

::: code-group
```ts [Definition]
get_top_domains: tool({
  description: "获取流量排名靠前的域名列表",
  inputSchema: topDomainsParameters,
  execute: async (params) => requestFromRust("get_domain_stats", params),
})
```

```ts [TypeScript]
const input = {
  days: 7,
  limit: 20,
};
```
:::
:::

::: details get_traffic_trend
**名称** `get_traffic_trend`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `granularity` | `"hourly" \| "daily"` | 是 | 枚举 | 时间粒度 |
| `days` | `number` | 否 | `int`、`1..=365`、默认 `7` | 统计天数 |

**返回值**

返回 `TrafficTrendResult`。

**说明**

内部调用 `requestFromRust("get_traffic_trend", params)`。Rust callback 会根据粒度自动构造 `start` / `end` 时间窗口，并返回对应粒度的 `points`。

**调用示例**

::: code-group
```ts [Definition]
get_traffic_trend: tool({
  description: "获取流量趋势数据",
  inputSchema: trafficTrendParameters,
  execute: async (params) => requestFromRust("get_traffic_trend", params),
})
```

```ts [TypeScript]
const input = {
  granularity: "daily",
  days: 7,
};
```
:::
:::

## 诊断工具

源文件：`ai-service/src/tools/diagnosis.tools.ts`

::: details check_connectivity
**名称** `check_connectivity`

**参数**

无。`inputSchema` 为 `z.object({}).strict()`。

**返回值**

返回 `ConnectivitySummary`。

**说明**

检查 Mihomo `version` / `proxies` API 是否可用、采集器是否运行、当前活动连接数，以及各代理组的当前选中节点。

**调用示例**

::: code-group
```ts [Definition]
check_connectivity: tool({
  description: "检查当前 Mihomo API、代理和采集器的连通性摘要",
  inputSchema: z.object({}).strict(),
  execute: async () => requestFromRust("check_connectivity"),
})
```

```ts [TypeScript]
const input = {};
// execute -> requestFromRust("check_connectivity")
```
:::
:::

::: details get_recent_errors
**名称** `get_recent_errors`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `minutes` | `number` | 否 | `int`、`1..=1440`、默认 `30` | 最近 N 分钟 |

**返回值**

返回 `RecentErrorsSummary`。

**说明**

当前版本没有独立的持久化错误日志；返回值基于即时运行时健康检查生成，因此更接近“近期健康问题摘要”而不是严格的错误日志查询。

**调用示例**

::: code-group
```ts [Definition]
get_recent_errors: tool({
  description: "获取最近的运行时错误或健康问题摘要",
  inputSchema: recentErrorsParameters,
  execute: async (params) => requestFromRust("get_recent_errors", params),
})
```

```ts [TypeScript]
const input = { minutes: 30 };
```
:::
:::

::: details get_rule_match_stats
**名称** `get_rule_match_stats`

**参数**

| 参数 | 类型 | 必填 | 约束 / 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `days` | `number` | 否 | `int`、`1..=30`、默认 `1` | 统计天数 |

**返回值**

返回 `RuleMatchStatsSummary`。

**说明**

工具名为 `get_rule_match_stats`，但底层实际调用 `requestFromRust("get_rule_stats", params)`。Rust callback 会固定使用 `DEFAULT_RULE_STATS_LIMIT = 20`，并额外计算 `totalHits`、`matchHits` 与 `matchRate`。

**调用示例**

::: code-group
```ts [Definition]
get_rule_match_stats: tool({
  description: "获取规则匹配统计，便于判断 MATCH 兜底比例",
  inputSchema: ruleMatchStatsParameters,
  execute: async (params) => requestFromRust("get_rule_stats", params),
})
```

```ts [TypeScript]
const input = { days: 1 };
```
:::
:::
