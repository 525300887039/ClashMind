# 开发指南

这一页不是仓库里所有开发说明的逐字搬运版。

它的目标是把当前项目最重要、最容易踩坑、最影响代码质量的一组约束收敛到一页里，方便你在真正动手之前建立共识。

如果你还没有理解项目边界，建议先看 [系统架构](./architecture.md)。

如果你还没有完成本地环境准备，先回到 [快速开始](./quickstart.md)。

## 开发原则

ClashMind 当前的开发原则可以概括成五句话：

1. 前端负责交互，Rust 负责中控，AI sidecar 负责模型相关逻辑。
2. 后端事实走 TanStack Query，本地交互走 Zustand。
3. 前端不直连 Mihomo，统一经过 Rust IPC。
4. 配置变更优先走校验、预览、确认，再考虑落盘。
5. 代码组织优先服从边界清晰，而不是“哪里方便先写哪里”。

这些原则听上去很抽象，但它们几乎决定了你每次改代码时文件应该落在哪一层。

## 前端规范

### 使用 TypeScript 严格模式

前端默认遵循严格类型约束。

最简单的理解方式是：

- 不要用 `any`
- 不要把后端返回值当成“随便长什么样都可以”
- 能在类型层约束的东西，尽量不要推迟到运行时再赌

项目里的 IPC 返回类型已经集中定义在 `src/lib/tauri-api.ts`。

这意味着你新增一个后端命令时，前端通常也要同步新增类型，而不是让调用方自己猜返回值。

### 组件、hooks 和 API 要分层

前端不是所有逻辑都堆在组件里。

比较推荐的职责划分是：

- 组件负责渲染与交互
- hooks 负责请求、缓存和动作封装
- `lib/tauri-api.ts` 负责统一的 IPC 函数入口

例如代理模块的 hook：

```ts
export function useProxies() {
  return useQuery({ queryKey: PROXY_KEYS.all, queryFn: api.proxy.getAll });
}

export function useSwitchProxy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ group, name }: { group: string; name: string }) =>
      api.proxy.switch(group, name),
    onSuccess: () => qc.invalidateQueries({ queryKey: PROXY_KEYS.all }),
  });
}
```

这段代码体现了前端的推荐模式：

- 组件不要直接 `invoke`
- 组件通过 hook 获取数据和动作
- hook 通过统一 `api` 对象访问 IPC

### 路径别名优先用 `@/*`

项目当前使用 `@/* -> src/*` 的路径别名。

这意味着在前端代码里，优先写：

```ts
import { api } from "@/lib/tauri-api";
import { useAppStore } from "@/stores/app-store";
```

而不是满屏相对路径层层回退。

当一个模块开始频繁使用 `../../../../` 时，通常说明路径组织或职责边界已经需要重新审视。

## Rust 后端规范

### 错误处理优先使用 `thiserror`

Rust 侧当前约定非常明确：

- 业务错误用 `thiserror` 定义
- IPC 命令返回 `Result<T, CustomError>`
- 错误类型需要能序列化给前端

这不是风格偏好，而是为了让 Tauri 命令错误在前端可读、可展示、可追踪。

### 禁止随手 `unwrap()` / `expect()`

这里的规则也很直接：

- 正常业务路径不要用 `unwrap()`
- 需要向上抛的错误就明确返回
- 测试中可以更宽松，但生产逻辑应把失败路径写出来

这条约束在 sidecar 管理、文件操作、SQLite 和 Mihomo 通信上尤其重要。

因为这些地方几乎都属于外部系统边界，失败不是例外，而是常态之一。

### Tauri 2 Sidecar 使用现代 API

项目明确使用 `tauri_plugin_shell::ShellExt` 管理 sidecar。

也就是说：

- 不要再用已经废弃或移除的旧 API 习惯
- 新增 sidecar 或 shell 逻辑时，优先沿用现有 `core/sidecar.rs` 的模式

如果你准备改 sidecar 启停逻辑，先读 `src-tauri/src/core/sidecar.rs`，不要凭旧版 Tauri 教程直接写。

## IPC 规则

### 前端不直连 Mihomo

这是项目里最重要的边界规则之一。

即使某个 Mihomo API 看起来“前端直接 fetch 一下也能成”，也不要这么做。

正确路径应当是：

1. 前端通过 `api.*` 发起 `invoke`
2. Rust 命令负责参数整形与边界处理
3. Rust 调用 Mihomo HTTP/WS 或操作本地文件
4. 结果再回到前端

原因很实际：

- 地址和 secret 不必散落在前端
- 文件写入不会穿透到 UI 层
- 日志、重试、快照和权限更容易收口

### IPC 类型统一放在 `src/lib/tauri-api.ts`

这个文件不是简单的工具文件。

它在当前项目里承担了三个角色：

- IPC 命令入口
- 前后端共享的 TypeScript 视图类型
- AI、代理、统计、配置、系统能力的统一门面

真实代码结构大致是这样：

```ts
export const api = {
  ai: {
    start: () => invoke("start_ai_service"),
    status: () => invoke<boolean>("get_ai_status"),
    chat: (params: AiChatParams) => invoke("ai_chat", { params }),
  },
  proxy: {
    getAll: () => invoke<ProxiesResponse>("get_proxies"),
    switch: (group: string, name: string) => invoke("switch_proxy", { group, name }),
  },
  config: {
    read: (path: string) => invoke<string>("read_config", { path }),
    write: (path: string, content: string) => invoke("write_config", { path, content }),
  },
} as const;
```

如果你要新增一个 IPC 能力，推荐顺序通常是：

1. 先补 Rust 命令
2. 再补前端类型
3. 最后把 `api` 门面和对应 hook 接起来

这样调用方就不需要自己知道具体命令名和参数细节。

### AI 相关 IPC 也要遵循相同边界

AI 功能虽然复杂，但同样不能绕过统一出口。

例如：

- 启停 AI sidecar 通过 `start_ai_service` / `stop_ai_service`
- 聊天通过 `ai_chat`
- 模型列表通过 `fetch_ai_models`
- 应用配置变更通过 `apply_config_change`

这保证了 AI 相关流程仍然受 Rust 统一约束，而不是变成前端和 AI sidecar 的直连通道。

:::warning 不要引入第二套调用路径
如果某个功能已经有 `invoke -> Rust -> sidecar/Mihomo` 的正路，就不要为了“图快”在前端新开一条直连路径。

一旦出现两套入口，缓存、日志、权限和错误处理会立刻分叉。
:::

## 状态管理规则

### TanStack Query 管后端事实

下列数据优先放到 Query：

- 代理列表
- 配置文件内容
- 统计查询结果
- AI 设置
- 快照列表
- 连接状态

判断标准很简单：

如果这份数据来自 Rust、Mihomo、SQLite 或 AI service，而且存在“重新获取”这件事，那么它大概率应该进 Query。

### Zustand 管客户端状态

下列状态更适合放到 Zustand：

- 主题
- 侧边栏折叠
- 当前页面
- 表单草稿
- AI 面板中的本地消息流状态
- 待确认配置 payload 的临时持有

全局应用 store 的真实结构可以作为参考：

```ts
interface AppState {
  theme: Theme;
  sidebarCollapsed: boolean;
  currentPage: Page;
  mihomoConfigDir: string;
  apiAddress: string;
  apiSecret: string;
  httpPort: number;
  socksPort: number;
  language: "zh-CN" | "en-US";
}
```

AI 会话 store 则更像一个本地状态机：

- 有消息数组
- 有流式进行中的消息 ID
- 有工具调用状态
- 有待应用的配置 payload

这正是 Zustand 擅长的状态类型。

### 不要把 Query 当成全局状态容器

Query 适合缓存后端数据，不适合拿来保存任意本地 UI 偏好。

例如主题、导航、表单展开状态这类东西，硬塞到 Query 里会让缓存语义变得很奇怪。

同样地，也不要把需要失效、刷新、重试的后端事实手搓进 Zustand。

## 命名与目录约定

### 文件名使用 kebab-case

前端和多数 TypeScript 文件，当前约定使用 kebab-case。

例如：

- `config-diff-preview.tsx`
- `use-ai-settings.ts`
- `proxy-mode-switch.tsx`

这样做有两个收益：

- 文件名风格统一
- 在跨平台文件系统上更稳定，避免大小写混用带来的问题

### React 组件使用 PascalCase

文件名是 kebab-case，不代表导出组件也要同样命名。

组件本身保持 PascalCase 更符合 React 生态习惯。

例如：

- 文件名：`config-diff-preview.tsx`
- 组件名：`ConfigDiffPreview`

### 目录优先按职责和业务切分

当前仓库的推荐落点是：

- 跨业务 UI：`src/components/`
- 业务模块：`src/features/<domain>/`
- 全局 hook：`src/hooks/`
- 纯工具和 API：`src/lib/`
- 本地状态：`src/stores/`

Rust 侧同理：

- 命令定义在 `cmd/`
- 核心逻辑在 `core/`
- 数据采集在 `collector/`
- 数据库存取在 `db/`

如果你新增一段代码时犹豫该放哪，优先问自己：

“它是某个业务模块的一部分，还是一项跨模块能力？”

## UI 与依赖约定

### 样式优先用 Tailwind CSS 4 原子类

当前项目默认使用 Tailwind CSS 4。

这意味着：

- 页面和组件样式优先通过原子类表达
- 主题和设计 token 优先沿用现有变量与体系
- 不要为小范围样式频繁新增单独 CSS 文件

### 组件能力优先复用现有基建

当前 UI 生态主要包括：

- Radix UI
- lucide-react
- framer-motion
- Monaco Editor

如果你需要弹层、下拉、开关、Tabs 之类的基础交互，优先看是否已有对应 Radix 组件封装。

不要为了一个小需求再引入第二套功能重复的组件库。

### Radix UI 按需安装

当前约定不是“一次性装全家桶”。

而是需要哪个包，就明确引入哪个 `@radix-ui/react-*` 依赖。

这种方式更利于控制依赖体积，也更符合当前仓库已经建立起来的组件结构。

## AI 开发时的额外边界

如果你改的是 AI 功能，除了普通前后端规范外，还要额外注意三条。

第一，不要让模型直接落盘或直接操作系统状态。

第二，不要跳过脱敏、Schema 校验和 diff 预览。

第三，不要把 Provider 特定逻辑散落到前端组件里。

比较理想的职责分布是：

- 前端只负责设置表单、聊天 UI 和 diff 展示
- Provider 细节留在 `ai-service/src/providers/`
- 工具定义留在 `ai-service/src/tools/`
- 安全逻辑留在 `ai-service/src/safety/`

这样当某个 Provider、Schema 或工具策略变化时，不需要把 UI 层也拖下水。

## 提交与检查建议

### 提交前至少做这几项检查

对前端或文档改动，最少建议执行：

```bash
pnpm lint
pnpm type-check
```

如果你改了 Rust，最好再补：

```bash
cargo fmt --check
cargo clippy
cargo test
```

如果你改了文档站，再补一次：

```bash
cd docs
npm run docs:build
```

项目并不是要求你每次都把所有命令跑满，但至少应该针对改动层级做最基本的自检。

### 提交信息建议

仓库当前文档约定推荐使用语义前缀：

- `feat:`
- `fix:`
- `refactor:`

这不是说当前仓库已经强制配置了 commit lint。

它更像是一条团队协作建议：

让变更目的在历史记录里更容易扫读。

### 变更描述优先写“为什么”

在提交说明或 PR 描述里，尽量回答这三个问题：

1. 改了什么
2. 为什么要这么改
3. 如何验证

尤其是 IPC、状态管理和 AI 逻辑改动，如果不写“为什么”，后续维护者会很难判断这条边界是不是有意设计。

## 一个推荐的开发顺序

如果你准备加一个新功能，可以按下面这个顺序推进：

1. 先确认功能属于前端、Rust 还是 AI sidecar 哪一层。
2. 如果涉及后端能力，先定义 Rust 命令和返回结构。
3. 在 `src/lib/tauri-api.ts` 暴露前端入口。
4. 为这个入口写 Query hook 或 Mutation hook。
5. 最后再写组件和页面交互。

如果需求涉及 AI 配置变更，则再加两步：

6. 在 `ai-service/src/tools/` 补工具和 schema。
7. 检查脱敏、预览和确认链路是否完整。

这套顺序的好处是，你会天然沿着边界开发，而不是先把 UI 写死，再回头补系统层。

:::tip 新增功能时先找“现有模式”
在 ClashMind 里，最稳的开发方式不是从空白开始设计一套新写法。

更好的方式是先找一个最接近的现有 feature，沿着它的目录结构、hook 形态和 API 封装方式继续写。
:::

## 继续阅读

到这里，你应该已经知道在这个仓库里哪些做法是推荐路径，哪些做法会越过边界。

如果你准备改 AI 逻辑，继续看 [AI Sidecar](./ai-service.md)。

如果你想回到整体系统视角，回看 [系统架构](./architecture.md)。

如果你只是还没把项目跑起来，回到 [快速开始](./quickstart.md)。
