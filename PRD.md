# 智能 AI 桌宠 — 产品需求文档 (PRD)

> 版本: 1.0.0 | 日期: 2026-05-16 | 状态: 已确认

---

## 1. 产品概述

一款以情感陪伴为主的 Tauri 桌面 AI 桌宠。透明悬浮窗浮于桌面，感知用户活动状态并自动切换动画，支持 AI 对话与多模态文件理解。用户可自行替换角色逐帧动画 PNG 文件夹即可换宠。

### 1.1 交互规则

| 操作 | 行为 |
|------|------|
| 单击宠物 | 触发 CLICKED 卖萌反应，播完回到之前状态 |
| 双击宠物 | 打开对话面板（此时不切状态，用户发送消息后才进入 THINK） |
| 拖拽宠物 | 宠物切为 RAISED 视觉（缩小+阴影），移动到屏幕任意位置，释放后恢复原状态并保存坐标 |
| 右键宠物 | 功能菜单（状态切换 / 设置 / 退出） |
| 系统状态触发 | 宠物自动切换动画 + 可能弹出预置文字气泡 |

---

## 2. 技术栈

| 层 | 技术 |
|---|------|
| 桌面框架 | Tauri v2 |
| 前端 | Vue 3 + Vite |
| 后端 | Rust |
| AI | Kimi / DeepSeek / 通义千问（Adapter 层可切换） |
| 动画 | 逐帧 PNG 文件序列（每帧 1000×1000 PNG），JS 引擎驱动，支持每帧不等长时长 |
| 存储 | 本地 JSON（配置）+ 本地 SQLite（对话历史，可选） |

---

## 3. 角色文件规格

用户替换角色文件夹中的逐帧 PNG 子文件夹 + config.toml 即可换宠。

### 3.1 文件结构

每个状态对应一个**子文件夹**，文件夹内是按顺序命名的 PNG 帧。这种结构与 VPet 的素材目录直接兼容，复制粘贴即可使用。

```
角色文件夹/
  config.toml
  bubbles.json              ← 自动气泡文案
  appear/                   ← 启动入场（一次性）
    _000_125.png
    _001_125.png
    ...
  idle/                     ← 默认呼吸（循环）
    _000_125.png
    ...
  idle_var1/                ← Idle 变体：喵叫
  idle_var2/                ← Idle 变体：吹泡泡
  idle_var3/                ← Idle 变体：蹲下眨眼
  idle_var4/                ← Idle 变体：无聊打呼
  idle_var5/                ← Idle 变体：自娱自乐
  clicked/                  ← 点击反应（一次性）
  think/                    ← 思考中（循环，等待 LLM）
  talk/                     ← 说话中（循环，流式输出）
  sleeping/                 ← 睡觉（循环）
  typing/                   ← 看用户打字（循环）
  worried/                  ← 深夜提醒（循环）
  sweating/                 ← CPU 告警（循环）
  shutdown/                 ← 退出动画（一次性）
  raised/                   ← 拖拽被拎起（循环，可选）
```

### 3.2 帧文件命名约定

每个 PNG 文件名遵循 VPet 风格：

```
<前缀>_<帧序号>_<持续毫秒>.png
```

- **前缀**：任意字符（含中文），仅用于人类阅读，引擎不解析
- **帧序号**：3 位数字（`000`、`001`...），决定播放顺序，按字典序排序
- **持续毫秒**：该帧停留的毫秒数（`125`、`250`、`500`...），不同帧可不同
- **示例**：`启动动画二_000_125.png`、`摸头_005_250.png`、`_007_375.png`

> **为什么不等长**：眨眼、点头等关键帧需要更长停留（如 500ms），中间过渡帧短停（125ms）能让动作更自然。统一 fps 会丢失这种节奏感。

### 3.3 config.toml 格式

每个状态只声明文件夹路径（相对角色文件夹），引擎自动扫描文件夹内的 PNG。

```toml
[appear]
folder = "appear"
loop = false

[idle]
folder = "idle"
loop = true
# Idle 子变体池（文件夹名列表），前端 20-60s 间隔随机切换
# 变体切换属于同一 IDLE 状态内换图，不动状态机
variants = ["idle_var1", "idle_var2", "idle_var3", "idle_var4", "idle_var5"]

[clicked]
folder = "clicked"
loop = false

[think]
folder = "think"
loop = true

[talk]
folder = "talk"
loop = true

[sleeping]
folder = "sleeping"
loop = true

[typing]
folder = "typing"
loop = true

[worried]
folder = "worried"
loop = true

[sweating]
folder = "sweating"
loop = true

[shutdown]
folder = "shutdown"
loop = false

[raised]
folder = "raised"
loop = true
# 可选：缺则拖拽时用 CSS transform: scale(0.95) + drop-shadow
```

### 3.4 约束

- **每帧固定 1000×1000 PNG**（带 Alpha 透明通道），渲染时由 CSS 缩放到设置的宠物大小
- 文件夹名与 config.toml 中 `folder` 声明完全一致（区分大小写）
- 帧序号从 `000` 起，无需连续起始（但建议连续以便维护）
- 文件名末尾的毫秒数缺省时按 125ms 处理
- `raised/` 文件夹为**可选**：config.toml 中可省略 `[raised]` 段，拖拽时自动降级为 CSS 视觉效果
- `loop=false` 的动画播完后，由状态机决定下一步：APPEAR 播完转 IDLE，CLICKED 播完回落到点击前的状态，SHUTDOWN 播完关闭窗口。仅当没有任何后续状态变更时（理论上不会发生），才会停在最后一帧。

---

## 4. 宠物状态定义

### 4.1 状态优先级

数字越小优先级越高（1 = 最高）。优先级高的状态可抢占优先级低的状态。

```
 1. SHUTDOWN   → 用户退出（瞬态，播完关窗，不可被抢占）
 2. CLICKED    → 播完回落原状态（瞬态，不可被抢占）
 3. THINK      → 等待 LLM 回复中（protected，只在收到首 token 后让位给 TALK）
 4. TALK       → LLM 流式输出中（protected，只在流结束后让位）
 5. APPEAR     → 仅启动时，播完回落（瞬态，不可被抢占）
 6. WORRIED    → 深夜提醒（深夜场景下抑制 TYPING）
 7. SLEEPING   → 空闲打盹
 8. SWEATING   → CPU 告警
 9. TYPING     → 看用户打字
10. IDLE       → 默认兜底
```

> **说明**：
> - 当深夜（23:00–05:00）键盘活跃时，按上面的优先级会进入 WORRIED 而不是 TYPING——这是有意设计，深夜场景下"提醒休息"比"陪你打字"更重要。
> - THINK → TALK 的切换不由优先级驱动：THINK 状态下收到首 token 后，由 llm 模块直接置位 is_talking、清 is_thinking，状态机下一 tick 自动切到 TALK。用户关闭对话面板时由 `end_talking` 命令统一清除 is_talking/is_thinking。
> - RAISED 不是独立状态，是**拖拽操作期间** Vue 侧的纯视觉反馈（CSS 缩放 + 阴影，或切 raised.png），不经过 Rust 状态机。

### 4.2 状态表

| 状态 | 触发条件 | 退出条件 | 动画 | loop |
|------|---------|---------|------|------|
| **APPEAR** | 应用启动 | 播完自动转 IDLE | 入场跳入/淡入/破壳 | false |
| **IDLE** | 默认状态 / 其他状态自然结束 | 被其他状态抢占 | 呼吸微动，闲时随机切子变体（喵叫/吹泡泡/蹲下/无聊/自娱自乐） | true |
| **CLICKED** | 单击宠物 | 播完回原状态 | 弹跳/爱心/星星眼 | false |
| **THINK** | 发送 LLM 消息后自动进入 | 收到首 token → TALK；用户关闭对话面板 → IDLE | 歪头/眨眼/摸下巴思考中 | true |
| **TALK** | 收到 LLM 首 token 自动从 THINK 切换 | 流式输出结束 → IDLE；用户关闭对话面板 → IDLE | 嘴动+手势，说话中 | true |
| **SLEEPING** | 键鼠空闲 ≥ 10 分钟 | 检测到键鼠输入 | 趴下睡，Zzz | true |
| **TYPING** | 2 秒内键盘事件 ≥ 5 次 | 按键频率 < 阈值持续 10 秒 | 看书/侧头看屏幕 | true |
| **WORRIED** | 23:00-05:00 + 键盘活跃（最近 2 秒键盘事件 ≥ 5） | 键鼠空闲 ≥ 10 分钟 或 时间进入 05:00–23:00（退出后由状态机重新评估，可能直接进 SLEEPING） | 打哈欠/揉眼 | true |
| **SWEATING** | CPU ≥ 90% 持续 30 秒 | CPU 降至 70% 以下持续 10 秒 | 生病不适/虚弱 | true |
| **SHUTDOWN** | 用户点退出 | 播完关闭进程 | 渐变消失/挥手告别 | false |
| **RAISED** | 鼠标按住拖拽时 | 释放鼠标 | 被拎起/缩紧（可选，缺则 CSS scale 0.95 + drop-shadow） | true |

> **RAISED 不是状态机状态**：它是拖拽事件期间的纯视觉反馈。Vue 侧在 mousedown / touchstart 时直接切图或加 CSS class，mouseup 时恢复。Rust 状态机不感知它。列在此表仅为文档完整性。

---

## 5. 功能需求

### 5.1 窗口与外观

- 透明悬浮窗、无边框、无标题栏、置于顶层
- 宠物区域可点击，其余区域穿透到桌面
- 不在任务栏显示、不在 Alt+Tab 列表
- 宠物默认大小 128×128，设置中可调（96 / 128 / 192 px）。原始素材为 1000×1000，由 CSS `width/height` 缩放到目标尺寸
- 系统托盘图标始终可见

### 5.2 动画引擎

- JS 实现，基于递归 `setTimeout` 按帧调度（每帧持续时长可不同）
- 读取 config.toml，每个状态对应一个 PNG 帧文件夹
- 切换帧时直接换 `<img>` 元素的 `src`（指向预加载的 blob URL）
- 状态切换时停止当前调度、重置帧序号、加载新文件夹
- 启动时预加载所有状态的所有帧（以 blob URL 形式缓存，保持 PNG 压缩格式，不解码为 RGBA，避免内存爆炸）
- 渲染时由 CSS 把 1000×1000 的原图缩放到设置的宠物大小（96/128/192 px）
- **Idle 子变体**：IDLE 状态下，前端每 20-60 秒随机间隔，从 `idle.variants` 列表中随机选一个文件夹替换当前的 idle 动画，不触发状态机切换。主 idle 动画占 70% 时间，变体共占剩余 30%
- **raised 可选**：若角色文件夹中没有 `raised/` 子目录，拖拽时自动降级为 CSS `scale(0.95) + drop-shadow`

### 5.3 状态机（Rust 侧）

- 10 个状态（含 SHUTDOWN、THINK、TALK），按优先级决策
- THINK / TALK 通过 `Arc<AtomicBool>` 跨线程标记，由 LLM 模块写入、状态机 tick 读取，不参与优先级抢占
- 系统活动检测结果 → 状态机决策 → Tauri event 推给 Vue
- 演示模式下强制切状态，恢复后状态机重新接管

### 5.4 系统活动检测（Rust 侧）

| 检测项 | 实现 |
|--------|------|
| 键鼠空闲 | Win32 `GetLastInputInfo()` |
| 键盘频率 | `WH_KEYBOARD_LL` 低级别钩子（仅统计频率，不记录内容） |
| 前台进程名 | Win32 枚举前台窗口进程 |
| 当前时间 | `chrono` crate |
| CPU 使用率 | Windows 性能计数器 |

### 5.5 AI 对话

**Adapter 层**（Rust trait + `reqwest` HTTP 调用）：

```rust
trait LlmProvider {
    async fn chat_stream(&self, messages: Vec<ChatMessage>,
        app_handle: AppHandle) -> Result<(), String>;
    async fn chat_stream_with_images(&self, messages: Vec<ChatMessage>,
        images: Vec<ImageAttachment>, app_handle: AppHandle) -> Result<(), String>;
}
// 流式响应通过 emit pet-chat-chunk / pet-chat-done / pet-chat-error 推给前端
```

- 预设 3 个 Provider：Kimi (Moonshot)、DeepSeek、通义千问
- 用户在设置中选 Provider、填 API Key、选模型
- 流式输出到 Vue，打字机效果
- 多轮对话上下文在 Rust 侧维护
- **THINK → TALK 时序**：用户发送消息 → 状态机切 THINK → 首个 chunk 到达 → 状态机切 TALK → 流式输出 → 流结束 → 切回 IDLE

**API 请求格式参考**：

Kimi 兼容 OpenAI 格式：
```
POST https://api.moonshot.cn/v1/chat/completions
{
  "model": "moonshot-v1-8k",
  "messages": [
    { "role": "user", "content": "你好" }
  ],
  "stream": true
}
```

DeepSeek 格式：
```
POST https://api.deepseek.com/v1/chat/completions
{
  "model": "deepseek-chat",
  "messages": [...],
  "stream": true
}
```

多模态请求时，content 改为数组：
```json
{
  "role": "user",
  "content": [
    { "type": "text", "text": "这张图片里有什么" },
    { "type": "image_url", "image_url": { "url": "data:image/png;base64,iVBOR..." } }
  ]
}
```

### 5.6 文件处理

用户可将文件拖入聊天面板，Rust 负责提取内容后拼进 prompt。

| 文件类型 | 处理方式 | 依赖 crate |
|---------|---------|-----------|
| 纯文本 (.txt/.py/.js/.json/.md/.html 等) | 直接 UTF-8 读取文件内容 | 标准库 |
| PDF | 提取文本 | `pdf-extract` |
| DOCX | 解压 ZIP → 解析 XML 提取文本 | `zip` + `quick-xml` |
| PPTX | 同上，提取幻灯片中的文本 | `zip` + `quick-xml` |
| 图片 (.png/.jpg/.webp/.gif) | 读取文件 → base64 编码 → 发给多模态 API | `base64` |
| 其他未知类型 | 提示"不支持的文件格式" | - |

流程：
```
用户拖文件进对话框
  → Vue 通过 Tauri drag-drop event 拿到文件路径
  → invoke('process_file', path)
  → Rust 判断扩展名 → 提取文本 / 转 base64
  → 返回 { file_type, content_text_or_base64 }
  → Vue 把内容拼入 chat 请求
  → Rust → LLM API → 流式返回
```

### 5.7 自动气泡

> "冷却"指同一种气泡两次触发之间的最短间隔；"显示时长"是单次气泡在屏幕上停留的时间（默认 3 秒，由 `bubbles.json` 中的 `duration_ms` 控制）。

| 状态 | 触发时机 | 文案示例 | 冷却 |
|------|---------|---------|------|
| APPEAR | 进入状态 | "嘿！我来了~" / "早上好呀！" / "又见面了！" | 无 |
| THINK | 进入状态 | "嗯..." / "让我想想..." / "等一下哦~" | 无 |
| SLEEPING | 进入状态 | "Zzz..." / "好困，先睡一会儿..." / "呼——" | 5 分钟 |
| SLEEPING→IDLE | 回落（用户回来） | "你回来了！" / "睡醒了吗？" / "嗯？怎么了？" | 3 分钟 |
| TYPING | 进入状态 | "你在忙什么？" / "键盘敲得好快..." / "在写什么好东西？" | 10 分钟 |
| WORRIED | 进入状态 | "已经{time}了，还不睡吗？" / "熬夜会长黑眼圈..." / "该休息啦！" | 10 分钟 |
| SWEATING | 进入状态 | "好热...你的电脑在冒烟！" / "CPU 要炸了！" / "这是在编译什么怪物..." | 5 分钟 |
| CLICKED→原状态 | 回落 | "嘻嘻~" / "痒！" / "别闹~" | 无 |
| SHUTDOWN | 进入状态 | "拜拜~" / "下次见！" / "不要忘了我哦..." | 无 |

- 支持变量 `{time}` `{pet_name}`
- 文案存 `bubbles.json`，用户可自定义
- CSS 淡入 → 停留 → 淡出

### 5.8 设置面板

右键菜单 → 设置：

| 设置项 | 说明 |
|--------|------|
| 宠物名字 | 自定义，气泡变量 `{pet_name}` 用 |
| Provider 选择 | Kimi / DeepSeek / 通义千问 |
| API Key | 每个 Provider 独立保存。切 Provider 时自动加载该 Provider 已保存的 Key，避免反复重填 |
| 模型选择 | 下拉（如 moonshot-v1-8k / deepseek-chat 等） |
| 角色文件夹 | 浏览本地文件夹 |
| 宠物大小 | 96 / 128 / 192 px |
| 开机自启 | checkbox，默认关 |

配置存 `%APPDATA%/desktoppet/config.json`。

### 5.9 演示模式

右键菜单 → 状态切换：

- 下拉选择任意状态，显示触发条件描述
- 强制进入该状态 + 播对应动画 + 触发对应气泡
- 「恢复自动」退出演示，状态机接管

### 5.10 系统托盘

- 托盘图标 + 右键菜单：显示/隐藏宠物、状态切换、设置、退出

---

## 6. 架构设计

### 6.1 通信协议

**Commands（Vue → Rust）**：

| Command | 用途 |
|---------|------|
| `get_pet_state` | 获取当前状态 |
| `get_config` | 读取配置 |
| `save_config` | 保存配置 |
| `save_pet_position(x, y)` | 保存宠物坐标 |
| `notify_click()` | 通知 Rust 用户单击了宠物 → 切 CLICKED |
| `animation_finished(state)` | 瞬态动画播完通知 → 切 IDLE/回落 |
| `start_talking()` | 通知 Rust 对话面板已打开（不切状态，THINK 由 send_chat 触发） |
| `end_talking()` | 通知 Rust 对话面板关闭 → 清除 is_thinking/is_talking → 状态机重新评估 |
| `send_chat(msg, images?, appended_text?)` | 发送对话消息 |
| `process_file(path)` | 处理拖入的文件，返回文本/base64 |
| `force_state(state)` | 演示模式：强制切换 |
| `resume_auto_state` | 恢复自动状态机 |
| `get_all_states` | 获取状态列表+描述 |
| `trigger_shutdown()` | 用户点退出时调用 → Rust 切换到 SHUTDOWN 状态，播完退出动画后关闭进程 |

**Events（Rust → Vue）**：

| Event | payload | 时机 |
|-------|---------|------|
| `pet-state-changed` | `{ state, previous }` | 状态切换 |
| `pet-bubble` | `{ text, duration_ms }` | 自动气泡 |
| `pet-chat-chunk` | `{ delta }` | 流式输出 token |
| `pet-chat-done` | `{}` | 输出完成 |
| `pet-chat-error` | `{ message }` | 出错 |

### 6.2 启动时序

```
App 启动
  → Rust: 加载 config → 状态机初始 APPEAR
  → Vue: invoke get_pet_state → 得到 APPEAR
  → 加载角色 config.toml → 扫描各状态文件夹下的 PNG → 全部预加载为 blob URL → 播 APPEAR
  → APPEAR 播完 → Rust 切 IDLE → emit pet-state-changed
  → Vue 收到 → 切换 IDLE 动画
```

### 6.3 前端目录

```
src/
├── App.vue
├── main.js
├── components/
│   ├── Pet.vue              # 宠物本体（动画渲染 + 交互）
│   ├── PetBubble.vue        # 自动气泡
│   ├── PetChat.vue          # 聊天面板（对话 + 文件拖入）
│   ├── PetMenu.vue          # 右键菜单
│   ├── PetSettings.vue      # 设置面板
│   └── PetDebug.vue         # 演示模式状态切换
├── composables/
│   ├── useAnimation.js      # 逐帧动画引擎
│   ├── usePetState.js       # 状态管理
│   ├── usePetDrag.js        # 拖拽
│   └── useChat.js           # 对话 + 流式渲染
├── assets/
│   └── pets/
│       └── default/         # 默认角色
│           ├── config.toml
│           ├── bubbles.json
│           ├── appear/      # 各状态的逐帧 PNG 子文件夹
│           ├── idle/  idle_var1/ ... idle_var5/
│           ├── clicked/  think/  talk/
│           ├── sleeping/  typing/  worried/  sweating/
│           ├── shutdown/  raised/
└── stores/
    └── petStore.js          # Pinia 全局状态
```

### 6.4 Rust 后端目录

```
src-tauri/src/
├── main.rs
├── lib.rs                   # Tauri 命令注册
├── state_machine/
│   ├── mod.rs               # 状态机核心 + 优先级
│   ├── states.rs            # 状态枚举
│   └── transitions.rs       # 流转规则
├── monitor/
│   ├── mod.rs               # 监测调度
│   ├── input.rs             # 键鼠钩子 + 空闲检测
│   ├── window.rs            # 前台进程检测
│   └── system.rs            # CPU + 时间
├── llm/
│   ├── mod.rs               # Adapter trait + 请求调度
│   ├── providers/
│   │   ├── kimi.rs
│   │   ├── deepseek.rs
│   │   └── qwen.rs
│   └── context.rs           # 多轮对话上下文管理
├── file_handler/
│   ├── mod.rs               # 文件类型判断 + 调度
│   ├── text.rs              # 纯文本读取
│   ├── pdf.rs               # PDF 提取
│   ├── office.rs            # DOCX/PPTX 提取
│   └── image.rs             # 图片 base64 编码
├── bubble/
│   └── mod.rs               # 气泡模板加载 + 变量替换 + 冷却
└── store/
    ├── mod.rs
    └── config.rs            # 配置读写 (JSON)
```

---

## 7. 开发任务

| 编号 | 任务 | 主要依赖 |
|------|------|---------|
| 1 | Tauri 透明悬浮窗 + 置顶 + 无边框 + 穿透 + 跳过任务栏 | — |
| 2 | JS 逐帧动画引擎（config.toml 文件夹扫描 + 文件名解析帧时长 + 递归 setTimeout 调度 + Idle 变体池） | — |
| 3 | 状态机核心（10 状态 + THINK/TALK AtomicBool + 优先级 + Tauri event） | — |
| 4 | 系统活动检测（空闲/键盘/CPU/时间/进程） | — |
| 5 | 单击/双击/拖拽/右键交互（含 Raised CSS 视觉反馈） | 1 |
| 6 | 右键菜单 + 演示模式 + 系统托盘 | 5 |
| 7 | 自动气泡系统（含 THINK/TALK/SHUTDOWN 气泡） | 3 |
| 8 | LLM Adapter 层（至少 1 个 Provider 可用）+ 流式输出 + THINK/TALK 信号 | — |
| 9 | 聊天面板 UI + 流式渲染 + 上下文管理 | 8 |
| 10 | 文件处理（txt/pdf/docx/pptx + 图片 base64）+ 多模态对话 | 9 |
| 11 | 设置面板（Provider/API Key/模型/角色/大小/自启） | — |
| 12 | 默认角色逐帧 PNG 占位图（全部 16 个状态文件夹） | — |
| 13 | Shutdown 退出动画 + 进程关闭 | 1 |
| 14 | 状态机与活动检测联调（4 → 3 输入 SystemSnapshot） | 3, 4 |

> 任务 1/2/3/4/8/11/12/13 之间无依赖，可并行启动。其余任务按"主要依赖"列推进。

---

## 8. 风险

| 风险 | 缓解 |
|------|------|
| 透明窗口穿透与可点击区域冲突 | Tauri 按区域控制穿透，宠物区域不穿透 |
| 键鼠钩子被 AV 误报 | 使用官方 Win32 API，不注入 DLL |
| LLM API 延迟/超时 | 异步调用 + 加载状态 + 超时提示 |
| PDF 文本提取不完整 | `pdf-extract` 覆盖大部分场景，复杂 PDF 降级提示 |
| 长期运行资源占用 | 事件驱动非轮询，动画降频 |

---

## 9. 术语表

| 术语 | 说明 |
|------|------|
| 逐帧 PNG | 每个状态用一个文件夹存放有序 PNG 帧，通过文件名 `_序号_毫秒.png` 控制播放节奏 |
| LLM Adapter | 统一 AI 接口抽象层 |
| 状态机 (FSM) | 管理宠物状态与切换规则，共 10 状态 |
| 瞬态 | `loop=false` 的状态，播完动画后自动退出（APPEAR/CLICKED/SHUTDOWN） |
| THINK→TALK 切换 | LLM 对话的等待-输出两段式流程，由 `Arc<AtomicBool>` 跨线程标记驱动 |
| Idle 子变体 | IDLE 状态下随机切换的备选帧文件夹，不触发状态机状态变更 |
| 自动气泡 | 状态触发、预置文案、非 LLM 生成，默认显示约 3 秒（由 `bubbles.json` 的 `duration_ms` 控制） |
| 演示模式 | 手动强制切换状态，可恢复自动 |
| RAISED | 拖拽操作期间的视觉反馈，不是状态机状态 |

---

## 10. 图片素材映射（VPet → desktoppet）

所有素材取自 VPet 默认角色 `vup`：
`VPet/VPet/VPet-Simulator.Windows/mod/0000_core/pet/vup/`

| desktoppet 状态 | VPet 源路径（基于 vup/） | 说明 |
|---|---|---|
| **APPEAR** | `StartUP/Nomal/` | 启动入场动画 |
| **IDLE 主** | `Default/Happy/1/` | 默认呼吸 |
| **IDLE var1** | `IDEL/Meow/Nomal/1/` | 喵叫 |
| **IDLE var2** | `IDEL/Bubbles/B/` | 吹泡泡 |
| **IDLE var3** | `IDEL/Squat/B_Nomal/1/` | 蹲下眨眼 |
| **IDLE var4** | `IDEL/Boring/B_Nomal/` | 无聊打呼 |
| **IDLE var5** | `IDEL/amusement_B/` | 自娱自乐 |
| **CLICKED** | `Touch_Head/Happy/B/` | 摸头反应循环段 |
| **THINK** | `Think/Nomal/B/` | 思考循环段 |
| **TALK** | `Say/Shining/B_2/` | 说话循环段，闪亮风格 |
| **SLEEPING** | `Sleep/B_Nomal/` | 睡觉循环段 |
| **TYPING** | `WORK/Study/B_1_Nomal/` | 看书=侧头看屏幕姿态 |
| **WORRIED** | `IDEL/yawning/Nomal/` | 打哈欠=深夜提醒 |
| **SWEATING** | `Default/Ill/2/` | 生病不适=CPU 过热 |
| **SHUTDOWN** | `Shutdown/Nomal_1/` | 退出关机动画 |
| **RAISED** | `Raise/Raised_Dynamic/Nomal/1/` | 被拎起动态（可选） |

**使用方式**：直接把上述 VPet 目录**整个文件夹复制**到 desktoppet 角色目录，重命名为 desktoppet 约定的状态名（如 `StartUP/Nomal/` → `appear/`）。无需拼接精灵图，无需修改文件名（VPet 的 `_序号_毫秒.png` 命名与 desktoppet 完全兼容）。在 config.toml 中只需声明各状态对应的文件夹名即可。

---

> 本文档为最终确认版本。功能范围已冻结，开发以此为准。

---

## 附录：实际实现与原始设计的主要差异

本节记录实际开发过程中与原始PRD的主要差异点，供后续维护参考：

### A.1 窗口与布局
- **窗口尺寸**：实际为 600×600（原设计 300×300），为容纳聊天面板等UI组件
- **宠物位置**：位于窗口左侧（left:110px），右侧预留360px给聊天面板
- **面板布局**：聊天面板贴窗口右侧，设置/演示面板居中悬浮（透明背景无遮罩）

### A.2 拖拽实现
- **实现方式**：手动实现（不使用Tauri `startDragging` API）
- **触发机制**：长按180ms或移动超过4px阈值触发拖拽
- **技术细节**：通过 `move_window_to` Rust命令跟随光标移动窗口

### A.3 鼠标穿透
- **实现方式**：Rust光标轮询（50ms间隔）+ 前端面板锁定机制
- **判定逻辑**：检测光标是否在宠物外接矩形内
- **面板锁定**：任意面板打开时通过 `force_block_passthrough` 强制不穿透

### A.4 模型支持
- **DeepSeek**：deepseek-chat, deepseek-reasoner（不支持图片）
- **Kimi**：moonshot-v1-8k/32k/128k, kimi-k2-6（k2-6支持图片）
- **通义千问**：qwen3.6-flash/plus, qwen3-vl-plus/flash（带vl的支持图片）

### A.5 配置存储
- **存储位置**：`%APPDATA%\desktoppet\config.json`
- **安全性**：当前版本API Key明文存储（未加密），后续版本可加入加密

### A.6 气泡定位
- **定位方式**：通过 `getBoundingClientRect()` 实时跟随宠物DOM位置
- **样式**：fixed定位，贴宠物头顶上方8px，白色半透明背景带下箭头尾巴

### A.7 演示模式限制
- **允许切换**：非瞬态状态（IDLE/THINK/TALK/SLEEPING/TYPING/WORRIED/SWEATING）
- **禁止切换**：瞬态状态（APPEAR/CLICKED/SHUTDOWN），避免误触发不可逆操作
