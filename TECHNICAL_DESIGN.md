# 智能 AI 桌宠 — 技术设计文档

> 版本: 1.0.0 | 日期: 2026-05-16 | 配套 PRD v1.0.0

---

## 1. 数据结构定义

### 1.1 Rust — 状态枚举

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PetState {
    Appear,
    Idle,
    Clicked,
    Think,      // 等待 LLM 回复
    Talk,       // LLM 流式输出中
    Sleeping,
    Typing,
    Worried,
    Sweating,
    Shutdown,   // 退出动画
}

impl PetState {
    /// 仅 loop=false 的状态才能由动画自身退出（瞬态）
    pub fn is_transient(self) -> bool {
        matches!(self, PetState::Appear | PetState::Clicked | PetState::Shutdown)
    }
}
```

> **优先级说明**：状态优先级隐式编码在 `evaluate_state` 的判断顺序里（详见 §2.1），不再单独维护一个 `priority()` 表，避免两份真相不一致。判断顺序对应 PRD §4.1 的优先级数字（1 最高）。

### 1.2 Rust — 检测数据

```rust
#[derive(Debug, Clone, Serialize)]
pub struct SystemSnapshot {
    /// 距最后一次键鼠输入的秒数
    pub idle_seconds: u64,
    /// 最近 2 秒内键盘事件数
    pub recent_key_count: u32,
    /// CPU 使用率 0-100
    pub cpu_percent: f32,
    /// 当地时间的小时 (0-23)
    pub local_hour: u32,
    /// 前台窗口的进程名
    pub foreground_process: String,
}
```

### 1.3 Rust — Tauri Command 参数类型

```rust
// ===== Commands =====

#[tauri::command]
async fn get_pet_state(state: State<'_, AppState>) -> Result<PetState, String>;

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String>;

#[tauri::command]
async fn save_config(state: State<'_, AppState>, config: AppConfig) -> Result<(), String>;

#[tauri::command]
async fn save_pet_position(state: State<'_, AppState>, x: i32, y: i32) -> Result<(), String>;

#[tauri::command]
async fn force_state(state: State<'_, AppState>, target: PetState) -> Result<(), String>;

#[tauri::command]
async fn resume_auto_state(state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
async fn get_all_states() -> Result<Vec<StateInfo>, String>;

#[tauri::command]
async fn send_chat(
    state: State<'_, AppState>,
    message: String,
    images: Option<Vec<ImageAttachment>>,
    appended_text: Option<String>,
) -> Result<(), String>;

#[tauri::command]
async fn process_file(path: String) -> Result<FileContent, String>;

#[tauri::command]
async fn animation_finished(state: State<'_, AppState>, state_name: String) -> Result<(), String>;

#[tauri::command]
async fn notify_click(state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
async fn start_talking(state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
async fn end_talking(state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
async fn trigger_shutdown(state: State<'_, AppState>) -> Result<(), String>;
```

### 1.4 Rust — Event Payload 类型

```rust
#[derive(Clone, Serialize)]
pub struct StateChangedEvent {
    pub state: String,      // 如 "idle"
    pub previous: String,   // 如 "typing"
}

#[derive(Clone, Serialize)]
pub struct BubbleEvent {
    pub text: String,
    pub duration_ms: u64,
}

#[derive(Clone, Serialize)]
pub struct ChatChunkEvent {
    pub delta: String,
}

#[derive(Clone, Serialize)]
pub struct ChatErrorEvent {
    pub message: String,
}
```

### 1.5 Rust — 配置类型

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub provider: String,           // "kimi" | "deepseek" | "qwen"
    /// 每个 Provider 独立保存 API Key，避免切换时丢失
    /// key: provider 名（"kimi" / "deepseek" / "qwen"），value: 已加密的 API Key
    pub api_keys: std::collections::HashMap<String, String>,
    pub model: String,
    pub pet_folder: String,         // 角色文件夹路径
    pub pet_size: u32,              // 96 | 128 | 192
    pub pet_position: Option<Position>,
    pub auto_start: bool,
    pub pet_name: String,           // 宠物名字，气泡变量 {pet_name} 用
}

impl AppConfig {
    /// 取当前 provider 对应的 API Key
    pub fn current_api_key(&self) -> &str {
        self.api_keys.get(&self.provider).map(|s| s.as_str()).unwrap_or("")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}
```

### 1.6 Rust — 文件处理类型

```rust
#[derive(Debug, Serialize)]
pub struct FileContent {
    pub file_type: String,    // "text" | "image"
    pub content: String,      // 文本内容 或 base64
    pub mime: Option<String>, // 图片时: "image/png" 等
    pub filename: String,
}

#[derive(Debug, Deserialize)]
pub struct ImageAttachment {
    pub base64: String,
    pub mime: String,
}
```

### 1.7 Vue — JS 侧关键接口

```typescript
// 单帧（从文件名解析得到）
interface AnimationFrame {
  url: string           // blob URL（预加载后）
  durationMs: number    // 该帧停留毫秒数
}

// 动画配置（从 config.toml + 文件夹扫描合并而来）
interface AnimationConfig {
  key: string           // 状态名，如 "idle"
  folder: string        // 相对角色文件夹的子目录名
  frames: AnimationFrame[]   // 已排序的帧列表
  loop: boolean
  variants?: string[]   // Idle 子变体的 folder 名列表
}

// 气泡配置（从 bubbles.json 解析）
interface BubbleRule {
  state: string
  trigger: 'on_enter' | 'on_return_from'
  text: string[]
  cooldown_seconds: number
  duration_ms: number
}

// Chat 消息
interface ChatMessage {
  role: 'user' | 'assistant'
  content: string
  images?: ImageAttachment[]
}
```

---

## 2. 状态机算法

### 2.1 核心逻辑

> **设计要点**
> 1. 每个 tick 只产出**一个**目标状态，避免同一 tick 内连续两次状态变更引起的闪烁。
> 2. 持续型条件（CPU 高负载持续 30s、TYPING/SWEATING 退出延时）由调用方维护计数器，作为入参传入。
> 3. 优先级隐式编码在判断顺序里，与 PRD §4.1 一致：SHUTDOWN > CLICKED > THINK > TALK > APPEAR > WORRIED > SLEEPING > SWEATING > TYPING > IDLE。
> 4. THINK / TALK 通过 `Arc<AtomicBool>` 跨线程标记（llm 模块写、状态机 tick 读），不参与常规优先级抢占。

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct EvalContext<'a> {
    pub current: PetState,
    pub snapshot: &'a SystemSnapshot,
    /// 演示模式强制状态
    pub force: Option<PetState>,
    /// LLM 等待中（用户已发消息，首 token 未到）
    pub is_thinking: Arc<AtomicBool>,
    /// LLM 流式输出中
    pub is_talking: Arc<AtomicBool>,
    /// CPU ≥ 90% 已持续秒数
    pub cpu_high_seconds: u32,
    /// 当前状态在"退出条件成立"上已持续秒数（仅 TYPING/SWEATING 用）
    pub exit_pending_seconds: u32,
}

/// 每 tick 评估一次，返回下一帧应处于的状态。
/// 同一 tick 内只会返回一个目标状态。
pub fn evaluate_state(ctx: &EvalContext) -> PetState {
    // 1. 演示模式强制覆盖（最高）
    if let Some(forced) = ctx.force {
        return forced;
    }

    // 2. 瞬态保护：APPEAR / CLICKED / SHUTDOWN 由 animation_finished 命令退出
    if ctx.current.is_transient() {
        return ctx.current;
    }

    // 3. THINK 保护（LLM 等待回复中，只在收到首 token 后切到 TALK）
    if ctx.is_thinking.load(Ordering::SeqCst) {
        return PetState::Think;
    }

    // 4. TALK 保护（LLM 流式输出中，只在流结束后退出）
    if ctx.is_talking.load(Ordering::SeqCst) {
        return PetState::Talk;
    }

    let snap = ctx.snapshot;
    let is_late_night = snap.local_hour >= 23 || snap.local_hour < 5;
    let keyboard_active = snap.recent_key_count >= 5;

    // 5. WORRIED：深夜 + 键盘活跃
    if is_late_night && keyboard_active {
        return PetState::Worried;
    }

    // 6. SLEEPING：键鼠空闲 ≥ 10 分钟
    if snap.idle_seconds >= 600 {
        return PetState::Sleeping;
    }

    // 7. SWEATING：CPU ≥ 90% 持续 30 秒
    if snap.cpu_percent >= 90.0 && ctx.cpu_high_seconds >= 30 {
        return PetState::Sweating;
    }
    if ctx.current == PetState::Sweating
        && snap.cpu_percent < 70.0
        && ctx.exit_pending_seconds < 10
    {
        return PetState::Sweating;
    }

    // 8. TYPING：键盘高频
    if keyboard_active {
        return PetState::Typing;
    }
    if ctx.current == PetState::Typing && ctx.exit_pending_seconds < 10 {
        return PetState::Typing;
    }

    // 9. 默认
    PetState::Idle
}
```

### 2.2 退出条件（已合并到 evaluate_state）

之前的 `should_exit` / `fallback_state` 设计存在两个问题：
1. 同一 tick 先跑 `evaluate_state` 又跑 `should_exit`，可能在一个 tick 内连续触发两次状态变更。
2. `fallback_state` 实际只是再调一次 `evaluate_state`，逻辑重复。

新设计把退出延时（TYPING / SWEATING 的 10s 缓冲）作为入参 `exit_pending_seconds` 传给 `evaluate_state`，由主循环维护这个计数器。每 tick 只产出一个目标状态。

各状态的退出语义（实现已落入 §2.1）：

| 状态 | 退出方式 |
|------|---------|
| APPEAR / CLICKED / SHUTDOWN | 由 `animation_finished` 命令显式触发，不参与 tick 评估。SHUTDOWN 播完后关闭进程 |
| THINK | 由 llm 模块清除 `is_thinking`、置位 `is_talking`，下一 tick 切到 TALK；用户关闭面板则清除两个标记 |
| TALK | 流结束或出错时由 llm 模块清除 `is_talking`，下一 tick 重新评估 |
| SLEEPING | 检测到任何键鼠输入（`idle_seconds < 1`）后自然不再被选中 |
| TYPING | 按键频率 < 阈值持续 10 秒（用 `exit_pending_seconds`） |
| WORRIED | 键鼠空闲 ≥ 10 分钟 或 时间进入 05:00–23:00（自然不再被选中） |
| SWEATING | CPU < 70% 持续 10 秒（用 `exit_pending_seconds`） |
| IDLE | 不主动退出，被其他条件抢占时自然替换 |

### 2.3 状态机驱动

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Rust 后端主循环，每 1 秒 tick 一次。
/// 独占线程，不阻塞 Tauri 主线程。
fn state_machine_loop(
    app_handle: tauri::AppHandle,
    is_thinking: Arc<AtomicBool>,
    is_talking: Arc<AtomicBool>,
) {
    let mut current = PetState::Appear;
    let mut prev_state = PetState::Appear; // 用于 CLICKED 回落
    let mut cpu_high_seconds: u32 = 0;
    let mut exit_pending_seconds: u32 = 0;
    let mut force_state: Option<PetState> = None;

    // 发出初始状态
    emit_state_change(&app_handle, current, PetState::Appear);

    loop {
        std::thread::sleep(Duration::from_secs(1));

        let snapshot = capture_system_snapshot();

        // 维护 CPU 高负载持续时间
        if snapshot.cpu_percent >= 90.0 {
            cpu_high_seconds = cpu_high_seconds.saturating_add(1);
        } else {
            cpu_high_seconds = 0;
        }

        // 维护退出延时计数（只对 TYPING / SWEATING 有意义）
        let in_exit_window = match current {
            PetState::Typing => snapshot.recent_key_count < 5,
            PetState::Sweating => snapshot.cpu_percent < 70.0,
            _ => false,
        };
        if in_exit_window {
            exit_pending_seconds = exit_pending_seconds.saturating_add(1);
        } else {
            exit_pending_seconds = 0;
        }

        let ctx = EvalContext {
            current,
            snapshot: &snapshot,
            force: force_state,
            is_thinking: Arc::clone(&is_thinking),
            is_talking: Arc::clone(&is_talking),
            cpu_high_seconds,
            exit_pending_seconds,
        };
        let next = evaluate_state(&ctx);

        if next != current {
            emit_state_change(&app_handle, next, current);
            exit_pending_seconds = 0;
            current = next;
        }
    }
}

/// 由 animation_finished Tauri command 调用
fn handle_animation_finished(state_name: &str, current: &mut PetState,
    prev: &mut PetState, app_handle: &AppHandle) {
    match state_name {
        "appear" if *current == PetState::Appear => {
            emit_state_change(app_handle, PetState::Idle, *current);
            *current = PetState::Idle;
        }
        "clicked" if *current == PetState::Clicked => {
            let restore_to = *prev;
            emit_state_change(app_handle, restore_to, *current);
            *current = restore_to;
        }
        "shutdown" if *current == PetState::Shutdown => {
            // 退出动画播完，关闭应用
            app_handle.exit(0);
        }
        _ => {}
    }
}

/// 由 notify_click Tauri command 调用
fn handle_user_click(current: &mut PetState, prev: &mut PetState,
    app_handle: &AppHandle) {
    if !current.is_transient() && *current != PetState::Think && *current != PetState::Talk {
        *prev = *current;
        emit_state_change(app_handle, PetState::Clicked, *current);
        *current = PetState::Clicked;
    }
}

/// 由 trigger_shutdown Tauri command 调用
/// SHUTDOWN 是不可逆操作，直接切换状态，不经过 evaluate_state
fn handle_trigger_shutdown(current: &mut PetState, app_handle: &AppHandle) {
    emit_state_change(app_handle, PetState::Shutdown, *current);
    *current = PetState::Shutdown;
    // Vue 收到 pet-state-changed 后播 shutdown 动画
    // 动画播完 Vue 调用 animation_finished("shutdown")
    // Rust 收到后调用 app_handle.exit(0)
}
```

---

## 3. 通信协议详解

### 3.1 命令（Vue invoke → Rust）

#### `get_pet_state() -> string`

Vue 启动时调用，获取当前状态。返回 `"appear"` / `"idle"` 等。

#### `get_config() -> AppConfig`

获取完整配置对象。

#### `save_config(AppConfig) -> void`

保存配置到 `%APPDATA%/desktoppet/config.json`。

API Key 写入前做 XOR 简单混淆（Windows 上用 DPAPI `CryptProtectData` 更好）。

#### `save_pet_position(x: i32, y: i32) -> void`

拖拽释放时调用，保存屏幕坐标到 config。

#### `force_state(state: string) -> void`

演示模式用。设置 `force_state`，状态机下次 tick 时强制切换。

#### `resume_auto_state() -> void`

清除 `force_state`，状态机恢复自动决策。

#### `get_all_states() -> Vec<StateInfo>`

```rust
struct StateInfo {
    id: String,        // "sleeping"
    label: String,     // "SLEEPING — 空闲挂机中"
    description: String, // "键鼠空闲 ≥ 10 分钟后触发"
}
```

演示模式下拉菜单用。

#### `animation_finished(state_name: string) -> void`

瞬态动画（APPEAR / CLICKED / SHUTDOWN）播完后由 Vue 调用。Rust 收到后：
- 如果当前状态是 APPEAR → 切换到 IDLE
- 如果当前状态是 CLICKED → 恢复到进入 CLICKED 之前的原状态
- 如果当前状态是 SHUTDOWN → 调用 `app_handle.exit(0)` 关闭进程

#### `notify_click() -> void`

Vue 检测到用户单击宠物时调用。Rust 内部：保存当前状态到 `prev_state`，切换到 CLICKED 并通过 event 通知 Vue。CLICKED 动画播完后 Vue 调用 `animation_finished("clicked")`，Rust 恢复到 `prev_state`。

#### `start_talking() -> void`

聊天面板打开时由 Vue 调用。状态机切换到 IDLE（对话面板打开时宠物不强制进入 THINK——THINK 只在用户实际发送消息后由 llm 模块触发）。

#### `end_talking() -> void`

聊天面板关闭时由 Vue 调用。清除 `is_thinking` 和 `is_talking` 标记，状态机下一 tick 重新评估。

#### `trigger_shutdown() -> void`

用户通过系统托盘或右键菜单点击"退出"时由 Vue 调用。Rust 内部：
1. 将当前状态切换到 `PetState::Shutdown`，并通过 `pet-state-changed` 事件通知 Vue
2. Vue 播放 shutdown 动画，播完后调用 `animation_finished("shutdown")`
3. Rust 收到后调用 `app_handle.exit(0)` 关闭进程

> **为什么不用 `force_state`**：`force_state` 是演示模式专用，会被 `resume_auto_state` 清除。SHUTDOWN 是不可逆操作，需要独立命令保证语义清晰。

#### `send_chat(message: string, images?: ImageAttachment[], appended_text?: string) -> void`

1. 在对话上下文中 push user message
2. 构造 API 请求体
3. 发送 HTTP 请求（流式）
4. 在 stream 循环中 emit `pet-chat-chunk`
5. 最后 emit `pet-chat-done` 或 `pet-chat-error`

```
send_chat 内部流程:

0. 设置 is_thinking = true，is_talking = false（触发 THINK 状态）
1. 从 AppConfig 拿 provider，再从 api_keys[provider] 取对应 Key 和 model
2. match provider:
   "kimi"    → POST https://api.moonshot.cn/v1/chat/completions
   "deepseek"→ POST https://api.deepseek.com/v1/chat/completions
   "qwen"    → POST https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions
3. 构造 messages 数组:
   - system prompt（角色设定）
   - 历史对话（最多保留 N 轮）
   - 当前 user message（含 appended_text + images）
4. reqwest post → SSE streaming
5. 收到首个有内容的 chunk 时：设置 is_thinking = false，is_talking = true（THINK → TALK）
6. 逐 chunk emit pet-chat-chunk
7. 流结束或出错时：设置 is_talking = false（TALK → 下一 tick 评估）
```

#### `process_file(path: string) -> FileContent`

```rust
fn process_file(path: &str) -> Result<FileContent> {
    fn ext_to_mime(ext: &str) -> &str {
        match ext {
            "png" => "png", "jpg" | "jpeg" => "jpeg",
            "webp" => "webp", "gif" => "gif", "bmp" => "bmp",
            _ => "png",
        }
    }

    let p = std::path::Path::new(path);
    let ext = p.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let filename = p.file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("unknown")
        .to_string();

    match ext.as_str() {
        // 纯文本
        "txt" | "md" | "py" | "js" | "ts" | "jsx" | "tsx"
        | "json" | "html" | "css" | "rs" | "java" | "go"
        | "c" | "cpp" | "h" | "yaml" | "yml" | "toml"
        | "xml" | "sql" | "sh" | "bat" | "ps1" | "log"
        | "csv" | "vue" | "svelte" => {
            let content = std::fs::read_to_string(path)?;
            Ok(FileContent {
                file_type: "text".into(),
                content,
                mime: None,
                filename: filename.clone(),
            })
        }

        // PDF
        "pdf" => {
            let text = pdf_extract::extract_text(path)?;
            Ok(FileContent {
                file_type: "text".into(),
                content: text,
                mime: None,
                filename: filename.clone(),
            })
        }

        // Office (DOCX / PPTX)
        "docx" | "pptx" => {
            let text = extract_office_text(path, ext)?;
            Ok(FileContent {
                file_type: "text".into(),
                content: text,
                mime: None,
                filename: filename.clone(),
            })
        }

        // 图片
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => {
            let bytes = std::fs::read(path)?;
            let base64_str = BASE64_STANDARD.encode(&bytes);
            Ok(FileContent {
                file_type: "image".into(),
                content: base64_str,
                mime: Some(format!("image/{}", ext_to_mime(ext))),
                filename: filename.clone(),
            })
        }

        _ => Err("不支持的文件类型".into())
    }
}
```

### 3.2 事件（Rust emit → Vue listen）

#### `pet-state-changed`

```rust
app_handle.emit("pet-state-changed", StateChangedEvent {
    state: "sleeping".into(),
    previous: "idle".into(),
})?;
```

Vue 侧：
```typescript
import { listen } from '@tauri-apps/api/event'

listen<{ state: string; previous: string }>('pet-state-changed', (e) => {
  petStore.switchState(e.payload.state, e.payload.previous)
})
```

#### `pet-bubble`

```rust
app_handle.emit("pet-bubble", BubbleEvent {
    text: "Zzz...".into(),
    duration_ms: 3000,
})?;
```

Vue 侧创建气泡 DOM 元素，CSS transition，到时自动移除。

#### `pet-chat-chunk` / `pet-chat-done` / `pet-chat-error`

```rust
// 流式输出中
app_handle.emit("pet-chat-chunk", ChatChunkEvent {
    delta: "你好".into(),
})?;

// 输出完成
app_handle.emit("pet-chat-done", ())?;

// 出错
app_handle.emit("pet-chat-error", ChatErrorEvent {
    message: "API 连接超时".into(),
})?;
```

---

## 4. 动画引擎设计

### 4.1 文件：`src/composables/useAnimation.js`

```typescript
// ===== 逐帧动画引擎 API =====

interface AnimationInstance {
  // 启动播放
  play(config: AnimationConfig, onDone?: () => void): void
  // 停止当前动画
  stop(): void
  // 当前是否正在播放
  isPlaying(): boolean
}

function createAnimation(imgEl: HTMLImageElement): AnimationInstance
```

### 4.2 核心调度（递归 setTimeout，支持不等长帧）

```typescript
function createAnimation(imgEl: HTMLImageElement): AnimationInstance {
  let timer: ReturnType<typeof setTimeout> | null = null
  let frameIndex = 0
  let playing = false

  function scheduleFrame(config: AnimationConfig, onDone?: () => void) {
    if (!playing) return
    const frame = config.frames[frameIndex]
    if (!frame) return

    // 显示当前帧
    imgEl.src = frame.url

    // 调度下一帧
    timer = setTimeout(() => {
      frameIndex++
      if (frameIndex >= config.frames.length) {
        if (config.loop) {
          frameIndex = 0
          scheduleFrame(config, onDone)
        } else {
          // 非循环动画播完
          playing = false
          onDone?.()
        }
      } else {
        scheduleFrame(config, onDone)
      }
    }, frame.durationMs)
  }

  return {
    play(config: AnimationConfig, onDone?: () => void) {
      this.stop()
      frameIndex = 0
      playing = true
      scheduleFrame(config, onDone)
    },

    stop() {
      playing = false
      if (timer) {
        clearTimeout(timer)
        timer = null
      }
    },

    isPlaying() {
      return playing
    },
  }
}
```

### 4.3 Pet.vue 中的使用

```typescript
const petImg = ref<HTMLImageElement>()
let anim: AnimationInstance

onMounted(() => {
  anim = createAnimation(petImg.value!)
})

// 状态切换时由 petStore 调用
function onStateChanged(newState: string) {
  const config = animConfigs[newState]
  if (!config) return

  if (!config.loop) {
    // 瞬态动画：播完通知 Rust
    anim.play(config, () => {
      invoke('animation_finished', { stateName: newState })
    })
  } else {
    anim.play(config)
  }

  // IDLE 状态开启变体循环
  if (newState === 'idle') {
    startIdleVariantCycle()
  } else {
    stopIdleVariantCycle()
  }
}
```

### 4.3a Idle 子变体切换

```typescript
// Idle 状态下，每 20-60 秒随机间隔切换子变体
// 使用递归 setTimeout 实现真正的随机间隔
let idleVariantTimer: ReturnType<typeof setTimeout> | null = null

function startIdleVariantCycle() {
  const idleConfig = animConfigs['idle']
  if (!idleConfig?.variants?.length) return

  function scheduleNext() {
    const delay = 20_000 + Math.random() * 40_000
    idleVariantTimer = setTimeout(() => {
      if (petStore.currentState !== 'idle') {
        scheduleNext()
        return
      }

      const roll = Math.random()
      if (roll < 0.7) {
        anim.play(idleConfig)
      } else {
        const varFolder = idleConfig.variants![Math.floor(Math.random() * idleConfig.variants!.length)]
        const varConfig = animConfigs[varFolder]
        if (varConfig) anim.play({ ...varConfig, key: 'idle', loop: true })
      }

      scheduleNext()
    }, delay)
  }

  scheduleNext()
}

function stopIdleVariantCycle() {
  if (idleVariantTimer) {
    clearTimeout(idleVariantTimer)
    idleVariantTimer = null
  }
}
```

### 4.3b Raised 拖拽视觉反馈

```typescript
// 在 usePetDrag.js 中
function onDragStart() {
  const raisedConfig = animConfigs['raised']
  if (raisedConfig) {
    anim.play(raisedConfig)
  } else {
    // CSS 降级
    petImg.value!.style.transform = 'scale(0.95)'
    petImg.value!.style.filter = 'drop-shadow(2px 4px 6px rgba(0,0,0,0.3))'
    petImg.value!.style.transition = 'transform 0.15s, filter 0.15s'
  }
}

function onDragEnd() {
  if (!animConfigs['raised']) {
    petImg.value!.style.transform = ''
    petImg.value!.style.filter = ''
  }
  // 恢复当前 Rust 状态对应的动画
  onStateChanged(petStore.currentState)
}
```

### 4.4 预加载策略

```typescript
import { readDir, readFile } from '@tauri-apps/plugin-fs'

// 帧文件名解析：提取序号和持续毫秒
// 格式：<前缀>_<帧序号>_<持续毫秒>.png
function parseFrameFile(filename: string): { index: number; durationMs: number } | null {
  const match = filename.match(/_(\d{3})_(\d+)\.png$/i)
  if (!match) return null
  return { index: parseInt(match[1], 10), durationMs: parseInt(match[2], 10) || 125 }
}

// 预加载单个状态文件夹的所有帧
async function loadFrames(petFolder: string, folder: string): Promise<AnimationFrame[]> {
  const dirPath = `${petFolder}/${folder}`
  const entries = await readDir(dirPath)

  const frames: Array<{ index: number; durationMs: number; path: string }> = []
  for (const entry of entries) {
    if (!entry.name?.toLowerCase().endsWith('.png')) continue
    const parsed = parseFrameFile(entry.name)
    if (!parsed) continue
    frames.push({ ...parsed, path: `${dirPath}/${entry.name}` })
  }

  // 按序号排序
  frames.sort((a, b) => a.index - b.index)

  // 读取文件内容，创建 blob URL
  const result: AnimationFrame[] = []
  for (const frame of frames) {
    const bytes = await readFile(frame.path)
    const blob = new Blob([bytes], { type: 'image/png' })
    const url = URL.createObjectURL(blob)
    result.push({ url, durationMs: frame.durationMs })
  }

  return result
}

// 启动时预加载全部状态的全部帧
// 内存说明：blob URL 保持 PNG 压缩格式，不解码为 RGBA，
// 浏览器在渲染时按需解码。实测 15 状态 × 15 帧均值约 50-100MB，可接受。
async function preloadAllAnimations(
  petFolder: string,
  tomlConfigs: Record<string, { folder: string; loop: boolean; variants?: string[] }>
): Promise<Record<string, AnimationConfig>> {
  const configs: Record<string, AnimationConfig> = {}

  for (const [key, cfg] of Object.entries(tomlConfigs)) {
    // raised 文件夹可选：不存在时跳过，拖拽降级为 CSS
    let frames: AnimationFrame[] = []
    try {
      frames = await loadFrames(petFolder, cfg.folder)
    } catch (e) {
      if (key === 'raised') {
        console.warn(`[animation] raised 文件夹不存在，拖拽将使用 CSS 降级`)
        continue
      }
      throw e  // 其他状态文件夹缺失视为错误
    }

    configs[key] = {
      key,
      folder: cfg.folder,
      frames,
      loop: cfg.loop,
      variants: cfg.variants,
    }

    // 预加载变体
    if (cfg.variants) {
      for (const varFolder of cfg.variants) {
        const varFrames = await loadFrames(petFolder, varFolder)
        configs[varFolder] = {
          key: varFolder,
          folder: varFolder,
          frames: varFrames,
          loop: true,
        }
      }
    }
  }

  return configs
}
```

### 4.5 文件加载：解析 config.toml

```typescript
import { parse } from 'smol-toml'
import { readTextFile } from '@tauri-apps/plugin-fs'

interface TomlConfig {
  folder: string
  loop: boolean
  variants?: string[]
}

async function loadAnimationConfigs(petFolder: string): Promise<Record<string, AnimationConfig>> {
  const tomlText = await readTextFile(`${petFolder}/config.toml`)
  const raw = parse(tomlText) as Record<string, TomlConfig>
  return preloadAllAnimations(petFolder, raw)
}
```

> **注意**：此文件加载在 Vue 侧完成，使用 Tauri 的 `@tauri-apps/plugin-fs` 读取本地文件。每帧读取后转为 blob URL 缓存在内存，之后切帧只是换 `<img>.src` 指针，无 IO 开销。

---

## 5. LLM Adapter 设计

### 5.1 通用 Trait

```rust
// src-tauri/src/llm/mod.rs

use async_trait::async_trait;
use tauri::AppHandle;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 纯文本对话，返回时通过 stream consumer 回调
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        app_handle: AppHandle,
    ) -> Result<(), String>;

    /// 多模态对话（带图片）
    async fn chat_stream_with_images(
        &self,
        messages: Vec<ChatMessage>,
        images: Vec<ImageAttachment>,
        app_handle: AppHandle,
    ) -> Result<(), String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,    // "system" | "user" | "assistant"
    pub content: String,
}
```

### 5.2 Provider 注册

```rust
// src-tauri/src/llm/mod.rs

use std::collections::HashMap;

pub struct LlmRouter {
    providers: HashMap<String, Box<dyn LlmProvider>>,
    config: AppConfig,
    is_thinking: Arc<AtomicBool>,
    is_talking: Arc<AtomicBool>,
}

impl LlmRouter {
    pub fn new(
        config: AppConfig,
        is_thinking: Arc<AtomicBool>,
        is_talking: Arc<AtomicBool>,
    ) -> Self {
        let mut providers: HashMap<String, Box<dyn LlmProvider>> = HashMap::new();
        let key = |name: &str| config.api_keys.get(name).cloned().unwrap_or_default();
        providers.insert("deepseek".into(), Box::new(providers::DeepSeekProvider::new(key("deepseek"))));
        providers.insert("kimi".into(),     Box::new(providers::KimiProvider::new(key("kimi"))));
        providers.insert("qwen".into(),     Box::new(providers::QwenProvider::new(key("qwen"))));

        Self { providers, config, is_thinking, is_talking }
    }

    /// 用户修改 API Key / Provider / 模型 后，重建 Router
    pub fn update_config(&mut self, new_config: AppConfig) {
        let key = |name: &str| new_config.api_keys.get(name).cloned().unwrap_or_default();
        self.providers.insert("deepseek".into(), Box::new(providers::DeepSeekProvider::new(key("deepseek"))));
        self.providers.insert("kimi".into(),     Box::new(providers::KimiProvider::new(key("kimi"))));
        self.providers.insert("qwen".into(),     Box::new(providers::QwenProvider::new(key("qwen"))));
        self.config = new_config;
    }

    pub async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        images: Option<Vec<ImageAttachment>>,
        appended_text: Option<String>,
        app_handle: AppHandle,
    ) -> Result<(), String> {
        // 进入 THINK 状态
        self.is_thinking.store(true, Ordering::SeqCst);
        self.is_talking.store(false, Ordering::SeqCst);

        let provider = self.providers.get(&self.config.provider)
            .ok_or_else(|| format!("未知的 Provider: {}", self.config.provider))?;

        // 校验：当前 Provider 必须配置 API Key
        if self.config.current_api_key().is_empty() {
            // 出错时清除标记
            self.is_thinking.store(false, Ordering::SeqCst);
            return Err(format!("Provider {} 尚未配置 API Key", self.config.provider));
        }

        // 构造最终 messages
        let mut final_messages = messages;

        // 插入 system prompt（替换变量）
        let system_prompt = SYSTEM_PROMPT.replace("{pet_name}", &self.config.pet_name);
        final_messages.insert(0, ChatMessage {
            role: "system".into(),
            content: system_prompt,
        });

        // 如果有 appended_text（文件提取的文本），追加到 user message
        if let Some(text) = appended_text {
            if let Some(last) = final_messages.last_mut() {
                if last.role == "user" {
                    last.content = format!("{}\n\n以下是被处理文件的文本内容：\n---\n{}\n---", last.content, text);
                }
            }
        }

        if let Some(ref imgs) = images {
            if !imgs.is_empty() {
                provider.chat_stream_with_images(final_messages, imgs.clone(), app_handle).await
            } else {
                provider.chat_stream(final_messages, app_handle).await
            }
        } else {
            provider.chat_stream(final_messages, app_handle).await
        }
    }
}
```

### 5.3 System Prompt

```rust
const SYSTEM_PROMPT: &str = r#"你是一只桌面宠物，名字叫 {pet_name}。你的性格是活泼可爱、偶尔傲娇。
规则：
1. 回复要简短，一般不超过 100 字，像聊天而不是写文章
2. 语气要可爱、亲切，可以适当使用拟声词和颜文字（但不要过多）
3. 如果用户给你看图片或文件，认真阅读后再回应
4. 不要提自己是 AI，你就是一只真实的桌面宠物
5. 用户叫你做事时（如翻译、解释代码），可以用稍微正式一点的语气回答
6. 但如果用户只是闲聊，保持轻松可爱
"#;
```

### 5.4 DeepSeek Provider

```rust
// src-tauri/src/llm/providers/deepseek.rs

const ENDPOINT: &str = "https://api.deepseek.com/v1/chat/completions";

pub struct DeepSeekProvider {
    api_key: String,
    is_thinking: Arc<AtomicBool>,
    is_talking: Arc<AtomicBool>,
}

#[async_trait]
impl LlmProvider for DeepSeekProvider {
    async fn chat_stream(&self, messages: Vec<ChatMessage>, app_handle: AppHandle) -> Result<(), String> {
        let body = json!({
            "model": "deepseek-chat",
            "messages": messages.iter().map(|m| json!({
                "role": m.role,
                "content": m.content,
            })).collect::<Vec<_>>(),
            "stream": true,
        });

        stream_request(
            &self.api_key, ENDPOINT, body, app_handle,
            Arc::clone(&self.is_thinking), Arc::clone(&self.is_talking),
        ).await
    }

    async fn chat_stream_with_images(&self, messages: Vec<ChatMessage>, images: Vec<ImageAttachment>, app_handle: AppHandle) -> Result<(), String> {
        // DeepSeek 视觉：content 改为数组格式
        let mut api_messages = Vec::new();
        for m in &messages {
            api_messages.push(json!({
                "role": m.role,
                "content": m.content,
            }));
        }

        // 最后一个 user message 需要改为多模态格式
        let last_idx = api_messages.len() - 1;
        let user_content = &messages.last().unwrap().content;
        let mut content_parts = vec![
            json!({ "type": "text", "text": user_content }),
        ];
        for img in &images {
            content_parts.push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", img.mime, img.base64)
                }
            }));
        }
        api_messages[last_idx] = json!({
            "role": "user",
            "content": content_parts,
        });

        let body = json!({
            "model": "deepseek-chat",
            "messages": api_messages,
            "stream": true,
        });

        stream_request(
            &self.api_key, ENDPOINT, body, app_handle,
            Arc::clone(&self.is_thinking), Arc::clone(&self.is_talking),
        ).await
    }
}
```

### 5.5 Kimi Provider

```rust
// src-tauri/src/llm/providers/kimi.rs

const ENDPOINT: &str = "https://api.moonshot.cn/v1/chat/completions";
// 默认模型，用户可在设置中切换
const DEFAULT_MODEL: &str = "kimi-k2-6";

// 实现与 DeepSeek 基本一致，endpoint 和 model 不同
// Kimi 也兼容 OpenAI 格式，包括 Vision API 格式
```

Kimi K2.6 特点：
- 上下文 256K tokens
- 支持图片理解（`kimi-k2-6` 模型，OpenAI Vision API 兼容格式）
- 支持文件上传（通过 File API 上传后获得 file_id，但直接传文本进 prompt 更简单）

### 5.6 Qwen（通义千问）Provider

```rust
// src-tauri/src/llm/providers/qwen.rs

const ENDPOINT: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";
// DashScope 兼容 OpenAI 格式

// 推荐的视觉模型是 qwen-vl-plus 或 qwen-vl-max
// 纯文本可用 qwen-turbo / qwen-plus / qwen-max
```

### 5.7 SSE 流式解析

```rust
// 通用：三个 Provider 都用同一个解析逻辑（因为都是 OpenAI 兼容格式）

async fn stream_request(
    api_key: &str,
    endpoint: &str,
    body: serde_json::Value,
    app_handle: AppHandle,
    is_thinking: Arc<AtomicBool>,
    is_talking: Arc<AtomicBool>,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let mut response = client
        .post(endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        is_thinking.store(false, Ordering::SeqCst);  // 错误时清除
        return Err(format!("HTTP {}: {}", status, text));
    }

    // SSE 逐行读取
    use futures::StreamExt;
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut first_chunk = true;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            is_thinking.store(false, Ordering::SeqCst);
            is_talking.store(false, Ordering::SeqCst);
            format!("流读取错误: {}", e)
        })?;
        let chunk_str = String::from_utf8_lossy(&chunk);
        buffer.push_str(&chunk_str);

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if line == "data: [DONE]" {
                is_thinking.store(false, Ordering::SeqCst);
                is_talking.store(false, Ordering::SeqCst);
                app_handle.emit("pet-chat-done", ()).ok();
                return Ok(());
            }

            if let Some(data) = line.strip_prefix("data: ") {
                match serde_json::from_str::<serde_json::Value>(data) {
                    Ok(parsed) => {
                        if let Some(content) = parsed["choices"][0]["delta"]["content"].as_str() {
                            if !content.is_empty() {
                                // 首个有效 content → THINK → TALK
                                if first_chunk {
                                    first_chunk = false;
                                    is_thinking.store(false, Ordering::SeqCst);
                                    is_talking.store(true, Ordering::SeqCst);
                                }
                                app_handle.emit("pet-chat-chunk", ChatChunkEvent {
                                    delta: content.to_string(),
                                }).ok();
                            }
                        }
                        if let Some(reason) = parsed["choices"][0]["finish_reason"].as_str() {
                            if reason == "stop" || reason == "length" {
                                is_thinking.store(false, Ordering::SeqCst);
                                is_talking.store(false, Ordering::SeqCst);
                                app_handle.emit("pet-chat-done", ()).ok();
                                return Ok(());
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
        }
    }

    is_thinking.store(false, Ordering::SeqCst);
    is_talking.store(false, Ordering::SeqCst);
    app_handle.emit("pet-chat-done", ()).ok();
    Ok(())
}
```

### 5.8 多轮对话上下文管理

```rust
// src-tauri/src/llm/context.rs

pub struct ConversationContext {
    messages: Vec<ChatMessage>,
    max_history: usize,  // 最大保留轮数
}

impl ConversationContext {
    pub fn new(max_history: usize) -> Self {
        Self { messages: Vec::new(), max_history }
    }

    pub fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push(ChatMessage {
            role: role.into(),
            content: content.into(),
        });
        self.trim();
    }

    pub fn get_messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    fn trim(&mut self) {
        // 保留最近 max_history 轮（一轮 = user + assistant）
        let max_messages = self.max_history * 2;
        if self.messages.len() > max_messages {
            let drain = self.messages.len() - max_messages;
            // 保留 system message（如果存在），删除最早的非系统消息
            let system_count = self.messages.iter().take_while(|m| m.role == "system").count();
            self.messages.drain(system_count..system_count + drain);
        }
    }
}
```

---

### 5.4 通义千问 Provider

```rust
// src-tauri/src/llm/providers/qwen.rs

const ENDPOINT: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";

pub struct QwenProvider {
    api_key: String,
}

impl QwenProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[async_trait]
impl LlmProvider for QwenProvider {
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
        app_handle: AppHandle,
        is_thinking: Arc<AtomicBool>,
        is_talking: Arc<AtomicBool>,
    ) -> Result<String, String> {
        stream_request(
            ENDPOINT,
            &self.api_key,
            messages,
            model,
            app_handle,
            is_thinking,
            is_talking,
        ).await
    }

    async fn chat_stream_with_images(
        &self,
        messages: Vec<ChatMessage>,
        images: Vec<ImageAttachment>,
        model: &str,
        app_handle: AppHandle,
        is_thinking: Arc<AtomicBool>,
        is_talking: Arc<AtomicBool>,
    ) -> Result<String, String> {
        // 通义千问多模态：把最后一条 user message 的 content 改为数组
        let mut final_messages = messages;
        if let Some(last) = final_messages.last_mut() {
            if last.role == "user" {
                let text_part = serde_json::json!({
                    "type": "text",
                    "text": last.content.clone()
                });
                let mut content_array = vec![text_part];
                for img in images {
                    content_array.push(serde_json::json!({
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:{};base64,{}", img.mime, img.base64)
                        }
                    }));
                }
                // 替换 content 为多模态数组（需要修改 ChatMessage 结构支持 serde_json::Value）
            }
        }

        stream_request(
            ENDPOINT,
            &self.api_key,
            final_messages,
            model,
            app_handle,
            is_thinking,
            is_talking,
        ).await
    }
}
```

### 5.5 流式请求通用函数

```rust
// src-tauri/src/llm/mod.rs

use futures::StreamExt;

/// ThinkingGuard: 确保无论正常退出还是 panic，都清理 is_thinking/is_talking
struct ThinkingGuard {
    is_thinking: Arc<AtomicBool>,
    is_talking: Arc<AtomicBool>,
}

impl Drop for ThinkingGuard {
    fn drop(&mut self) {
        self.is_thinking.store(false, Ordering::SeqCst);
        self.is_talking.store(false, Ordering::SeqCst);
    }
}

async fn stream_request(
    endpoint: &str,
    api_key: &str,
    messages: Vec<ChatMessage>,
    model: &str,
    app_handle: AppHandle,
    is_thinking: Arc<AtomicBool>,
    is_talking: Arc<AtomicBool>,
) -> Result<String, String> {
    let _guard = ThinkingGuard {
        is_thinking: Arc::clone(&is_thinking),
        is_talking: Arc::clone(&is_talking),
    };

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true
    });

    let resp = client
        .post(endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP 请求失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, text));
    }

    let mut stream = resp.bytes_stream();
    let mut full_text = String::new();
    let mut first_chunk = true;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("流读取失败: {}", e))?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if !line.starts_with("data: ") {
                continue;
            }
            let json_str = &line[6..];
            if json_str == "[DONE]" {
                break;
            }

            let parsed: serde_json::Value = serde_json::from_str(json_str)
                .map_err(|e| format!("JSON 解析失败: {}", e))?;

            if let Some(delta) = parsed["choices"][0]["delta"]["content"].as_str() {
                if first_chunk {
                    // 收到第一个 chunk → THINK → TALK
                    is_thinking.store(false, Ordering::SeqCst);
                    is_talking.store(true, Ordering::SeqCst);
                    first_chunk = false;
                }

                full_text.push_str(delta);
                app_handle.emit("pet-chat-chunk", ChatChunkEvent {
                    delta: delta.to_string(),
                }).ok();
            }
        }
    }

    // 流结束 → TALK → IDLE
    is_talking.store(false, Ordering::SeqCst);
    app_handle.emit("pet-chat-done", ()).ok();

    Ok(full_text)
}
```

---

## 6. 文件处理管线

### 6.1 分发逻辑

```
ext = 取文件扩展名，转小写

if ext in TEXT_EXTENSIONS:
    → text.rs     (std::fs::read_to_string)
if ext == "pdf":
    → pdf.rs      (pdf-extract crate)
if ext in ["docx", "pptx"]:
    → office.rs   (zip + quick-xml crates)
if ext in IMAGE_EXTENSIONS:
    → image.rs    (std::fs::read + base64 crate)
else:
    → 返回 Err("不支持的文件类型: .{ext}")
```

### 6.2 PDF 提取实现

```rust
// src-tauri/src/file_handler/pdf.rs
use pdf_extract;

pub fn extract_pdf_text(path: &str) -> Result<String, String> {
    pdf_extract::extract_text(path)
        .map_err(|e| format!("PDF 解析失败: {}", e))
}
```

依赖 `Cargo.toml`：`pdf-extract = "0.7"`

### 6.3 DOCX/PPTX 提取实现

```rust
// src-tauri/src/file_handler/office.rs
use std::io::Read;
use zip::ZipArchive;
use quick_xml::Reader;

pub fn extract_office_text(path: &str, ext: &str) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("打开文件失败: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("解压失败: {}", e))?;

    // DOCX 的文本在 word/document.xml
    // PPTX 的文本分散在 ppt/slides/slide*.xml
    let xml_path = match ext {
        "docx" => "word/document.xml",
        "pptx" => "ppt/slides/",
        _ => return Err("不支持的格式".into()),
    };

    if ext == "docx" {
        let mut doc = archive.by_name(xml_path).map_err(|e| format!("找不到文档内容: {}", e))?;
        let mut xml = String::new();
        doc.read_to_string(&mut xml).map_err(|e| format!("读取失败: {}", e))?;
        extract_text_from_docx_xml(&xml)
    } else {
        extract_text_from_pptx(&mut archive)
    }
}

fn extract_text_from_docx_xml(xml: &str) -> Result<String, String> {
    // DOCX 的正文文本嵌在 <w:t> 标签内，其他位置的 Text 事件
    // （如样式名、属性值等）不应混入。用一个 in_w_t 标志位精确收集。
    use quick_xml::events::Event;
    let mut reader = Reader::from_str(xml);
    let mut text = String::new();
    let mut buf = Vec::new();
    let mut in_w_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"w:t" {
                    in_w_t = true;
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"w:t" {
                    in_w_t = false;
                }
                // <w:p> 段落结束时插入换行，方便阅读
                if e.name().as_ref() == b"w:p" {
                    text.push('\n');
                }
            }
            Ok(Event::Text(ref e)) if in_w_t => {
                text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(format!("XML 解析错误: {}", err)),
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
}

fn extract_text_from_pptx(archive: &mut ZipArchive<File>) -> Result<String, String> {
    let mut all_text = String::new();
    for i in 1..=100 {  // 最多 100 张幻灯片
        let path = format!("ppt/slides/slide{}.xml", i);
        if let Ok(mut file) = archive.by_name(&path) {
            let mut xml = String::new();
            file.read_to_string(&mut xml).ok();
            let slide_text = extract_text_from_pptx_xml(&xml)?;
            if !slide_text.is_empty() {
                all_text.push_str(&format!("[幻灯片 {}]\n{}\n\n", i, slide_text));
            }
        } else {
            break;
        }
    }
    Ok(all_text)
}

fn extract_text_from_pptx_xml(xml: &str) -> Result<String, String> {
    // PPTX 的正文文本在 <a:t> 标签内（与 DOCX 的 <w:t> 不同）
    use quick_xml::events::Event;
    let mut reader = Reader::from_str(xml);
    let mut text = String::new();
    let mut buf = Vec::new();
    let mut in_a_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"a:t" {
                    in_a_t = true;
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"a:t" {
                    in_a_t = false;
                }
                // <a:p> 段落结束时插入换行
                if e.name().as_ref() == b"a:p" {
                    text.push('\n');
                }
            }
            Ok(Event::Text(ref e)) if in_a_t => {
                text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(format!("XML 解析错误: {}", err)),
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
}
```

依赖 `Cargo.toml`：`zip = "2"`, `quick-xml = "0.37"`

### 6.4 图片 Base64 编码

```rust
// src-tauri/src/file_handler/image.rs
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

pub fn encode_image(path: &str) -> Result<(String, String), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取图片失败: {}", e))?;

    // 推断 MIME 类型
    let mime = match std::path::Path::new(path).extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase().as_str()
    {
        "png"  => "image/png",
        "jpg"  => "image/jpeg",
        "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif"  => "image/gif",
        "bmp"  => "image/bmp",
        _      => "image/png", // 降级
    };

    let b64 = BASE64.encode(&bytes);
    Ok((b64, mime.to_string()))
}
```

---

## 7. 配置格式 Schema

### 7.1 用户配置 `config.json`（`%APPDATA%/desktoppet/config.json`）

```json
{
  "provider": "deepseek",
  "api_keys": {
    "deepseek": "<加密存储>",
    "kimi": "<加密存储>",
    "qwen": "<加密存储>"
  },
  "model": "deepseek-chat",
  "pet_folder": "C:/Users/xxx/Documents/my-pet",
  "pet_size": 128,
  "pet_position": { "x": 100, "y": 200 },
  "auto_start": false,
  "pet_name": "小咪"
}
```

> 各 Provider 的 Key 独立保存，切换 Provider 时不会丢失之前已填的 Key。未填写的 Provider 在 `api_keys` 里可缺省（取值时按空字符串处理，发送前会校验）。

### 7.2 角色配置 `config.toml`（角色文件夹内）

```toml
[appear]
folder = "appear"
loop = false

[idle]
folder = "idle"
loop = true
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

### 7.3 气泡配置 `bubbles.json`（角色文件夹内）

```json
[
  {
    "state": "appear",
    "trigger": "on_enter",
    "text": [
      "嘿！我来了~",
      "早上好呀！",
      "又见面了！",
      "{pet_name} 回来啦！"
    ],
    "cooldown_seconds": 0,
    "duration_ms": 3000
  },
  {
    "state": "sleeping",
    "trigger": "on_enter",
    "text": [
      "Zzz...",
      "好困，先睡一会儿...",
      "呼——"
    ],
    "cooldown_seconds": 300,
    "duration_ms": 3000
  },
  {
    "state": "sleeping",
    "trigger": "on_return_from",
    "text": [
      "你回来了！",
      "睡醒了吗？",
      "嗯？怎么了？"
    ],
    "cooldown_seconds": 180,
    "duration_ms": 3000
  },
  {
    "state": "typing",
    "trigger": "on_enter",
    "text": [
      "你在忙什么？",
      "键盘敲得好快...",
      "在写什么好东西？"
    ],
    "cooldown_seconds": 600,
    "duration_ms": 3000
  },
  {
    "state": "worried",
    "trigger": "on_enter",
    "text": [
      "已经{time}了，还不睡吗？",
      "熬夜会长黑眼圈...",
      "该休息啦！"
    ],
    "cooldown_seconds": 600,
    "duration_ms": 4000
  },
  {
    "state": "sweating",
    "trigger": "on_enter",
    "text": [
      "好热...你的电脑在冒烟！",
      "CPU 要炸了！",
      "这是在编译什么怪物..."
    ],
    "cooldown_seconds": 300,
    "duration_ms": 3000
  },
  {
    "state": "think",
    "trigger": "on_enter",
    "text": [
      "嗯...",
      "让我想想...",
      "等一下哦~"
    ],
    "cooldown_seconds": 0,
    "duration_ms": 2000
  },
  {
    "state": "clicked",
    "trigger": "on_return_from",
    "text": [
      "嘻嘻~",
      "痒！",
      "别闹~"
    ],
    "cooldown_seconds": 0,
    "duration_ms": 2000
  },
  {
    "state": "shutdown",
    "trigger": "on_enter",
    "text": [
      "拜拜~",
      "下次见！",
      "不要忘了我哦..."
    ],
    "cooldown_seconds": 0,
    "duration_ms": 2000
  }
]
```

**变量替换**：
- `{pet_name}` → 替换为 config.json 中的 `pet_name` 值
- `{time}` → 替换为当前时间，格式如 "凌晨2点"（Rust 侧 `chrono` 格式化）

**触发规则详解**：
- `trigger: "on_enter"` → 进入该状态时弹出，受冷却限制
- `trigger: "on_return_from"` → 从该状态离开（回落到之前状态）时弹出
- `cooldown_seconds: 0` → 无冷却，每次触发都弹

---

## 8. 关键决策记录

| 决策 | 理由 | 替代方案（已否决） |
|------|------|-------------------|
| 动画用逐帧 PNG 而非 Sprite Sheet | VPet 素材直接兼容、支持不等长帧时长、用户换角色无需拼图工具；1000×1000 帧预加载为 blob URL 后切帧是纯内存操作 | Sprite Sheet（需拼图、丢失不等长帧信息、用户替换门槛高） |
| 逐帧预加载保持 PNG 压缩格式（blob URL）而非解码为 RGBA | 1000×1000 RGBA = 4MB/帧，15 状态 × 15 帧均值 ≈ 900MB，远超可接受范围；blob URL 保持压缩，浏览器按需解码，实测内存占用降至 50-100MB 量级 | 预解码为 ImageBitmap（内存爆炸）、懒加载（切换时卡顿） |
| 状态机在 Rust 侧 | 状态变迁依赖系统监测数据（钩子只在 Rust 侧有），避免跨进程同步数据的一致性问题 | Vue 侧做状态机（需要把监测数据频繁传给前端，增加 IPC 开销） |
| 动画预加载在 Vue 侧 | 图片最终要渲染到浏览器，前端直接读本地文件最高效 | Rust 侧读文件再传给前端（多余 IPC） |
| LLM 请求在 Rust 侧 | API Key 存本地，前端不应直接持有 Key（XSS 风险）；Rust 侧做 HTTP 不会被 CORS 限制 | 前端直接调 API（CORS 问题 + Key 暴露） |
| 对话上下文在 Rust 侧维护 | 与 LLM Adapter 同侧，上下文管理不经过 IPC | Vue 侧维护（需要每次把完整 messages 传给后端） |
| 气泡冷却按同状态同类触发计算 | 避免频繁骚扰用户，TYPING 设 10min 冷却 | 无冷却（会疯狂弹） |
| config.json 存 `%APPDATA%` | Windows 标准做法，不会污染安装目录 | 存安装目录旁（`tauri.conf.json` 旁边，但有权限问题） |
| API Key XOR 混淆 + DPAPI | 不是专业加密但防止明文泄露，DPAPI 绑定当前 Windows 用户 | 明文存储（不安全）、AES 加密（Key 也需要存，形成套娃） |
| 三个 Provider 全用 OpenAI 兼容格式 | Kimi、DeepSeek、通义千问 DashScope 全部兼容 OpenAI Chat Completions API 格式 | 每个 Provider 独立格式解析（现在不需要） |
| THINK/TALK 用 Arc\<AtomicBool\> 而非状态机内状态 | 跨线程（llm 线程 ↔ 状态机线程）无需锁，比 channel 更轻量；两个标记独立，不存在竞态 | mpsc::channel（额外线程通信开销）、Mutex（过重） |
| RAISED 不做为状态机状态 | 拖拽是前端 UI 事件，不涉及系统检测或业务逻辑，纯 CSS/vue 处理最简洁 | 进状态机（增加不必要的 IPC 往返和状态切换延迟） |
| Idle 变体在前端随机切换 | 不变更 Rust 状态，避免不必要的状态切换事件；变体只是同一状态下的帧文件夹替换 | 每个变体做一个状态（状态膨胀，维护成本高） |

---

## 9. Rust 依赖清单 (Cargo.toml)

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
async-trait = "0.1"
reqwest = { version = "0.12", features = ["json", "stream"] }
tokio = { version = "1", features = ["full"] }
futures = "0.3"
base64 = "0.22"
chrono = "0.4"
pdf-extract = "0.7"
zip = "2"
quick-xml = "0.37"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_System_Performance",
    "Win32_System_Threading",
    "Win32_Foundation",
] }
```

---

## 10. 前端依赖清单 (package.json additions)

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-fs": "^2",
    "pinia": "^2",
    "smol-toml": "^1"
  }
}
```

---

> 此文档面向实现者。需配合 PRD.md 阅读——PRD 回答"要什么"，本文档回答"怎么做"。


---

## 附录：实际实现技术细节

本节记录实际开发过程中的关键技术实现细节，供后续维护和优化参考：

### A.1 窗口管理实现

**实际窗口配置**（`tauri.conf.json`）：
```json
{
  "windows": [{
    "label": "main",
    "width": 600,
    "height": 600,
    "decorations": false,
    "transparent": true,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "shadow": false,
    "dragDropEnabled": true
  }]
}
```

**布局实现**：
- 宠物本体：`position: absolute; left: 110px; top: 50%; transform: translate(-50%, -50%)`
- 聊天面板：`position: fixed; top: 8px; right: 8px; width: 360px`
- 设置/演示面板：居中悬浮，透明背景无遮罩

### A.2 手动拖拽实现

**前端逻辑**（`usePetDrag.js`）：
```javascript
// mousedown → 启动180ms长按定时器 + 监听mousemove
// 移动超过4px阈值 OR 长按到时 → 进入拖拽
// 拖拽期间调用 invoke('move_window_to', {x, y})
// mouseup → 保存位置 invoke('save_pet_position', {x, y})
```

**Rust命令**：
```rust
#[tauri::command]
async fn move_window_to(app: AppHandle, x: i32, y: i32) -> Result<(), String> {
    window.set_position(PhysicalPosition { x, y })
}

#[tauri::command]
async fn get_window_position(app: AppHandle) -> Result<Position, String> {
    window.outer_position()
}
```

### A.3 鼠标穿透实现

**Rust光标轮询线程**（50ms间隔）：
```rust
static FORCE_BLOCK_PASSTHROUGH: AtomicBool = AtomicBool::new(false);
static PET_CENTER_X/Y/RADIUS: AtomicI32;

fn start_cursor_poll(app_handle: AppHandle) {
    loop {
        sleep(50ms);
        let cursor = GetCursorPos(); // Win32 API
        let force_block = FORCE_BLOCK_PASSTHROUGH.load();
        
        if force_block {
            // 面板打开时强制不穿透
            set_ignore_cursor_events(false);
        } else {
            // 判断光标是否在宠物外接矩形内
            let in_pet = abs(cursor.x - PET_CENTER_X) <= RADIUS
                      && abs(cursor.y - PET_CENTER_Y) <= RADIUS;
            set_ignore_cursor_events(!in_pet);
        }
    }
}
```

**前端面板锁定**（`Pet.vue`）：
```javascript
watch([menuVisible, settingsVisible, debugVisible, chatVisible], () => {
    const anyOpen = menuVisible || settingsVisible || debugVisible || chatVisible;
    invoke('set_force_block_passthrough', { block: anyOpen });
});
```

**宠物边界上报**：
```javascript
// 每2秒 + 拖拽结束后上报宠物屏幕坐标和半径
const rect = petImg.getBoundingClientRect();
const pos = await getCurrentWindow().outerPosition();
const centerX = pos.x + (rect.left + rect.width/2) * dpr;
const centerY = pos.y + (rect.top + rect.height/2) * dpr;
const radius = Math.max(rect.width, rect.height)/2 * dpr + 4;
invoke('update_pet_bounds', { centerX, centerY, radius });
```

### A.4 LLM多模态实现

**图片格式**（OpenAI兼容）：
```json
{
  "role": "user",
  "content": [
    {"type": "text", "text": "用户文本"},
    {
      "type": "image_url",
      "image_url": {
        "url": "data:image/jpeg;base64,/9j/4AAQ..."
      }
    }
  ]
}
```

**模型支持情况**：
- DeepSeek: deepseek-chat/reasoner **不支持**图片（会报400错误）
- Kimi: kimi-k2-6 **支持**图片
- 通义千问: qwen3.6-flash/plus, qwen3-vl-plus/flash **支持**图片

**ThinkingGuard防卡死机制**：
```rust
struct ThinkingGuard {
    is_thinking: Arc<AtomicBool>,
    is_talking: Arc<AtomicBool>,
}
impl Drop for ThinkingGuard {
    fn drop(&mut self) {
        // 无论正常退出还是panic，都清理状态
        self.is_thinking.store(false, Ordering::SeqCst);
        self.is_talking.store(false, Ordering::SeqCst);
    }
}
```

### A.5 文件处理实现

**支持格式**：
- **文本**：txt, md, json, py, js, ts, rs, html, css, yaml, toml, xml, sql, sh, log, csv, vue 等
- **PDF**：使用 `pdf-extract` crate 提取文本
- **Office**：
  - DOCX: 解ZIP → 读 `word/document.xml` → 收集 `<w:t>` 文本节点
  - PPTX: 解ZIP → 遍历 `ppt/slides/slide*.xml` → 收集 `<a:t>` 文本节点
- **图片**：png, jpg, jpeg, webp, gif, bmp → base64编码（最大10MB）

**Rust命令**：
```rust
#[tauri::command]
async fn process_file(path: String) -> Result<FileContent, String> {
    // 根据扩展名分发到不同处理器
    // 返回 {file_type: "text"|"image", content, mime, filename}
}
```

### A.6 配置管理实现

**配置结构**：
```rust
struct AppConfig {
    provider: String,              // "deepseek" | "kimi" | "qwen"
    api_keys: HashMap<String, String>, // per-provider独立存储
    model: String,                 // 当前选中的模型
    pet_folder: String,            // 角色文件夹路径（空则用默认）
    pet_size: u32,                 // 96 | 128 | 192
    pet_position: Option<Position>, // 窗口位置
    auto_start: bool,              // 开机自启（配置项存在但未实现注册）
    pet_name: String,              // 宠物名字（用于气泡变量替换）
}
```

**存储位置**：
```rust
fn config_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or(".");
    PathBuf::from(appdata).join("desktoppet").join("config.json")
}
// Windows: C:\Users\<用户名>\AppData\Roaming\desktoppet\config.json
```

**实时同步**：
```rust
#[tauri::command]
async fn save_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    config.save()?;
    app.emit("pet-config-changed", config)?; // 通知前端
    Ok(())
}
```

前端监听：
```javascript
listen('pet-config-changed', (event) => {
    if (event.payload.pet_size) {
        petStore.petSize = event.payload.pet_size;
        nextTick(() => reportPetBounds()); // 重新上报边界
    }
});
```

### A.7 气泡系统实现

**触发逻辑**（`bubble/mod.rs`）：
```rust
impl BubbleManager {
    pub fn on_state_change(
        &self,
        new_state: &str,
        prev_state: &str,
        pet_name: &str,
    ) -> Option<BubblePayload> {
        // 优先 on_return_from（离开旧状态）
        if let Some(payload) = self.try_trigger(prev_state, "on_return_from", pet_name) {
            return Some(payload);
        }
        // 其次 on_enter（进入新状态）
        self.try_trigger(new_state, "on_enter", pet_name)
    }
}
```

**变量替换**：
```rust
fn replace_variables(text: &str, pet_name: &str) -> String {
    let now = Local::now();
    let hour = now.hour();
    let time_str = match hour {
        0..=5 => "凌晨",
        6..=11 => "上午",
        12..=17 => "下午",
        18..=23 => "晚上",
        _ => "",
    };
    text.replace("{pet_name}", pet_name)
        .replace("{time}", time_str)
}
```

**前端定位**（`PetBubble.vue`）：
```javascript
// 通过 getBoundingClientRect() 实时跟随宠物DOM位置
const rect = petImg.getBoundingClientRect();
bubbleStyle.value = {
    position: 'fixed',
    left: `${rect.left + rect.width/2}px`,
    top: `${rect.top - 8}px`,
    transform: 'translate(-50%, -100%)'
};
```

### A.8 已知问题和限制

1. **图片理解bug**：发送图片后偶尔卡住，需要重启应用（可能与流式响应解析有关）
2. **面板交互bug**：聊天窗口打开时切换到TYPING状态会导致所有面板无法交互
3. **拖拽体验**：长按180ms触发机制有时不够灵敏
4. **API Key安全**：当前明文存储，未实现加密
5. **开机自启**：配置项存在但未实现Windows注册表注册
6. **退出错误**：关闭时命令行显示 `Failed to unregister class Chrome_WidgetWin_0. Error = 1412`（非致命错误）

### A.9 性能优化

1. **动画预加载**：启动时一次性加载所有帧为blob URL，避免运行时IO
2. **光标轮询**：50ms间隔，平衡响应速度和CPU占用
3. **状态机tick**：1秒间隔，避免过度检测
4. **气泡冷却**：防止同一气泡频繁弹出
5. **上下文管理**：对话历史保留最近10轮，避免token超限

---

> 本文档记录实际实现细节，与PRD配套使用。如有差异以实际代码为准。
