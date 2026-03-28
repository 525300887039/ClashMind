<div align="center">

# ClashMind

[![License](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Version](https://img.shields.io/badge/Version-0.1.0-green.svg)](./package.json)
[![Tauri](https://img.shields.io/badge/Tauri-2-FFC131?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=white)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.80+-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)

🧠 AI 智能化 Mihomo 代理管理桌面客户端 ⚡

</div>

ClashMind 是一款基于 Tauri 2 构建的跨平台桌面代理管理工具，以 Mihomo 内核为驱动，提供直观的代理切换、实时流量监控、配置编辑等功能。前端采用 React 19 + TypeScript，后端采用 Rust，兼顾性能与开发体验。

> **Note**: 项目处于早期开发阶段 (v0.1.0)，功能持续迭代中。

## ✨ 功能特性

- 🌐 **代理管理** — 查看代理组/节点、一键切换、单节点/批量延迟测试
- 🔗 **连接管理** — 实时连接列表（1s 刷新），关闭单个/全部连接
- 🛡️ **规则查看** — 查看所有代理路由规则，支持搜索过滤
- 📜 **日志查看** — WebSocket 实时日志流，按级别过滤、搜索、暂停/恢复
- 📝 **配置编辑** — Monaco Editor YAML 编辑，保存 & 热重载
- ⚙️ **系统设置** — Mihomo 配置目录、API 地址/密钥、端口、开机启动、语言、主题
- 🖥️ **系统代理** — 一键开启/关闭系统代理
- 📊 **流量监控** — WebSocket 实时上传/下载速度显示
- 🎨 **主题切换** — 跟随系统的深色/浅色主题，支持手动切换
- 🗂️ **系统托盘** — 模式切换（规则/全局/直连）、系统代理开关、显示/退出

## 🛠️ 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri 2 |
| 前端 | React 19 + TypeScript 5 + Vite 6 |
| 样式 | Tailwind CSS 4 |
| 状态管理 | TanStack Query 5（服务端状态）+ Zustand 5（客户端状态） |
| UI 组件 | Radix UI + lucide-react + framer-motion |
| 代码编辑器 | Monaco Editor |
| 国际化 | i18next + react-i18next |
| Rust 后端 | tokio + reqwest + tokio-tungstenite + serde + thiserror |
| 系统代理 | sysproxy |

## 🚀 快速开始

### 环境要求

- [Node.js](https://nodejs.org/) >= 18
- [npm](https://www.npmjs.com/) >= 10 (Node.js 自带)
- [Rust](https://www.rust-lang.org/tools/install) >= 1.80
- Tauri 2 系统依赖（参考 [Tauri 官方文档](https://v2.tauri.app/start/prerequisites/)）
- [Mihomo](https://github.com/MetaCubeX/mihomo) 内核二进制文件

### 安装 & 运行

```bash
# 克隆仓库
git clone https://github.com/525300887039/ClashMind.git
cd ClashMind

# 安装依赖
npm install

# 下载 mihomo 核心（首次必须）
bash scripts/download-mihomo.sh

# 启动开发模式
npm run tauri dev

# 构建生产版本
npm run tauri build
```

### 其他命令

```bash
npm run lint          # ESLint 检查
npm run type-check    # TypeScript 类型检查
```

## 📁 项目结构

```
ClashMind/
├── src/                        # 前端源码
│   ├── components/layout/      # 布局组件（Header、Sidebar、AppLayout）
│   ├── features/               # 功能模块
│   │   ├── proxy/              # 代理管理
│   │   ├── connections/        # 连接管理
│   │   ├── rules/              # 规则查看
│   │   ├── logs/               # 日志查看
│   │   ├── config/             # 配置编辑
│   │   ├── settings/           # 系统设置
│   │   └── traffic/            # 流量监控
│   ├── hooks/                  # 全局 Hooks
│   ├── lib/                    # 工具函数、常量、Tauri API 封装
│   └── stores/                 # Zustand 状态管理
├── src-tauri/                  # Rust 后端
│   └── src/
│       ├── cmd/                # IPC 命令（proxy、config、system、sidecar）
│       ├── core/               # 核心逻辑（Mihomo API、Sidecar、系统代理、流量、日志）
│       └── tray.rs             # 系统托盘
└── package.json
```

## 🏗️ 架构说明

- 前端**不直连** Mihomo API，所有通信经 Rust IPC 中转，确保安全性与一致性
- 实时数据（流量、日志）通过 WebSocket 订阅，支持自动重连与指数退避
- 前端状态分层：TanStack Query 管理服务端数据，Zustand 管理客户端 UI 状态

## 📄 开源协议

[MIT](./LICENSE)
