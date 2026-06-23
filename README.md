# 🐾 DesktopPet — 智能 AI 桌面宠物

> 基于 **Tauri v2 + Vue 3 + Rust** 的桌面 AI 桌宠应用

一款以情感陪伴为主的桌面 AI 宠物。透明悬浮窗浮于桌面，感知用户活动状态并自动切换动画，支持 AI 对话、多模态文件理解与本地知识库问答。用户只需替换角色逐帧动画 PNG 文件夹即可换宠。

---

## ✨ 功能特性

- 🎭 **逐帧动画引擎** — 多状态 PNG 序列帧动画，支持每帧不等长时长，动作节奏自然
- 🤖 **多模型 AI 对话** — 支持 Kimi / DeepSeek / 通义千问，Adapter 层可自由切换
- 🖼️ **多模态理解** — 拖入图片、PDF、DOCX、PPTX 等文件，自动提取内容发给 LLM 分析
- 📚 **RAG 本地知识库** — 指定文件夹即可索引文档，基于私有知识库问答，全程本地处理
- 🎨 **自由换宠** — 替换角色文件夹 + config.toml 即可换宠，兼容 VPet 素材
- 🖱️ **丰富交互** — 单击卖萌、双击对话、拖拽移动、右键菜单、系统托盘
- 💬 **自动气泡** — 状态触发式文字气泡，文案可自定义
- 🌙 **场景感知** — 深夜提醒休息、CPU 过热预警、键盘活跃检测等

---

## 🛠 技术栈

| 层 | 技术 |
|---|------|
| 桌面框架 | [Tauri v2](https://v2.tauri.app/) |
| 前端 | Vue 3 + Vite + Pinia |
| 后端 | Rust |
| AI 对话 | Kimi / DeepSeek / 通义千问（Adapter 层可切换） |
| 向量模型 | ONNX Runtime + all-MiniLM-L6-v2 |
| 动画 | 逐帧 PNG 序列，JS 引擎驱动 |
| 存储 | 本地 JSON 配置 + SQLite（可选） |

---

## 🚀 快速开始

### 环境要求

- **Node.js** ≥ 18
- **Rust** ≥ 1.70
- **Windows 10/11**（透明窗口依赖 Windows API）

### 安装与运行

```bash
# 1. 进入项目目录
cd desktoppet

# 2. 安装前端依赖
npm install

# 3. 开发模式运行
npm run tauri dev

# 4. 生产构建
npm run tauri build
```

> 首次运行时，构建脚本会自动下载 ONNX Runtime DLL 和 Embedding 模型文件。

---

## 📁 项目结构

```
desktoppet-main/
├── PRD.md                              # 产品需求文档
├── TECHNICAL_DESIGN.md                 # 技术设计文档
├── RAG-PRD.md                          # RAG 知识库需求文档
├── RAG_TECHNICAL_DESIGN.md             # RAG 技术设计文档
├── RAG_TROUBLESHOOTING.md              # RAG 故障排除指南
│
└── desktoppet/                         # 应用主目录
    ├── src/                            # Vue 3 前端源码
    │   ├── components/
    │   │   ├── Pet.vue                 # 宠物本体（动画 + 交互）
    │   │   ├── PetBubble.vue           # 自动气泡
    │   │   ├── PetChat.vue             # 聊天面板
    │   │   ├── PetMenu.vue             # 右键菜单
    │   │   ├── PetSettings.vue         # 设置面板
    │   │   └── PetDebug.vue            # 演示模式
    │   ├── composables/
    │   │   ├── useAnimation.js         # 逐帧动画引擎
    │   │   ├── usePetState.js          # 状态管理
    │   │   ├── usePetDrag.js           # 拖拽交互
    │   │   ├── useChat.js              # 对话 + 流式渲染
    │   │   └── usePanelDrag.js         # 面板拖拽
    │   ├── stores/
    │   │   └── petStore.js             # Pinia 全局状态
    │   └── assets/
    │       └── pets/default/           # 默认角色素材
    │           ├── config.toml          # 角色配置
    │           ├── bubbles.json         # 气泡文案
    │           ├── appear/  idle/  idle_var1~5/
    │           ├── clicked/  think/  talk/
    │           ├── sleeping/  typing/  worried/
    │           ├── sweating/  shutdown/  raised/
    │
    └── src-tauri/                      # Rust 后端源码
        ├── src/
        │   ├── main.rs                 # 入口
        │   ├── lib.rs                  # Tauri 命令注册
        │   ├── state_machine/          # 状态机（10 状态 + 优先级）
        │   ├── monitor/                # 系统活动检测（键鼠/CPU/时间）
        │   ├── llm/                    # LLM Adapter + 多 Provider
        │   ├── rag/                    # RAG 知识库索引与检索
        │   ├── file_handler/           # 文件处理（txt/pdf/docx/pptx/img）
        │   ├── bubble/                 # 气泡模板 + 冷却
        │   └── store/                  # 配置读写
        └── models/                     # ONNX 模型文件
```

---

## 🎮 交互说明

| 操作 | 行为 |
|------|------|
| 🖱️ 单击宠物 | 触发卖萌反应（弹跳/爱心/星星眼） |
| 🖱️ 双击宠物 | 打开 AI 对话面板 |
| 🖱️ 拖拽宠物 | 移动到屏幕任意位置 |
| 🖱️ 右键宠物 | 功能菜单（状态切换 / 设置 / 退出） |
| ⌨️ 打字时 | 宠物切到「看用户打字」动画 |
| 😴 空闲 ≥ 10 分钟 | 宠物进入睡觉状态 |
| 🌙 深夜 + 键盘活跃 | 宠物提醒休息 |
| 🔥 CPU ≥ 90% | 宠物表现不适 |

---

## 🐱 换宠指南

1. 准备一个角色素材文件夹，包含各状态的逐帧 PNG 子文件夹和一个 `config.toml`
2. 将文件夹放到 `desktoppet/src/assets/pets/` 下
3. 在设置面板中选择该角色文件夹

### config.toml 示例

```toml
[idle]
folder = "idle"
loop = true
variants = ["idle_var1", "idle_var2"]

[clicked]
folder = "clicked"
loop = false

[talk]
folder = "talk"
loop = true
```

帧文件命名约定：`<前缀>_<帧序号>_<持续毫秒>.png`（如 `动画_000_125.png`），兼容 VPet 素材格式。

---

## ⚙️ 设置项

| 设置 | 说明 |
|------|------|
| 宠物名字 | 自定义，气泡中 `{pet_name}` 会替换 |
| AI Provider | Kimi / DeepSeek / 通义千问 |
| API Key | 每个 Provider 独立保存 |
| 模型选择 | 根据 Provider 动态切换可用模型列表 |
| 角色文件夹 | 浏览本地文件夹换宠 |
| 宠物大小 | 96 / 128 / 192 px |
| 知识库文件夹 | RAG 本地文档索引目录 |
| 开机自启 | 默认关闭 |

---

## 📚 RAG 知识库

支持文档类型：`.txt` `.md` `.py` `.js` `.json` `.html` `.pdf` `.docx` `.pptx`

- 在设置中指定知识库文件夹
- 点击重建索引，系统自动解析所有文档并生成向量索引
- 聊天面板切换到「知识库模式」即可基于私有文档提问
- 全部处理在本地完成，**不上传至任何云端服务**

> 详细说明见 [RAG-PRD.md](RAG-PRD.md) 和 [RAG_TECHNICAL_DESIGN.md](RAG_TECHNICAL_DESIGN.md)

---

## 📄 文档索引

| 文档 | 内容 |
|------|------|
| [PRD.md](PRD.md) | 产品需求文档 — 功能规格、交互规则、状态定义 |
| [TECHNICAL_DESIGN.md](TECHNICAL_DESIGN.md) | 技术设计文档 — 数据结构、架构、通信协议 |
| [RAG-PRD.md](RAG-PRD.md) | RAG 知识库需求文档 |
| [RAG_TECHNICAL_DESIGN.md](RAG_TECHNICAL_DESIGN.md) | RAG 技术实现细节 |
| [RAG_TROUBLESHOOTING.md](RAG_TROUBLESHOOTING.md) | RAG 常见问题排查 |

---

## 📜 许可证

MIT License

---

<p align="center">Made with 🐾 by <a href="https://github.com/luo-hao666">luo-hao666</a></p>
