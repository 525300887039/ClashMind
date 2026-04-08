# 项目概述

ClashMind 是一个面向 Mihomo 用户的桌面客户端。

它用 Tauri 2 把 React 前端、Rust 后端、Mihomo sidecar 和 AI sidecar 组合在一起，目标不是再做一个“只能点按钮切节点”的图形壳，而是把代理管理、配置编辑、统计分析和 AI 辅助放进同一个工作台。

如果你第一次接触这个仓库，建议先读完本页，再继续看 [快速开始](./quickstart.md)、[系统架构](./architecture.md) 和 [AI Sidecar](./ai-service.md)。

:::tip 阅读顺序
如果你的目标是本地跑起来，下一页应该先看 [快速开始](./quickstart.md)。

如果你的目标是理解项目边界和模块职责，下一页应该先看 [系统架构](./architecture.md)。

如果你的目标是接入模型、排查 AI 行为或写 AI 相关功能，下一页应该先看 [AI Sidecar](./ai-service.md)。
:::

## ClashMind 是什么

从产品视角看，ClashMind 是一个跨平台 Mihomo 桌面客户端。

从工程视角看，它是一个三层应用：

1. React 19 + TypeScript 负责桌面 WebView 界面。
2. Rust + Tauri 负责系统集成、进程管理、IPC 和持久化。
3. Mihomo 与 AI Service 作为 sidecar 进程独立运行。

这样做的结果是：

- 代理管理、连接列表、规则、日志、配置编辑这些传统能力仍然完整存在。
- AI 能力不会直接侵入主进程，而是通过独立的 Node.js sidecar 接入。
- 前端不用直接接触 Mihomo HTTP API，也不用自己处理敏感配置落盘。

这也是 ClashMind 与“单纯把 Mihomo API 包一层 UI”的客户端最大的不同。

## 项目定位

ClashMind 的定位可以用一句话概括：

> 一个带 AI 工作流的 Mihomo 桌面控制台。

这里的“控制台”包含四层含义。

第一层是基础运维能力。

你可以查看代理组、切换节点、测速、关闭连接、看规则、看日志、切系统代理。

第二层是配置管理能力。

你可以直接读写本地 `config.yaml`，通过 Monaco Editor 编辑 YAML，并在保存后触发 Mihomo 热重载。

第三层是数据观察能力。

项目把连接、域名、规则、流量趋势和地理信息整理进 SQLite，既能显示实时数据，也能做历史聚合。

第四层才是差异化最明显的 AI 能力。

AI 不只是一个聊天框，而是能读取上下文、调用工具、生成配置变更预览、等待确认、再由 Rust 侧正式应用的受控工作流。

## 核心能力

### 代理与连接管理

ClashMind 保留了 Mihomo 客户端最常用的一组能力：

- 读取代理组和节点。
- 切换指定代理组的当前节点。
- 对单节点或整组节点做延迟测试。
- 观察当前连接和总上传下载量。
- 关闭单个连接或全部连接。

这些能力在前端被组织成独立的 feature 模块，在 Rust 侧再统一转发到 Mihomo API。

### 规则、日志与配置编辑

对于需要精细操作代理规则的用户，ClashMind 没有把能力简化成“只能导入订阅”。

它保留了更接近工程工具的交互：

- 查看路由规则并过滤。
- 订阅实时日志流。
- 编辑运行配置文件。
- 保存文件并触发热重载。
- 在 AI 改动前创建快照，必要时回滚。

这意味着它既能服务普通用户，也能服务经常手改 YAML 的进阶用户。

### 统计与数据洞察

项目并不只关心“现在这个节点能不能用”。

它还会把连接、规则命中、域名访问、流量变化和 IP 维度的统计保存下来，让你能回答更多问题：

- 最近七天最耗流量的域名是什么。
- 规则命中里 `MATCH` 的兜底比例是不是过高。
- 某段时间连接数和流量峰值出现在什么时候。
- 某些国家或地区的流量占比是否异常。

这些统计既可以直接给 UI 页面使用，也可以成为 AI 生成报告的上下文输入。

### AI 驱动的配置工作流

ClashMind 当前最有辨识度的地方，是把 AI 当作一个有边界的操作代理，而不是一个只会“建议你怎么改”的文本机器人。

AI sidecar 可以：

- 读取当前配置快照。
- 获取代理、连接和统计摘要。
- 通过 Function Calling 生成结构化操作。
- 产出配置 diff 预览。
- 等待用户确认。
- 由 Rust 侧正式写入配置、热重载并记录快照。

这让“自然语言修改 Mihomo 配置”从一个高风险动作，变成一个有校验、有预览、有回滚的流程。

## 技术栈总览

ClashMind 的技术栈有明显的分层设计。

| 层级 | 当前技术 | 作用 |
|------|------|------|
| 桌面壳 | Tauri 2 | 打包 WebView、暴露 IPC、管理 sidecar 和系统能力 |
| 前端 | React 19 + TypeScript 5 + Vite 6 | 页面、交互、状态同步 |
| 样式 | Tailwind CSS 4 | 原子类样式与主题 |
| UI 组件 | Radix UI + lucide-react + framer-motion | 交互、图标和动画 |
| 服务端状态 | TanStack Query 5 | 管理 IPC 请求和缓存 |
| 客户端状态 | Zustand 5 | 管主题、页面、侧边栏和 AI 会话状态 |
| Rust 后端 | tokio + reqwest + serde + thiserror | Mihomo API、文件操作、错误处理 |
| 数据层 | SQLite via tauri-plugin-sql | 连接、流量、域名、规则、快照、会话 |
| AI sidecar | Node.js 22 + AI SDK + Zod + esbuild | 模型接入、工具调用、安全校验 |

这里有两个值得先记住的工程取舍。

第一个取舍是“前端不直连 Mihomo”。

这样虽然多了一层 Rust 中转，但安全边界、配置一致性和平台能力都更清晰。

第二个取舍是“AI 用独立 sidecar，而不是直接塞进 Tauri 主进程”。

这样模型 SDK、流式处理和工具调用逻辑可以留在更适合它们的 Node.js 运行时里。

## 与其他 Mihomo 客户端的差异

ClashMind 并不是把 Mihomo 客户端常见能力重新实现一遍就结束了。

更准确地说，它是在成熟的桌面代理客户端思路上，把 AI 工作流和数据分析能力做成一等公民。

| 维度 | 传统 Mihomo GUI 客户端 | ClashMind |
|------|------|------|
| 代理切换 | 通常具备 | 具备 |
| 连接与日志 | 通常具备 | 具备，并继续向统计链路沉淀 |
| YAML 编辑 | 有些支持 | 直接支持，并和热重载、快照配合 |
| 数据统计 | 常常较浅 | 有 SQLite 持久化和聚合查询 |
| AI 助手 | 通常没有，或只是外部文案助手 | 内置 AI sidecar，可走工具调用 |
| 配置修改安全 | 多数依赖用户自行检查 | Schema 校验、脱敏、Diff 预览、确认后应用 |
| 架构重点 | UI 壳 + Mihomo 集成 | UI + Rust 中控 + AI 工作流 |

这并不意味着 ClashMind 要取代所有传统客户端。

更准确的说法是：

- 如果你只需要一个轻量 UI，很多现有客户端已经足够。
- 如果你需要在桌面环境里把“代理控制、配置维护、统计分析、AI 辅助”连成一个闭环，ClashMind 的目标更贴近这个场景。

## 仓库里的能力分布

从目录划分上看，项目不是按技术栈切得很散，而是按边界清晰地分成三块。

| 目录 | 主要职责 | 典型内容 |
|------|------|------|
| `src/` | React 前端 | 页面、hooks、UI 组件、状态管理 |
| `src-tauri/src/` | Rust 后端 | IPC 命令、sidecar 管理、数据库、Mihomo 客户端 |
| `ai-service/src/` | Node.js AI sidecar | JSON-RPC、Provider 适配、工具集、安全层 |

这三块之间不是随意互调，而是有明确流向：

1. 前端通过 `invoke` 请求 Rust 命令。
2. Rust 通过 HTTP、WebSocket 或 stdin/stdout 与外部进程通信。
3. AI sidecar 再通过回调请求 Rust 读取配置、查询统计或执行受控操作。

如果你想快速感受这个边界，可以直接看前端统一 API 封装。

下面这段来自真实代码，展示了前端如何把 Mihomo、AI、统计、配置和系统能力收敛到一个对象里：

```ts
export const api = {
  mihomo: {
    start: (configPath: string) => invoke("start_mihomo", { configPath }),
    stop: () => invoke("stop_mihomo"),
    restart: (configPath: string) => invoke("restart_mihomo", { configPath }),
  },
  ai: {
    start: () => invoke("start_ai_service"),
    stop: () => invoke("stop_ai_service"),
    status: () => invoke<boolean>("get_ai_status"),
    chat: (params: AiChatParams) => invoke("ai_chat", { params }),
    generateReport: (type, date, settings) =>
      invoke<ReportResult>("ai_generate_report", { reportType: type, date, settings }),
  },
  proxy: {
    getAll: () => invoke<ProxiesResponse>("get_proxies"),
    switch: (group: string, name: string) => invoke("switch_proxy", { group, name }),
  },
} as const;
```

这段代码背后的含义很重要。

前端并不知道 Mihomo 的 HTTP 地址细节，也不直接知道 AI sidecar 的进程通信格式。

它只知道“我需要一个代理列表”或“我需要发送一次 AI 对话请求”，其余边界由 Rust 和 Node.js 侧处理。

## 当前阶段与预期

仓库目前仍处于早期版本阶段。

这意味着两件事同时成立：

- 已经有一套相对完整的工程结构和真实实现。
- 仍然有一部分功能在持续演进，尤其是 AI 相关交互、文档和后续体验打磨。

因此阅读项目时，建议把它理解为“一个已经跑通主路径、但还在快速演进中的桌面应用”。

对贡献者来说，这通常是好事：

- 架构边界已经建立，容易判断改动应该落在哪一层。
- 还有不少可以继续补强的空间，特别是文档、可观测性、异常处理和体验细节。

:::warning 早期阶段意味着接口仍可能演进
如果你准备基于当前实现继续开发，请把源码视为最高优先级事实来源。

像 `DEVELOPMENT.md` 这类设计文档更适合作为背景和意图说明。

当设计文档与代码存在差异时，应以当前仓库实际实现为准，再回头修正文档。
:::

## 适合谁使用

ClashMind 目前最适合三类读者和使用者。

第一类是有 Mihomo 使用经验、但希望桌面体验更完整的用户。

第二类是愿意直接编辑 YAML，希望同时拥有日志、连接和统计视图的进阶用户。

第三类是对“AI 如何安全参与本地代理配置管理”这个命题本身感兴趣的开发者。

如果你属于第三类，建议把 [AI Sidecar](./ai-service.md) 和 [开发指南](./development.md) 一起阅读。

## 建议阅读路径

如果你已经理解了“这个项目在做什么”，接下来可以按目标继续：

- 想本地启动应用：看 [快速开始](./quickstart.md)
- 想理解模块职责：看 [系统架构](./architecture.md)
- 想理解模型调用、工具和安全链路：看 [AI Sidecar](./ai-service.md)
- 想开始参与提交：看 [开发指南](./development.md)
- 想继续查接口级细节：后续可结合即将补充的 API 参考页一起阅读

读到这里，你应该已经有一个足够清晰的心智模型：

ClashMind 不是“加了聊天框的 Mihomo UI”。

它更像是一个把代理管理、配置维护、统计分析和 AI 工作流组合在一起的桌面控制台。
