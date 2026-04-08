import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'ClashMind 文档',
  description: 'AI 智能化 Mihomo 代理管理桌面客户端',
  lang: 'zh-CN',
  lastUpdated: true,

  themeConfig: {
    nav: [
      { text: '指南', link: '/guide/overview' },
      { text: '快速开始', link: '/guide/quickstart' },
      {
        text: 'GitHub',
        link: 'https://github.com/525300887039/ClashMind'
      }
    ],

    sidebar: [
      {
        text: '概述',
        items: [
          { text: '项目概述', link: '/guide/overview' },
          { text: '快速开始', link: '/guide/quickstart' }
        ]
      },
      {
        text: '用户指南',
        items: [
          { text: '系统架构', link: '/guide/architecture' },
          { text: 'AI Sidecar', link: '/guide/ai-service' },
          { text: '开发指南', link: '/guide/development' }
        ]
      },
      {
        text: 'API 参考',
        items: [
          { text: 'Tauri IPC 命令', link: '/api/tauri-ipc' },
          { text: 'AI 工具', link: '/api/ai-tools' }
        ]
      }
    ],

    socialLinks: [
      {
        icon: 'github',
        link: 'https://github.com/525300887039/ClashMind'
      }
    ],

    editLink: {
      pattern:
        'https://github.com/525300887039/ClashMind/edit/master/docs/:path',
      text: '在 GitHub 上编辑此页'
    },

    outline: {
      label: '页面导航',
      level: [2, 3]
    },

    lastUpdated: {
      text: '最后更新于'
    },

    search: {
      provider: 'local',
      options: {
        translations: {
          button: { buttonText: '搜索文档' },
          modal: {
            noResultsText: '无法找到相关结果',
            resetButtonTitle: '清除查询条件',
            footer: {
              selectText: '选择',
              navigateText: '切换',
              closeText: '关闭'
            }
          }
        }
      }
    },

    docFooter: {
      prev: '上一页',
      next: '下一页'
    },

    footer: {
      message: 'MIT License',
      copyright: 'Copyright ClashMind Contributors'
    }
  }
})
