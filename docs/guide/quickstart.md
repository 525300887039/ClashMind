# 快速开始

这一页的目标很简单：

让你在尽量少踩坑的前提下，把 ClashMind 在本地跑起来，并知道哪些命令是在启动前端、哪些命令是在启动完整桌面应用、哪些命令是在准备 AI sidecar 与 Mihomo sidecar。

如果你还不熟悉项目背景，建议先看 [项目概述](./overview.md)。

如果你已经能跑起来、但想理解内部层次，下一页建议看 [系统架构](./architecture.md)。

:::tip 先有一个整体预期
ClashMind 不是纯 Web 项目。

完整开发体验至少会涉及四部分：Vite 前端、Tauri 桌面壳、Rust 后端、sidecar 二进制。

因此“能跑起来”通常不是单条命令的问题，而是环境依赖、二进制准备和脚本职责都要对上。
:::

## 环境要求

根据仓库当前实现和文档任务基线，建议使用以下环境：

| 项目 | 建议版本 | 说明 |
|------|------|------|
| Node.js | 22 LTS | `ai-service` 的构建脚本以 Node 22 为目标 |
| pnpm | 9+ | 根仓库与文档站都使用 pnpm/npm 风格脚本 |
| Rust | 1.80+ | Tauri 2 与后端逻辑依赖 Rust 工具链 |
| Tauri 系统依赖 | 按平台安装 | WebView、打包和桌面壳必须依赖系统组件 |
| Mihomo 二进制 | 与当前平台匹配 | 作为 `src-tauri/binaries/` 下的 sidecar 运行 |

需要额外说明的一点是：

`README.md` 里写的是 `Node.js >= 18`，但较新的开发规划与 `ai-service/build.mjs` 都按 Node 22 设计。

因此这份快速开始文档把 Node 22 作为推荐基线。

:::warning 关于 Node 版本
如果你使用的是 Node 18，纯前端依赖可能仍然能安装，但 AI sidecar 的打包脚本并不是按这个版本设计的。

为了避免构建期和运行期行为不一致，建议直接使用 Node 22 LTS。
:::

## 克隆仓库

先把仓库拉到本地：

```bash
git clone https://github.com/525300887039/ClashMind.git
cd ClashMind
```

如果你只是想浏览文档或前端代码，这一步已经足够。

如果你想跑完整桌面应用，后面还要继续安装依赖和准备 sidecar。

## 安装依赖

在仓库根目录执行：

```bash
pnpm install
```

这一步会安装前端依赖，同时也会为 `ai-service/` 准备构建所需依赖。

你可以从根 `package.json` 里看到当前最关键的一组脚本：

```json
{
  "scripts": {
    "dev": "vite",
    "dev:tauri": "pnpm --filter ai-service build && pnpm dev",
    "build": "tsc -b && vite build",
    "build:tauri": "pnpm --filter ai-service build && pnpm build",
    "tauri": "tauri",
    "lint": "eslint .",
    "type-check": "tsc --noEmit"
  }
}
```

从这里可以直接看出两点：

第一，`dev:tauri` 和 `build:tauri` 会先构建 `ai-service`。

第二，这两个脚本本身并不直接调用 `tauri dev` 或 `tauri build`，而是作为 Tauri 的前置命令使用。

## 准备 Mihomo sidecar

首次开发时，通常还需要把 Mihomo 可执行文件下载到 `src-tauri/binaries/`。

仓库已经提供了一个脚本：

```bash
bash scripts/download-mihomo.sh
```

如果你想指定版本，也可以显式传入 tag：

```bash
bash scripts/download-mihomo.sh v1.19.10
```

脚本的核心逻辑不是简单下载固定文件，而是根据本机系统和架构选择对应产物名称，并把它放到 Tauri sidecar 约定的位置。

下面是脚本中的真实片段：

```bash
case "$os" in
  Linux)
    case "$arch" in
      x86_64)  ASSET="mihomo-linux-amd64";    SIDECAR="mihomo-x86_64-unknown-linux-gnu";    EXT=".gz" ;;
      aarch64) ASSET="mihomo-linux-arm64";     SIDECAR="mihomo-aarch64-unknown-linux-gnu";   EXT=".gz" ;;
    esac
    ;;
  Darwin)
    case "$arch" in
      x86_64)  ASSET="mihomo-darwin-amd64";    SIDECAR="mihomo-x86_64-apple-darwin";         EXT=".gz" ;;
      arm64)   ASSET="mihomo-darwin-arm64";     SIDECAR="mihomo-aarch64-apple-darwin";        EXT=".gz" ;;
    esac
    ;;
  MINGW*|MSYS*|CYGWIN*|Windows_NT)
    case "$arch" in
      x86_64)  ASSET="mihomo-windows-amd64";   SIDECAR="mihomo-x86_64-pc-windows-msvc.exe";  EXT=".zip" ;;
      aarch64) ASSET="mihomo-windows-arm64";    SIDECAR="mihomo-aarch64-pc-windows-msvc.exe"; EXT=".zip" ;;
    esac
    ;;
esac
```

对开发者来说，真正重要的是最后的落点：

- Mihomo 二进制最终会被放到 `src-tauri/binaries/`
- 文件名必须符合 Tauri sidecar 的目标三元组命名规则

## 准备 AI sidecar

AI sidecar 不是一个开发时临时跑的 Node 进程脚本。

当前仓库把它打包成平台相关的单文件可执行程序，再放到 `src-tauri/binaries/` 下，由 Rust 统一启动。

`ai-service/build.mjs` 里的真实输出路径如下：

```js
const outputFile = resolve(
  PROJECT_ROOT,
  "..",
  "src-tauri",
  "binaries",
  `ai-service-${triple}${isWindows ? ".exe" : ""}`,
);
```

这也是为什么根脚本里的 `dev:tauri` 和 `build:tauri` 都先执行：

```bash
pnpm --filter ai-service build
```

如果你忘了这一步，桌面壳起来之后通常会在启动 AI 服务时失败，因为 Tauri 找不到对应的二进制。

## 启动开发环境

这里需要区分两个层次的命令。

### 只启动前端开发服务器并预构建 AI sidecar

```bash
pnpm dev:tauri
```

这个脚本会做两件事：

1. 先构建 `ai-service`
2. 再运行 `vite`

它适合下面这些场景：

- 你只想调试前端页面
- 你暂时不需要桌面壳
- 你先想验证前端依赖是否安装正常

### 启动完整 Tauri 桌面开发模式

```bash
pnpm tauri dev
```

这是更完整、也更符合“桌面应用开发”含义的命令。

原因在于 `src-tauri/tauri.conf.json` 已经把前置命令写进了 Tauri 配置：

```json
"build": {
  "beforeBuildCommand": "pnpm run build:tauri",
  "beforeDevCommand": "pnpm run dev:tauri",
  "frontendDist": "../dist",
  "devUrl": "http://localhost:1420"
}
```

也就是说：

- `pnpm tauri dev` 会先调用 `pnpm run dev:tauri`
- 然后再启动 Tauri 开发壳
- Rust 后端、前端 dev server 和 sidecar 管理都会进入完整链路

如果你的目标是调试 IPC、系统代理、sidecar 生命周期或者 AI 流式事件，请优先使用 `pnpm tauri dev`。

:::tip 两个命令的推荐用法
想快速确认依赖和前端是否正常，用 `pnpm dev:tauri`。

想调试真正的桌面应用路径，用 `pnpm tauri dev`。
:::

## 构建生产版本

构建同样分两个层次理解。

### 只生成前端产物并预构建 AI sidecar

```bash
pnpm build:tauri
```

这个脚本会：

1. 构建 `ai-service`
2. 运行 `tsc -b && vite build`

它会生成 Web 前端构建产物，但不会直接触发 Tauri 打包流程。

### 构建完整桌面安装包

```bash
pnpm tauri build
```

这是生成完整桌面发行包时应该使用的命令。

它会读取 `src-tauri/tauri.conf.json` 中的 `beforeBuildCommand`，先执行 `pnpm run build:tauri`，然后进入 Tauri 自己的打包流程。

如果你只是想确认前端是否能成功编译，`pnpm build:tauri` 就够了。

如果你想得到平台安装包或可执行应用，应该用 `pnpm tauri build`。

## 常用检查命令

日常开发里最常用的辅助命令是这几个：

```bash
pnpm lint
pnpm type-check
```

前者用于 ESLint 检查。

后者用于 TypeScript 类型检查。

如果你改动了 Rust 侧逻辑，通常还应该补跑 Cargo 相关检查，不过这属于 [开发指南](./development.md) 的范围。

## Tauri 如何找到 sidecar

当前仓库把 Mihomo 和 AI service 都当作 external binary 打包：

```json
"bundle": {
  "active": true,
  "targets": "all",
  "externalBin": [
    "binaries/mihomo",
    "binaries/ai-service"
  ]
}
```

这意味着：

- 你不能随便改 sidecar 文件名
- 你不能把二进制放到任意目录
- 你需要让构建产物与当前平台三元组匹配

如果你在 Windows 上拿了 Linux 的 sidecar，或者只生成了 Web bundle 没有生成 sidecar executable，应用启动时就会失败。

## AI 配置的最小可运行条件

即使桌面应用能跑起来，也不代表 AI 功能一定立即可用。

AI 侧至少还依赖：

- 一个可用的 Provider
- 正确的模型名
- 需要时填写 API Key
- `openai_compatible` 场景下额外填写 `Base URL`

这个规则在前端 hook 中是明确写出来的：

```ts
export function providerRequiresApiKey(provider: AiProviderKind) {
  return provider === "openai" || provider === "claude" || provider === "gemini";
}

export function providerRequiresBaseUrl(provider: AiProviderKind) {
  return provider === "openai_compatible";
}
```

所以，如果你在设置页里只选了 `openai_compatible`，但没填 `Base URL`，模型列表自动获取和连接测试都不会正常工作。

## 常见问题

### 1. `pnpm install` 成功了，但 `pnpm tauri dev` 失败

最常见原因有三个：

- Tauri 的系统依赖没装完整
- Mihomo sidecar 不存在或命名不对
- AI sidecar 没有构建到 `src-tauri/binaries/`

优先检查：

1. 是否执行过 `bash scripts/download-mihomo.sh`
2. 是否执行过 `pnpm --filter ai-service build`
3. `src-tauri/binaries/` 下是否真的有当前平台对应的可执行文件

### 2. Windows 上出现 Rust 或链接器相关错误

通常要检查：

- 是否安装了 Rust MSVC toolchain
- 是否安装了 Visual Studio C++ Build Tools
- 是否打开了新的终端会话让环境变量生效

如果你能跑 `cargo --version`，但 Tauri 构建仍失败，问题往往不是 Cargo 本身，而是 C++ 构建链或 WebView 相关依赖。

### 3. Linux 上启动时报 WebKitGTK 或系统库缺失

这类问题通常不在仓库里修，而在系统依赖层解决。

ClashMind 基于 Tauri 2，Linux 需要 WebKitGTK 及相关桌面运行库。

处理方法不是反复删 `node_modules`，而是按 Tauri 官方 prerequisite 文档补齐系统包。

### 4. `pnpm dev:tauri` 启动了，但看不到桌面窗口

这是正常现象。

因为 `pnpm dev:tauri` 只是：

- 预构建 AI sidecar
- 启动 Vite 开发服务器

真正启动桌面窗口的是 `pnpm tauri dev`。

### 5. AI 服务启动失败

优先检查下面几项：

- `src-tauri/binaries/` 是否存在 `ai-service-{triple}`
- 当前系统架构与构建目标是否匹配
- 是否用了太旧的 Node 版本导致 `ai-service` 打包失败

如果你是在 watch 或多次启动场景下遇到问题，也可以留意 `build.mjs` 对“可执行文件被占用”的处理逻辑，尤其是 Windows 上文件锁会更常见。

### 6. 模型列表获取失败

这不一定是网络问题，也可能是配置不完整。

当前实现会根据 Provider 判断是否必须填写 API Key 或 Base URL。

缺了这些字段时，即使页面可打开，也只会回退到内置模型列表，或者直接提示你先补配置。

### 7. 我应该先跑哪个命令

最实用的决策方式是：

- 验证依赖和前端：`pnpm dev:tauri`
- 跑完整桌面应用：`pnpm tauri dev`
- 验证前端构建：`pnpm build:tauri`
- 打桌面包：`pnpm tauri build`

## 下一步

如果你已经能把项目启动起来，下一步通常有三种方向：

- 想理解为什么这些命令是这样分层的，继续看 [系统架构](./architecture.md)
- 想理解 AI sidecar 是怎么通过 JSON-RPC、工具调用和安全层工作的，继续看 [AI Sidecar](./ai-service.md)
- 想直接开始改代码，继续看 [开发指南](./development.md)

到这一步，你应该已经掌握了本地运行的最小闭环：

安装依赖。

准备 Mihomo sidecar。

构建 AI sidecar。

再根据目标选择启动前端或启动完整 Tauri 开发环境。
