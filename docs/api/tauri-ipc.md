# Tauri IPC 命令参考

本文档整理 `src-tauri/src/cmd/*.rs` 中所有 `#[tauri::command]`，共 52 个命令，按模块分组说明。

:::tip 阅读约定
- 参数表中的 `Rust 类型` 保留源码签名原样。
- `AppHandle` 与 `tauri::State<'_, ...>` 属于 Tauri 注入上下文，不需要前端手动传入；文档仍会列出以保证和源码一致。
- TypeScript 示例统一对齐项目现有 `src/lib/tauri-api.ts` 的 `invoke(...)` 用法，默认已执行 `import { invoke } from "@tauri-apps/api/core";`。
- 返回 `serde_json::Value` 的命令表示“直接透传 JSON”；如果源码没有进一步约束结构，文档不会补造字段。
:::

## 模块总览

| 模块 | 源文件 | 命令数 | 主要职责 |
| --- | --- | ---: | --- |
| AI 服务 | `src-tauri/src/cmd/ai.rs` | 16 | AI sidecar 生命周期、聊天、报告、快照、对话持久化 |
| Mihomo Sidecar | `src-tauri/src/cmd/sidecar.rs` | 6 | Mihomo 进程启动、重启、配置目录初始化 |
| 代理 | `src-tauri/src/cmd/proxy.rs` | 5 | 代理组切换、延迟测试、规则查询 |
| 配置 | `src-tauri/src/cmd/config.rs` | 5 | 配置文件读写与 Mihomo 运行时配置读写 |
| 系统 | `src-tauri/src/cmd/system.rs` | 7 | 版本、连接管理、系统代理、Mihomo 客户端地址更新 |
| 采集器 | `src-tauri/src/cmd/collector.rs` | 5 | WebSocket 采集器生命周期与实时连接缓存 |
| 统计 | `src-tauri/src/cmd/stats.rs` | 8 | 数据库清理、统计总览、流量、域名、规则、Geo 统计 |

## AI 服务命令

源文件：`src-tauri/src/cmd/ai.rs`

::: details start_ai_service
**名称** `start_ai_service`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app` | `AppHandle` | Tauri 注入 | 当前应用句柄 |
| `state` | `tauri::State<'_, AiSidecarState>` | Tauri 注入 | AI sidecar 生命周期状态 |

**返回值**

成功时返回 `()`；失败时返回 `AiSidecarError`。

**说明**

启动 AI sidecar 进程。

**调用示例**

::: code-group
```rust [Rust]
pub async fn start_ai_service(
    app: AppHandle,
    state: tauri::State<'_, AiSidecarState>,
) -> Result<(), AiSidecarError>
```

```ts [TypeScript]
await invoke("start_ai_service");
```
:::
:::

::: details stop_ai_service
**名称** `stop_ai_service`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app` | `AppHandle` | Tauri 注入 | 当前应用句柄 |
| `state` | `tauri::State<'_, AiSidecarState>` | Tauri 注入 | AI sidecar 生命周期状态 |

**返回值**

成功时返回 `()`；失败时返回 `AiSidecarError`。

**说明**

停止 AI sidecar 进程。

**调用示例**

::: code-group
```rust [Rust]
pub fn stop_ai_service(
    app: AppHandle,
    state: tauri::State<'_, AiSidecarState>,
) -> Result<(), AiSidecarError>
```

```ts [TypeScript]
await invoke("stop_ai_service");
```
:::
:::

::: details get_ai_status
**名称** `get_ai_status`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, AiSidecarState>` | Tauri 注入 | AI sidecar 生命周期状态 |

**返回值**

成功时返回 `bool`，表示 sidecar 是否运行；失败时返回 `AiSidecarError`。

**说明**

查询 AI sidecar 当前是否已启动。

**调用示例**

::: code-group
```rust [Rust]
pub fn get_ai_status(
    state: tauri::State<'_, AiSidecarState>,
) -> Result<bool, AiSidecarError>
```

```ts [TypeScript]
const running = await invoke<boolean>("get_ai_status");
```
:::
:::

::: details get_ai_settings
**名称** `get_ai_settings`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app` | `AppHandle` | Tauri 注入 | 用于解析应用数据目录并读取 `ai-settings.json` |

**返回值**

成功时返回 `AiSettings`；失败时返回 `AiSettingsError`。

**说明**

读取 AI 设置文件。字段见文末 [通用类型](#通用类型) 中的 `AiSettings`。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_ai_settings(app: AppHandle) -> Result<AiSettings, AiSettingsError>
```

```ts [TypeScript]
const settings = await invoke<AiSettings>("get_ai_settings");
```
:::
:::

::: details set_ai_settings
**名称** `set_ai_settings`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app` | `AppHandle` | Tauri 注入 | 用于解析应用数据目录并写入设置文件 |
| `settings` | `AiSettings` | 前端传入 | AI 设置对象，写入前会执行 `normalized()` |

**返回值**

成功时返回 `()`；失败时返回 `AiSettingsError`。

**说明**

保存 AI 设置到 `ai-settings.json`。空白字符串会被 `trim()`，`temperature` 会被限制到 `0.0..=1.0`，`max_tokens` 至少为 `1`。

**调用示例**

::: code-group
```rust [Rust]
pub async fn set_ai_settings(
    app: AppHandle,
    settings: AiSettings,
) -> Result<(), AiSettingsError>
```

```ts [TypeScript]
await invoke("set_ai_settings", {
  settings: {
    provider: "openai",
    model: "gpt-4o-mini",
    apiKey: "sk-...",
    baseUrl: "",
    temperature: 0.3,
    maxTokens: 4096,
    autoStart: false,
  },
});
```
:::
:::

::: details ai_ping
**名称** `ai_ping`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, AiSidecarState>` | Tauri 注入 | AI sidecar 生命周期状态 |

**返回值**

成功时返回 `serde_json::Value`，当前实现结构为 `{ pong: true, timestamp: number }`；失败时返回 `AiSidecarError`。

**说明**

向 AI sidecar 发送 `ping` JSON-RPC 请求。

**调用示例**

::: code-group
```rust [Rust]
pub async fn ai_ping(
    state: tauri::State<'_, AiSidecarState>,
) -> Result<serde_json::Value, AiSidecarError>
```

```ts [TypeScript]
const ping = await invoke<{ pong: boolean; timestamp: number }>("ai_ping");
```
:::
:::

::: details test_ai_connection
**名称** `test_ai_connection`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app` | `AppHandle` | Tauri 注入 | 用于在 sidecar 未启动时自动启动 |
| `state` | `tauri::State<'_, AiSidecarState>` | Tauri 注入 | AI sidecar 生命周期状态 |
| `settings` | `AiSettings` | 前端传入 | 连通性测试使用的 Provider 配置 |

**返回值**

成功时返回 `AiConnectionTestResult`；失败时返回 `AiSettingsError`。

**说明**

测试当前 Provider 是否可连通。调用前会先校验 `model`、`apiKey`、`baseUrl` 是否满足所选 Provider 要求。

**调用示例**

::: code-group
```rust [Rust]
pub async fn test_ai_connection(
    app: AppHandle,
    state: tauri::State<'_, AiSidecarState>,
    settings: AiSettings,
) -> Result<AiConnectionTestResult, AiSettingsError>
```

```ts [TypeScript]
const result = await invoke<AiConnectionTestResult>("test_ai_connection", {
  settings,
});
```
:::
:::

::: details fetch_ai_models
**名称** `fetch_ai_models`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app` | `AppHandle` | Tauri 注入 | 用于在 sidecar 未启动时自动启动 |
| `state` | `tauri::State<'_, AiSidecarState>` | Tauri 注入 | AI sidecar 生命周期状态 |
| `settings` | `AiSettings` | 前端传入 | 模型列表查询使用的 Provider 配置 |

**返回值**

成功时返回 `AiModelCatalog`；失败时返回 `AiSettingsError`。

**说明**

向 AI sidecar 请求模型列表。返回值中的 `source` 可能是 `remote`、`fallback` 或 `empty`。

**调用示例**

::: code-group
```rust [Rust]
pub async fn fetch_ai_models(
    app: AppHandle,
    state: tauri::State<'_, AiSidecarState>,
    settings: AiSettings,
) -> Result<AiModelCatalog, AiSettingsError>
```

```ts [TypeScript]
const catalog = await invoke<AiModelCatalog>("fetch_ai_models", {
  settings,
});
```
:::
:::

::: details ai_chat
**名称** `ai_chat`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, AiSidecarState>` | Tauri 注入 | AI sidecar 生命周期状态 |
| `params` | `AiChatParams` | 前端传入 | 聊天消息、上下文和 Provider 设置 |

**返回值**

成功时返回 `()`；失败时返回 `AiSidecarError`。

**说明**

发起流式聊天请求。`messages` 不能为空，且 `params.settings.model` 不能为空。真正的文本和工具事件通过流式事件通道返回，而不是命令同步返回值。

**调用示例**

::: code-group
```rust [Rust]
pub fn ai_chat(
    state: tauri::State<'_, AiSidecarState>,
    params: AiChatParams,
) -> Result<(), AiSidecarError>
```

```ts [TypeScript]
await invoke("ai_chat", {
  params: {
    messages: [{ role: "user", content: "请分析最近一周流量" }],
    context: {
      availableProxies: ["DIRECT", "Proxy"],
    },
    settings: {
      provider: "openai",
      model: "gpt-4o-mini",
      apiKey: "sk-...",
    },
  },
});
```
:::
:::

::: details ai_generate_report
**名称** `ai_generate_report`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app` | `AppHandle` | Tauri 注入 | 用于在 sidecar 未启动时自动启动 |
| `state` | `tauri::State<'_, AiSidecarState>` | Tauri 注入 | AI sidecar 生命周期状态 |
| `report_type` | `ReportType` | 前端传入 | 报告类型，枚举值见文末 |
| `date` | `Option<String>` | 前端传入 | 报告结束日期，格式 `YYYY-MM-DD`；可选 |
| `settings` | `AiProviderSettings` | 前端传入 | 报告生成使用的 Provider 配置 |

**返回值**

成功时返回 `ReportResult`；失败时返回 `AiReportError`。

**说明**

生成日报或周报。内部会先通过回调拿到统计数据，再调用 AI sidecar 的 `generate_report` 方法。

**调用示例**

::: code-group
```rust [Rust]
pub async fn ai_generate_report(
    app: AppHandle,
    state: tauri::State<'_, AiSidecarState>,
    report_type: ReportType,
    date: Option<String>,
    settings: AiProviderSettings,
) -> Result<ReportResult, AiReportError>
```

```ts [TypeScript]
const report = await invoke<ReportResult>("ai_generate_report", {
  reportType: "weekly",
  date: "2026-04-08",
  settings: {
    provider: "openai",
    model: "gpt-4o-mini",
    apiKey: "sk-...",
    temperature: 0.35,
    maxTokens: 4096,
  },
});
```
:::
:::

::: details apply_config_change
**名称** `apply_config_change`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `AppHandle` | Tauri 注入 | 用于访问数据库和应用状态 |
| `geoip_config` | `tauri::State<'_, GeoIpConfigState>` | Tauri 注入 | 当前活动配置目录状态 |
| `mihomo_state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |
| `original_config` | `String` | 前端传入 | 用户确认前看到的原始配置 YAML |
| `modified_config` | `String` | 前端传入 | 待应用的修改后 YAML |

**返回值**

成功时返回 `()`；失败时返回 `AiConfigChangeError`。

**说明**

在真正写文件前会比对当前运行时配置是否仍等于 `original_config`，防止基线过期；随后创建 AI 自动快照、写入文件并触发热重载，若热重载失败会自动回滚。

**调用示例**

::: code-group
```rust [Rust]
pub async fn apply_config_change(
    app_handle: AppHandle,
    geoip_config: tauri::State<'_, GeoIpConfigState>,
    mihomo_state: tauri::State<'_, MihomoState>,
    original_config: String,
    modified_config: String,
) -> Result<(), AiConfigChangeError>
```

```ts [TypeScript]
await invoke("apply_config_change", {
  originalConfig,
  modifiedConfig,
});
```
:::
:::

::: details reject_config_change
**名称** `reject_config_change`

**参数**

无。

**返回值**

成功时返回 `()`；失败时返回 `AiConfigChangeError`。

**说明**

当前实现为显式 no-op，用于确认“用户拒绝本次配置变更”这一动作。

**调用示例**

::: code-group
```rust [Rust]
pub fn reject_config_change() -> Result<(), AiConfigChangeError>
```

```ts [TypeScript]
await invoke("reject_config_change");
```
:::
:::

::: details list_snapshots
**名称** `list_snapshots`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `AppHandle` | Tauri 注入 | 用于访问快照数据库 |
| `limit` | `i32` | 前端传入 | 最大返回条数 |

**返回值**

成功时返回 `Vec<ConfigSnapshot>`；失败时返回 `AiConfigChangeError`。

**说明**

按创建时间倒序返回配置快照列表。

**调用示例**

::: code-group
```rust [Rust]
pub async fn list_snapshots(
    app_handle: AppHandle,
    limit: i32,
) -> Result<Vec<ConfigSnapshot>, AiConfigChangeError>
```

```ts [TypeScript]
const snapshots = await invoke<ConfigSnapshot[]>("list_snapshots", {
  limit: 20,
});
```
:::
:::

::: details create_snapshot
**名称** `create_snapshot`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `AppHandle` | Tauri 注入 | 用于访问快照数据库 |
| `geoip_config` | `tauri::State<'_, GeoIpConfigState>` | Tauri 注入 | 当前活动配置目录状态 |
| `description` | `Option<String>` | 前端传入 | 快照描述；为空时默认 `"手动快照"` |
| `file_path` | `Option<String>` | 前端传入 | 指定快照目标文件；为空时使用当前活动配置文件 |

**返回值**

成功时返回 `i64`（新快照 ID）；失败时返回 `AiConfigChangeError`。

**说明**

读取目标配置文件当前内容并创建手动快照。

**调用示例**

::: code-group
```rust [Rust]
pub async fn create_snapshot(
    app_handle: AppHandle,
    geoip_config: tauri::State<'_, GeoIpConfigState>,
    description: Option<String>,
    file_path: Option<String>,
) -> Result<i64, AiConfigChangeError>
```

```ts [TypeScript]
const snapshotId = await invoke<number>("create_snapshot", {
  description: "调整前备份",
  filePath: "C:/Users/no525/.config/mihomo/config.yaml",
});
```
:::
:::

::: details restore_snapshot
**名称** `restore_snapshot`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `AppHandle` | Tauri 注入 | 用于访问快照数据库 |
| `geoip_config` | `tauri::State<'_, GeoIpConfigState>` | Tauri 注入 | 当前活动配置目录状态 |
| `mihomo_state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |
| `id` | `i64` | 前端传入 | 快照 ID |

**返回值**

成功时返回 `()`；失败时返回 `AiConfigChangeError`。

**说明**

恢复指定快照前会先验证快照文件路径必须和当前活动配置文件一致，并自动创建一份“恢复前备份”快照。

**调用示例**

::: code-group
```rust [Rust]
pub async fn restore_snapshot(
    app_handle: AppHandle,
    geoip_config: tauri::State<'_, GeoIpConfigState>,
    mihomo_state: tauri::State<'_, MihomoState>,
    id: i64,
) -> Result<(), AiConfigChangeError>
```

```ts [TypeScript]
await invoke("restore_snapshot", { id: 42 });
```
:::
:::

::: details save_conversation_message
**名称** `save_conversation_message`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `AppHandle` | Tauri 注入 | 用于访问会话数据库 |
| `params` | `SaveConversationMessageParams` | 前端传入 | 单条会话消息及元数据 |

**返回值**

成功时返回 `i64`（消息 ID）；失败时返回 `AiConfigChangeError`。

**说明**

持久化一条聊天消息，并在写入后触发历史清理。

**调用示例**

::: code-group
```rust [Rust]
pub async fn save_conversation_message(
    app_handle: AppHandle,
    params: SaveConversationMessageParams,
) -> Result<i64, AiConfigChangeError>
```

```ts [TypeScript]
const messageId = await invoke<number>("save_conversation_message", {
  params: {
    role: "assistant",
    content: "建议切换到自动测速代理组。",
    tokensUsed: 284,
    model: "gpt-4o-mini",
  },
});
```
:::
:::

## Mihomo Sidecar 命令

源文件：`src-tauri/src/cmd/sidecar.rs`

::: details start_mihomo
**名称** `start_mihomo`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app` | `AppHandle` | Tauri 注入 | 当前应用句柄 |
| `state` | `tauri::State<'_, SidecarState>` | Tauri 注入 | Mihomo sidecar 生命周期状态 |
| `geoip_config` | `tauri::State<'_, GeoIpConfigState>` | Tauri 注入 | 当前配置目录状态 |
| `config_path` | `String` | 前端传入 | Mihomo 配置目录路径 |

**返回值**

成功时返回 `()`；失败时返回 `SidecarError`。

**说明**

设置当前配置目录后启动 Mihomo，并重启日志订阅与流量订阅任务。

**调用示例**

::: code-group
```rust [Rust]
pub fn start_mihomo(
    app: AppHandle,
    state: tauri::State<'_, SidecarState>,
    geoip_config: tauri::State<'_, GeoIpConfigState>,
    config_path: String,
) -> Result<(), SidecarError>
```

```ts [TypeScript]
await invoke("start_mihomo", {
  configPath: "C:/Users/no525/.config/mihomo",
});
```
:::
:::

::: details stop_mihomo
**名称** `stop_mihomo`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, SidecarState>` | Tauri 注入 | Mihomo sidecar 生命周期状态 |

**返回值**

成功时返回 `()`；失败时返回 `SidecarError`。

**说明**

停止 Mihomo sidecar。

**调用示例**

::: code-group
```rust [Rust]
pub fn stop_mihomo(
    state: tauri::State<'_, SidecarState>,
) -> Result<(), SidecarError>
```

```ts [TypeScript]
await invoke("stop_mihomo");
```
:::
:::

::: details restart_mihomo
**名称** `restart_mihomo`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app` | `AppHandle` | Tauri 注入 | 当前应用句柄 |
| `state` | `tauri::State<'_, SidecarState>` | Tauri 注入 | Mihomo sidecar 生命周期状态 |
| `geoip_config` | `tauri::State<'_, GeoIpConfigState>` | Tauri 注入 | 当前配置目录状态 |
| `config_path` | `String` | 前端传入 | Mihomo 配置目录路径 |

**返回值**

成功时返回 `()`；失败时返回 `SidecarError`。

**说明**

重启 Mihomo，并重新建立日志与流量订阅。

**调用示例**

::: code-group
```rust [Rust]
pub fn restart_mihomo(
    app: AppHandle,
    state: tauri::State<'_, SidecarState>,
    geoip_config: tauri::State<'_, GeoIpConfigState>,
    config_path: String,
) -> Result<(), SidecarError>
```

```ts [TypeScript]
await invoke("restart_mihomo", {
  configPath: "C:/Users/no525/.config/mihomo",
});
```
:::
:::

::: details get_mihomo_status
**名称** `get_mihomo_status`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, SidecarState>` | Tauri 注入 | Mihomo sidecar 生命周期状态 |

**返回值**

成功时返回 `bool`；失败时返回 `SidecarError`。

**说明**

查询 Mihomo 进程是否正在运行。

**调用示例**

::: code-group
```rust [Rust]
pub fn get_mihomo_status(
    state: tauri::State<'_, SidecarState>,
) -> Result<bool, SidecarError>
```

```ts [TypeScript]
const running = await invoke<boolean>("get_mihomo_status");
```
:::
:::

::: details check_config_exists
**名称** `check_config_exists`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `config_path` | `String` | 前端传入 | Mihomo 配置目录路径 |

**返回值**

成功时返回 `bool`；失败时返回 `SidecarError`。

**说明**

检查目录下是否存在 `config.yaml`，并且文件内容包含 `external-controller`。

**调用示例**

::: code-group
```rust [Rust]
pub fn check_config_exists(config_path: String) -> Result<bool, SidecarError>
```

```ts [TypeScript]
const ok = await invoke<boolean>("check_config_exists", {
  configPath: "C:/Users/no525/.config/mihomo",
});
```
:::
:::

::: details ensure_default_config
**名称** `ensure_default_config`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `geoip_config` | `tauri::State<'_, GeoIpConfigState>` | Tauri 注入 | 当前配置目录状态 |
| `config_path` | `String` | 前端传入 | Mihomo 配置目录路径 |

**返回值**

成功时返回 `()`；失败时返回 `SidecarError`。

**说明**

确保配置目录存在，并在缺少有效 `config.yaml` 时写入默认内容。

**调用示例**

::: code-group
```rust [Rust]
pub fn ensure_default_config(
    geoip_config: tauri::State<'_, GeoIpConfigState>,
    config_path: String,
) -> Result<(), SidecarError>
```

```ts [TypeScript]
await invoke("ensure_default_config", {
  configPath: "C:/Users/no525/.config/mihomo",
});
```
:::
:::

## 代理命令

源文件：`src-tauri/src/cmd/proxy.rs`

::: details get_proxies
**名称** `get_proxies`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |

**返回值**

成功时返回 `serde_json::Value`，即 Mihomo `/proxies` 原始 JSON；失败时返回 `MihomoError`。

**说明**

获取当前所有代理组和节点信息。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_proxies(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError>
```

```ts [TypeScript]
const proxies = await invoke<Record<string, unknown>>("get_proxies");
```
:::
:::

::: details switch_proxy
**名称** `switch_proxy`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |
| `group` | `String` | 前端传入 | 代理组名称 |
| `name` | `String` | 前端传入 | 目标节点名称 |

**返回值**

成功时返回 `()`；失败时返回 `MihomoError`。

**说明**

切换代理组的当前节点。

**调用示例**

::: code-group
```rust [Rust]
pub async fn switch_proxy(
    state: tauri::State<'_, MihomoState>,
    group: String,
    name: String,
) -> Result<(), MihomoError>
```

```ts [TypeScript]
await invoke("switch_proxy", {
  group: "Proxy",
  name: "HK-01",
});
```
:::
:::

::: details test_delay
**名称** `test_delay`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |
| `name` | `String` | 前端传入 | 节点名称 |
| `url` | `String` | 前端传入 | 延迟测试 URL |
| `timeout` | `u32` | 前端传入 | 超时时间，单位毫秒 |

**返回值**

成功时返回 `u32`（延迟毫秒值）；失败时返回 `MihomoError`。

**说明**

对单个代理节点执行延迟测试。

**调用示例**

::: code-group
```rust [Rust]
pub async fn test_delay(
    state: tauri::State<'_, MihomoState>,
    name: String,
    url: String,
    timeout: u32,
) -> Result<u32, MihomoError>
```

```ts [TypeScript]
const delay = await invoke<number>("test_delay", {
  name: "HK-01",
  url: "http://www.gstatic.com/generate_204",
  timeout: 5000,
});
```
:::
:::

::: details test_group_delay
**名称** `test_group_delay`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |
| `group` | `String` | 前端传入 | 代理组名称 |
| `url` | `String` | 前端传入 | 延迟测试 URL |
| `timeout` | `u32` | 前端传入 | 超时时间，单位毫秒 |

**返回值**

成功时返回 `HashMap<String, u32>`，键为节点名、值为延迟毫秒值；失败时返回 `MihomoError`。

**说明**

对整个代理组执行延迟测试。

**调用示例**

::: code-group
```rust [Rust]
pub async fn test_group_delay(
    state: tauri::State<'_, MihomoState>,
    group: String,
    url: String,
    timeout: u32,
) -> Result<HashMap<String, u32>, MihomoError>
```

```ts [TypeScript]
const delays = await invoke<Record<string, number>>("test_group_delay", {
  group: "Auto",
  url: "http://www.gstatic.com/generate_204",
  timeout: 5000,
});
```
:::
:::

::: details get_rules
**名称** `get_rules`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |

**返回值**

成功时返回 `serde_json::Value`，即 Mihomo `/rules` 原始 JSON；失败时返回 `MihomoError`。

**说明**

获取当前规则列表。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_rules(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError>
```

```ts [TypeScript]
const rules = await invoke<Record<string, unknown>>("get_rules");
```
:::
:::

## 配置命令

源文件：`src-tauri/src/cmd/config.rs`

::: details read_config
**名称** `read_config`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `path` | `String` | 前端传入 | 配置文件路径，内部会执行 `expand_tilde` |

**返回值**

成功时返回 `String`（文件全文）；失败时返回 `ConfigError`。

**说明**

读取指定配置文件内容。

**调用示例**

::: code-group
```rust [Rust]
pub async fn read_config(path: String) -> Result<String, ConfigError>
```

```ts [TypeScript]
const content = await invoke<string>("read_config", {
  path: "~/mihomo/config.yaml",
});
```
:::
:::

::: details write_config
**名称** `write_config`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `path` | `String` | 前端传入 | 配置文件路径，内部会执行 `expand_tilde` |
| `content` | `String` | 前端传入 | 要写入的完整文件内容 |

**返回值**

成功时返回 `()`；失败时返回 `ConfigError`。

**说明**

覆写指定配置文件内容。

**调用示例**

::: code-group
```rust [Rust]
pub async fn write_config(
    path: String,
    content: String,
) -> Result<(), ConfigError>
```

```ts [TypeScript]
await invoke("write_config", {
  path: "~/mihomo/config.yaml",
  content,
});
```
:::
:::

::: details reload_config
**名称** `reload_config`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |

**返回值**

成功时返回 `()`；失败时返回 `ConfigError`。

**说明**

调用 Mihomo `reload_configs()` 热重载当前配置。

**调用示例**

::: code-group
```rust [Rust]
pub async fn reload_config(
    state: tauri::State<'_, MihomoState>,
) -> Result<(), ConfigError>
```

```ts [TypeScript]
await invoke("reload_config");
```
:::
:::

::: details get_configs
**名称** `get_configs`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |

**返回值**

成功时返回 `serde_json::Value`，即 Mihomo 当前运行时配置 JSON；失败时返回 `MihomoError`。

**说明**

获取 Mihomo 运行时配置快照。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_configs(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError>
```

```ts [TypeScript]
const runtimeConfig = await invoke<Record<string, unknown>>("get_configs");
```
:::
:::

::: details patch_configs
**名称** `patch_configs`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |
| `payload` | `serde_json::Value` | 前端传入 | 发送给 Mihomo 的部分配置补丁 |

**返回值**

成功时返回 `()`；失败时返回 `MihomoError`。

**说明**

向 Mihomo 运行时配置发送 JSON patch。

**调用示例**

::: code-group
```rust [Rust]
pub async fn patch_configs(
    state: tauri::State<'_, MihomoState>,
    payload: serde_json::Value,
) -> Result<(), MihomoError>
```

```ts [TypeScript]
await invoke("patch_configs", {
  payload: {
    mode: "rule",
  },
});
```
:::
:::

## 系统命令

源文件：`src-tauri/src/cmd/system.rs`

::: details get_version
**名称** `get_version`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |

**返回值**

成功时返回 `serde_json::Value`，即 Mihomo `/version` 原始 JSON；失败时返回 `MihomoError`。

**说明**

获取 Mihomo 版本信息。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_version(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError>
```

```ts [TypeScript]
const version = await invoke<Record<string, unknown>>("get_version");
```
:::
:::

::: details close_connection
**名称** `close_connection`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |
| `id` | `String` | 前端传入 | 连接 ID |

**返回值**

成功时返回 `()`；失败时返回 `MihomoError`。

**说明**

关闭指定连接。

**调用示例**

::: code-group
```rust [Rust]
pub async fn close_connection(
    state: tauri::State<'_, MihomoState>,
    id: String,
) -> Result<(), MihomoError>
```

```ts [TypeScript]
await invoke("close_connection", { id: "conn-123" });
```
:::
:::

::: details close_all_connections
**名称** `close_all_connections`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |

**返回值**

成功时返回 `()`；失败时返回 `MihomoError`。

**说明**

关闭当前所有连接。

**调用示例**

::: code-group
```rust [Rust]
pub async fn close_all_connections(
    state: tauri::State<'_, MihomoState>,
) -> Result<(), MihomoError>
```

```ts [TypeScript]
await invoke("close_all_connections");
```
:::
:::

::: details get_connections
**名称** `get_connections`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |

**返回值**

成功时返回 `serde_json::Value`，即 Mihomo `/connections` 原始 JSON；失败时返回 `MihomoError`。

**说明**

获取当前连接列表和总流量信息。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_connections(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError>
```

```ts [TypeScript]
const connections = await invoke<Record<string, unknown>>("get_connections");
```
:::
:::

::: details set_system_proxy
**名称** `set_system_proxy`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `enable` | `bool` | 前端传入 | 是否启用系统代理 |
| `port` | `u16` | 前端传入 | 代理端口；host 固定为 `127.0.0.1` |

**返回值**

成功时返回 `()`；失败时返回 `SysproxyError`。

**说明**

设置系统代理状态，实际调用 `sysproxy::set_system_proxy(enable, "127.0.0.1", port)`。

**调用示例**

::: code-group
```rust [Rust]
pub fn set_system_proxy(enable: bool, port: u16) -> Result<(), SysproxyError>
```

```ts [TypeScript]
await invoke("set_system_proxy", {
  enable: true,
  port: 7890,
});
```
:::
:::

::: details get_system_proxy
**名称** `get_system_proxy`

**参数**

无。

**返回值**

成功时返回对象 `{ enable: boolean, host: string, port: number }`；失败时返回 `SysproxyError`。

**说明**

读取当前系统代理状态。

**调用示例**

::: code-group
```rust [Rust]
pub fn get_system_proxy() -> Result<serde_json::Value, SysproxyError>
```

```ts [TypeScript]
const proxy = await invoke<{ enable: boolean; host: string; port: number }>(
  "get_system_proxy",
);
```
:::
:::

::: details update_mihomo_client
**名称** `update_mihomo_client`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | Mihomo 客户端状态 |
| `base_url` | `String` | 前端传入 | Mihomo API Base URL |
| `secret` | `String` | 前端传入 | Mihomo API Secret |

**返回值**

成功时返回 `()`；失败时返回 `MihomoError`。

**说明**

更新内存中的 Mihomo 客户端连接地址和 secret。

**调用示例**

::: code-group
```rust [Rust]
pub async fn update_mihomo_client(
    state: tauri::State<'_, MihomoState>,
    base_url: String,
    secret: String,
) -> Result<(), MihomoError>
```

```ts [TypeScript]
await invoke("update_mihomo_client", {
  baseUrl: "http://127.0.0.1:9090",
  secret: "",
});
```
:::
:::

## 采集器命令

源文件：`src-tauri/src/cmd/collector.rs`

::: details start_collector
**名称** `start_collector`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `tauri::AppHandle` | Tauri 注入 | 当前应用句柄 |
| `state` | `tauri::State<'_, CollectorState>` | Tauri 注入 | 采集器生命周期状态 |
| `mihomo_state` | `tauri::State<'_, MihomoState>` | Tauri 注入 | 提供 Mihomo API 地址与 secret |

**返回值**

成功时返回 `()`；失败时返回 `CollectorError`。

**说明**

启动 WebSocket 连接采集器，重置实时缓存并在后台启动采集任务。

**调用示例**

::: code-group
```rust [Rust]
pub async fn start_collector(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, CollectorState>,
    mihomo_state: tauri::State<'_, MihomoState>,
) -> Result<(), CollectorError>
```

```ts [TypeScript]
await invoke("start_collector");
```
:::
:::

::: details stop_collector
**名称** `stop_collector`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `tauri::AppHandle` | Tauri 注入 | 当前应用句柄 |
| `state` | `tauri::State<'_, CollectorState>` | Tauri 注入 | 采集器生命周期状态 |

**返回值**

成功时返回 `()`；失败时返回 `CollectorError`。

**说明**

停止采集器运行时，并清空实时缓存。

**调用示例**

::: code-group
```rust [Rust]
pub async fn stop_collector(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, CollectorState>,
) -> Result<(), CollectorError>
```

```ts [TypeScript]
await invoke("stop_collector");
```
:::
:::

::: details get_collector_status
**名称** `get_collector_status`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `state` | `tauri::State<'_, CollectorState>` | Tauri 注入 | 采集器生命周期状态 |

**返回值**

成功时返回 `bool`；失败时返回 `CollectorError`。

**说明**

查询采集器是否正在运行。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_collector_status(
    state: tauri::State<'_, CollectorState>,
) -> Result<bool, CollectorError>
```

```ts [TypeScript]
const running = await invoke<boolean>("get_collector_status");
```
:::
:::

::: details get_realtime_connections
**名称** `get_realtime_connections`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `store` | `tauri::State<'_, RealtimeStore>` | Tauri 注入 | 实时连接缓存 |

**返回值**

成功时返回 `Vec<ConnectionRecord>`；失败时返回 `CollectorError`。

**说明**

返回当前活动连接列表，字段见文末 `ConnectionRecord`。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_realtime_connections(
    store: tauri::State<'_, RealtimeStore>,
) -> Result<Vec<ConnectionRecord>, CollectorError>
```

```ts [TypeScript]
const connections = await invoke<ConnectionRecord[]>("get_realtime_connections");
```
:::
:::

::: details get_realtime_summary
**名称** `get_realtime_summary`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `store` | `tauri::State<'_, RealtimeStore>` | Tauri 注入 | 实时连接缓存 |

**返回值**

成功时返回 `RealtimeSummary`；失败时返回 `CollectorError`。

**说明**

返回活动连接数、总上传下载、热点域名和热点规则摘要。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_realtime_summary(
    store: tauri::State<'_, RealtimeStore>,
) -> Result<RealtimeSummary, CollectorError>
```

```ts [TypeScript]
const summary = await invoke<RealtimeSummary>("get_realtime_summary");
```
:::
:::

## 统计命令

源文件：`src-tauri/src/cmd/stats.rs`

::: details manual_cleanup
**名称** `manual_cleanup`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `tauri::AppHandle` | Tauri 注入 | 用于访问数据库 |

**返回值**

成功时返回 `cleanup::CleanupReport`；失败时返回 `DbError`。

**说明**

手动执行一轮数据库清理，包含连接、小时聚合、域名统计和 GeoIP 缓存。

**调用示例**

::: code-group
```rust [Rust]
pub async fn manual_cleanup(
    app_handle: tauri::AppHandle,
) -> Result<cleanup::CleanupReport, DbError>
```

```ts [TypeScript]
const report = await invoke<CleanupReport>("manual_cleanup");
```
:::
:::

::: details get_db_stats
**名称** `get_db_stats`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `tauri::AppHandle` | Tauri 注入 | 用于访问数据库 |

**返回值**

成功时返回 `DbStats`；失败时返回 `DbError`。

**说明**

返回数据库表行数、文件大小和最早连接时间。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_db_stats(
    app_handle: tauri::AppHandle,
) -> Result<DbStats, DbError>
```

```ts [TypeScript]
const stats = await invoke<DbStats>("get_db_stats");
```
:::
:::

::: details get_domain_stats
**名称** `get_domain_stats`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `tauri::AppHandle` | Tauri 注入 | 用于访问数据库 |
| `days` | `i32` | 前端传入 | 统计窗口天数 |
| `limit` | `i32` | 前端传入 | 返回条数上限 |

**返回值**

成功时返回 `Vec<DomainStat>`；失败时返回 `DbError`。

**说明**

查询指定时间窗口内的热点域名统计。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_domain_stats(
    app_handle: tauri::AppHandle,
    days: i32,
    limit: i32,
) -> Result<Vec<DomainStat>, DbError>
```

```ts [TypeScript]
const domains = await invoke<DomainStat[]>("get_domain_stats", {
  days: 7,
  limit: 20,
});
```
:::
:::

::: details get_traffic_hourly
**名称** `get_traffic_hourly`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `tauri::AppHandle` | Tauri 注入 | 用于访问数据库 |
| `start` | `String` | 前端传入 | 起始时间字符串 |
| `end` | `String` | 前端传入 | 结束时间字符串 |

**返回值**

成功时返回 `Vec<TrafficPoint>`；失败时返回 `DbError`。

**说明**

查询小时粒度流量与连接数时间序列。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_traffic_hourly(
    app_handle: tauri::AppHandle,
    start: String,
    end: String,
) -> Result<Vec<TrafficPoint>, DbError>
```

```ts [TypeScript]
const points = await invoke<TrafficPoint[]>("get_traffic_hourly", {
  start: "2026-04-01T00:00:00Z",
  end: "2026-04-08T00:00:00Z",
});
```
:::
:::

::: details get_traffic_daily
**名称** `get_traffic_daily`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `tauri::AppHandle` | Tauri 注入 | 用于访问数据库 |
| `start` | `String` | 前端传入 | 起始时间字符串 |
| `end` | `String` | 前端传入 | 结束时间字符串 |

**返回值**

成功时返回 `Vec<TrafficPoint>`；失败时返回 `DbError`。

**说明**

查询天粒度流量与连接数时间序列。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_traffic_daily(
    app_handle: tauri::AppHandle,
    start: String,
    end: String,
) -> Result<Vec<TrafficPoint>, DbError>
```

```ts [TypeScript]
const points = await invoke<TrafficPoint[]>("get_traffic_daily", {
  start: "2026-04-01T00:00:00Z",
  end: "2026-04-08T00:00:00Z",
});
```
:::
:::

::: details get_stats_overview
**名称** `get_stats_overview`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `tauri::AppHandle` | Tauri 注入 | 用于访问数据库 |
| `days` | `i32` | 前端传入 | 统计窗口天数 |

**返回值**

成功时返回 `StatsOverview`；失败时返回 `DbError`。

**说明**

返回总连接数、总上传、总下载、活动连接数和唯一域名数。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_stats_overview(
    app_handle: tauri::AppHandle,
    days: i32,
) -> Result<StatsOverview, DbError>
```

```ts [TypeScript]
const overview = await invoke<StatsOverview>("get_stats_overview", {
  days: 7,
});
```
:::
:::

::: details get_rule_stats
**名称** `get_rule_stats`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `tauri::AppHandle` | Tauri 注入 | 用于访问数据库 |
| `days` | `i32` | 前端传入 | 统计窗口天数 |
| `limit` | `i32` | 前端传入 | 返回条数上限 |

**返回值**

成功时返回 `Vec<RuleStat>`；失败时返回 `DbError`。

**说明**

查询规则命中统计。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_rule_stats(
    app_handle: tauri::AppHandle,
    days: i32,
    limit: i32,
) -> Result<Vec<RuleStat>, DbError>
```

```ts [TypeScript]
const rules = await invoke<RuleStat[]>("get_rule_stats", {
  days: 7,
  limit: 20,
});
```
:::
:::

::: details get_geo_stats
**名称** `get_geo_stats`

**参数**

| 参数 | Rust 类型 | 来源 | 说明 |
| --- | --- | --- | --- |
| `app_handle` | `tauri::AppHandle` | Tauri 注入 | 用于访问数据库 |
| `geoip_config` | `tauri::State<'_, GeoIpConfigState>` | Tauri 注入 | 提供 MMDB 文件路径 |
| `geoip_lookup` | `tauri::State<'_, GeoIpLookup>` | Tauri 注入 | GeoIP 查询与缓存能力 |
| `days` | `i32` | 前端传入 | 统计窗口天数 |

**返回值**

成功时返回 `Vec<GeoStat>`；失败时返回 `DbError`。

**说明**

按国家聚合指定时间窗口内的连接数与总流量。内部会先按目标 IP 聚合，再通过 GeoIP 解析国家信息。

**调用示例**

::: code-group
```rust [Rust]
pub async fn get_geo_stats(
    app_handle: tauri::AppHandle,
    geoip_config: tauri::State<'_, GeoIpConfigState>,
    geoip_lookup: tauri::State<'_, GeoIpLookup>,
    days: i32,
) -> Result<Vec<GeoStat>, DbError>
```

```ts [TypeScript]
const stats = await invoke<GeoStat[]>("get_geo_stats", {
  days: 7,
});
```
:::
:::

## 通用类型

以下类型直接出自 `src-tauri/src/cmd/ai.rs`、`src-tauri/src/cmd/stats.rs`、`src-tauri/src/collector/*`、`src-tauri/src/db/*`。

### 枚举值

| 类型 | 可选值 |
| --- | --- |
| `AiProviderKind` | `openai`、`openai_compatible`、`claude`、`gemini` |
| `AiChatRole` | `user`、`assistant`、`system` |
| `ReportType` | `daily`、`weekly` |
| `AiModelCatalogSource` | `remote`、`fallback`、`empty` |

### AiProviderSettings

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `provider` | `AiProviderKind` | Provider 类型 |
| `model` | `String` | 模型名称 |
| `apiKey` | `Option<String>` | API Key，可选 |
| `baseUrl` | `Option<String>` | Base URL，可选 |
| `temperature` | `Option<f64>` | 采样温度，可选 |
| `maxTokens` | `Option<u32>` | 最大输出 token 数，可选 |

### AiSettings

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `provider` | `AiProviderKind` | Provider 类型 |
| `model` | `String` | 模型名称 |
| `apiKey` | `String` | API Key |
| `baseUrl` | `String` | Base URL |
| `temperature` | `f64` | 采样温度 |
| `maxTokens` | `u32` | 最大输出 token 数 |
| `autoStart` | `bool` | 是否自动启动 AI sidecar |

### AiChatMessage

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `role` | `AiChatRole` | 消息角色 |
| `content` | `String` | 消息内容 |

### AiChatContext

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `currentConfig` | `Option<String>` | 当前配置文本，可选 |
| `recentStats` | `Option<serde_json::Value>` | 最近统计信息，可选 |
| `availableProxies` | `Option<Vec<String>>` | 当前可用代理名称列表，可选 |

### AiChatParams

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `messages` | `Vec<AiChatMessage>` | 聊天消息数组 |
| `context` | `Option<AiChatContext>` | 补充上下文，可选 |
| `settings` | `AiProviderSettings` | 对话使用的 Provider 设置 |

### SaveConversationMessageParams

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `role` | `AiChatRole` | 消息角色 |
| `content` | `String` | 消息正文 |
| `toolCalls` | `Option<serde_json::Value>` | 工具调用结果，可选 |
| `tokensUsed` | `Option<i32>` | token 使用量，可选 |
| `model` | `Option<String>` | 模型名称，可选 |

### AiConnectionTestResult

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `success` | `bool` | 是否连接成功 |
| `latencyMs` | `u64` | 耗时毫秒数 |
| `message` | `String` | 结果消息 |

### AiModelCatalog

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `models` | `Vec<String>` | 模型名称列表 |
| `source` | `AiModelCatalogSource` | 模型来源 |
| `message` | `String` | 返回消息 |

### ReportPeriod

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `start` | `String` | 统计起始日期 |
| `end` | `String` | 统计结束日期 |

### ReportTrafficSummary

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `upload` | `i64` | 上传字节数 |
| `download` | `i64` | 下载字节数 |

### ReportDomainStat

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `domain` | `String` | 域名 |
| `traffic` | `i64` | 总流量 |

### ReportRuleStat

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `rule` | `String` | 规则名称 |
| `hitCount` | `i64` | 命中次数 |

### ReportComparison

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `trafficChange` | `f64` | 相比上一周期的流量变化 |
| `connectionChange` | `f64` | 相比上一周期的连接数变化 |

### ReportDailyTrendPoint

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `date` | `String` | 日期 |
| `upload` | `i64` | 上传字节数 |
| `download` | `i64` | 下载字节数 |
| `totalTraffic` | `i64` | 总流量 |
| `connCount` | `i64` | 连接数 |

### ReportStats

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `totalTraffic` | `ReportTrafficSummary` | 总流量摘要 |
| `totalConnections` | `i64` | 总连接数 |
| `topDomains` | `Vec<ReportDomainStat>` | 热门域名 |
| `topRules` | `Vec<ReportRuleStat>` | 热门规则 |
| `peakHour` | `Option<String>` | 峰值小时，可选 |
| `comparison` | `Option<ReportComparison>` | 环比变化，可选 |
| `dailyTrend` | `Option<Vec<ReportDailyTrendPoint>>` | 日趋势，可选 |
| `matchRate` | `Option<f64>` | 命中率，可选 |

### ReportResult

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `type` | `ReportType` | 报告类型 |
| `period` | `ReportPeriod` | 报告周期 |
| `content` | `String` | AI 生成的正文 |
| `stats` | `ReportStats` | 统计摘要 |
| `generatedAt` | `String` | 生成时间 |

### ConfigSnapshot

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | `i64` | 快照 ID |
| `content` | `String` | 快照内容 |
| `source` | `String` | 快照来源，例如 `manual`、`ai` |
| `description` | `Option<String>` | 快照描述 |
| `filePath` | `Option<String>` | 关联配置文件路径 |
| `createdAt` | `String` | 创建时间 |

### CleanupReport

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `connectionsDeleted` | `usize` | 删除的连接记录数 |
| `hourlyDeleted` | `usize` | 删除的小时聚合数 |
| `domainStatsDeleted` | `usize` | 删除的域名统计数 |
| `geoipDeleted` | `usize` | 删除的 GeoIP 缓存数 |
| `executedAt` | `String` | 执行时间 |

### DomainStat

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `domain` | `String` | 域名 |
| `hitCount` | `i64` | 命中次数 |
| `upload` | `i64` | 上传字节数 |
| `download` | `i64` | 下载字节数 |

### TrafficPoint

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `time` | `String` | 时间桶 |
| `upload` | `i64` | 上传字节数 |
| `download` | `i64` | 下载字节数 |
| `connCount` | `i64` | 连接数 |

### StatsOverview

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `totalConnections` | `i64` | 总连接数 |
| `totalUpload` | `i64` | 总上传字节数 |
| `totalDownload` | `i64` | 总下载字节数 |
| `activeConnections` | `i64` | 当前活动连接数 |
| `uniqueDomains` | `i64` | 唯一域名数 |

### RuleStat

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `rule` | `String` | 规则名称 |
| `hitCount` | `i64` | 命中次数 |
| `upload` | `i64` | 上传字节数 |
| `download` | `i64` | 下载字节数 |

### GeoStat

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `countryCode` | `String` | 国家代码 |
| `country` | `String` | 国家名称 |
| `connCount` | `i64` | 连接数 |
| `totalTraffic` | `i64` | 总流量 |

### DbStats

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `connectionCount` | `i64` | `connections` 表记录数 |
| `hourlyCount` | `i64` | `traffic_hourly` 表记录数 |
| `dailyCount` | `i64` | `traffic_daily` 表记录数 |
| `domainCount` | `i64` | `domain_stats` 表记录数 |
| `geoipCount` | `i64` | `geoip_cache` 表记录数 |
| `dbSizeBytes` | `i64` | 数据库文件大小 |
| `oldestConnection` | `Option<String>` | 最早连接时间 |

### ConnectionRecord

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | `String` | 连接 ID |
| `host` | `String` | 目标主机名 |
| `dstIp` | `Option<String>` | 目标 IP |
| `dstPort` | `Option<i32>` | 目标端口 |
| `srcIp` | `Option<String>` | 源 IP |
| `srcPort` | `Option<i32>` | 源端口 |
| `network` | `String` | 网络协议 |
| `connType` | `String` | 连接类型 |
| `rule` | `String` | 命中规则 |
| `rulePayload` | `Option<String>` | 规则载荷 |
| `proxyChain` | `String` | 代理链 |
| `upload` | `i64` | 上传字节数 |
| `download` | `i64` | 下载字节数 |
| `startTime` | `String` | 开始时间 |

### RealtimeSummary

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `activeCount` | `usize` | 当前活动连接数 |
| `totalUpload` | `i64` | 总上传字节数 |
| `totalDownload` | `i64` | 总下载字节数 |
| `topDomains` | `Vec<(String, i64)>` | 热门域名与流量 |
| `topRules` | `Vec<(String, usize)>` | 热门规则与命中次数 |
