# RAG 本地知识库问答 — 技术设计文档

> 版本: 1.1.0 | 日期: 2026-05-29 | 配套 RAG-PRD v1.1.0

> v1.1.0 变更：根据实际开发结果更新数据结构、API、线程模型、构建流程。

---

## 1. 数据结构定义

### 1.1 Rust — 知识库存储类型

```rust
/// 单个索引块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbChunk {
    pub id: String,              // UUID v4 唯一标识
    pub text: String,            // 原始文本内容
    pub source_file: String,     // 来源文件名（相对知识库根目录的路径）
    pub embedding: Vec<f32>,     // 向量（维度取决于模型，当前 ~768 维）
}

/// 知识库元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbMetadata {
    pub file_count: usize,       // 实际产生文本块的文件数
    pub chunk_count: usize,      // 文本块总数
    pub last_indexed: String,    // ISO 8601 时间
    pub kb_folder: String,       // 知识库根目录路径
}

/// 完整的知识库存储文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbStore {
    pub metadata: KbMetadata,
    pub chunks: Vec<KbChunk>,
}
```

### 1.2 Rust — Event Payload 类型

```rust
#[derive(Debug, Clone, Serialize)]
pub struct KbIndexProgressEvent {
    pub current: usize,
    pub total: usize,
    pub current_file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct KbIndexDoneEvent {
    pub file_count: usize,
    pub chunk_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct KbStatus {
    pub indexed: bool,
    pub file_count: usize,
    pub chunk_count: usize,
    pub last_indexed: String,
    pub kb_folder: String,
    pub model_loaded: bool,        // v1.1 新增
}

#[derive(Clone, Serialize)]
pub struct ChatDoneEvent {
    pub sources: Vec<String>,      // v1.1 修改，原为 ()
}
```

### 1.3 Rust — AppState 变更

```rust
pub struct AppState {
    // === 现有字段，不变 ===
    pub config: Mutex<AppConfig>,
    pub current_state: Mutex<PetState>,
    pub prev_state: Mutex<PetState>,
    pub is_thinking: Arc<AtomicBool>,
    pub is_talking: Arc<AtomicBool>,
    pub force_state: Mutex<Option<PetState>>,
    pub input_monitor: monitor::input::InputMonitor,
    pub bubble_manager: Mutex<BubbleManager>,
    pub conversation: Mutex<ConversationContext>,

    // === 新增字段 ===
    pub rag_conversation: Mutex<ConversationContext>,
    pub kb_store: Arc<Mutex<Option<KbStore>>>,              // 外层 Arc 支持多引用共享
    pub embedding_model: Arc<Mutex<Option<EmbeddingModel>>>, // 同上
    pub is_indexing: Arc<AtomicBool>,
    pub shutdown_flag: Arc<AtomicBool>,                     // v1.1 新增，用于干净退出
}
```

> **为什么 `kb_store` 和 `embedding_model` 用 `Arc<Mutex<Option<T>>>`**：需要在 setup 闭包和命令处理函数中同时持有引用。`State<'_, Arc<AppState>>` 可 clone 出 `Arc`，内层 `Mutex` 提供内部可变性。

### 1.4 Rust — AppConfig 变更

```rust
pub struct AppConfig {
    pub provider: String,
    pub api_keys: HashMap<String, String>,
    pub model: String,
    pub pet_folder: String,
    pub pet_size: u32,
    pub pet_position: Option<Position>,
    pub auto_start: bool,
    pub pet_name: String,
    pub kb_folder: Option<String>,
}
```

### 1.5 Rust — send_chat 命令签名变更

```rust
#[tauri::command]
async fn send_chat(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    message: String,
    images: Option<Vec<ImageAttachment>>,
    appended_text: Option<String>,
    rag_mode: Option<bool>,     // 新增
) -> Result<(), String>;
```

---

## 2. 模块架构

### 2.1 新增 Rust 模块：`rag/`

```
src-tauri/src/rag/
├── mod.rs          # 模块入口 + build_index 管线 + 3 个 Tauri 命令
├── chunking.rs     # 文档分块（段落分割 + 滑动窗口）
├── embedding.rs    # ONNX 模型加载 + 文本向量化（mean pooling + L2 归一化）
└── store.rs        # kb_store.json 读写 + 余弦相似度检索
```

**依赖关系：**

```
rag::mod (Tauri commands + build_index)
  ├─ rag::store        →  kb_store.json 读写 / 检索
  ├─ rag::embedding    →  ONNX 模型加载 + 文本向量化
  ├─ rag::chunking     →  文档分块
  └─ file_handler      →  process_file()（复用现有）

内部依赖链：
  build_index → embedding（批量向量化文档）
  build_index → chunking（文档分块）
  build_index → file_handler（提取文本）
  send_chat   → embedding（检索时向量化查询）
  send_chat   → store::search（余弦检索）
  embedding  无内部依赖（仅依赖 ort + tokenizers crate）
  chunking   无内部依赖（纯文本处理）
```

### 2.2 与现有代码的集成点

| 现有文件 | 变更方式 |
|----------|----------|
| `src-tauri/src/lib.rs` | `mod rag;` + AppState 新增 5 个字段 + setup() 中后台加载模型和 kb_store + send_chat 增加检索分支 + generate_handler 注册 3 个新命令 + shutdown_flag 退出机制 |
| `src-tauri/src/store/config.rs` | AppConfig 加 `kb_folder: Option<String>` 字段 |
| `src-tauri/src/llm/context.rs` | 不变。`ConversationContext` 被两个上下文实例复用 |
| `src-tauri/src/llm/mod.rs` | 不变。SYSTEM_PROMPT 和 stream_request 被注入 KB 上下文后的消息调用 |
| `src-tauri/src/file_handler/mod.rs` | 不变。索引时直接复用 `process_file()` |
| `src-tauri/build.rs` | **新增**：自动下载 ONNX Runtime 1.24.2 DLL |
| `src/components/PetChat.vue` | 新增模式切换滑块 + 参考来源展示 |
| `src/components/PetSettings.vue` | 新增 KB 配置区域 + 错误显示 + 状态更新 |
| `src-tauri/Cargo.toml` | 新增 ort, tokenizers, ndarray, uuid |

---

## 3. 分块引擎 (`rag/chunking.rs`)

### 3.1 算法

```
输入：原始文本 + 来源文件路径（相对 KB 根目录）
输出：Vec<KbChunk>（embedding 字段为空 Vec，后续由 embedding 模块填充）

步骤：
1. 统一换行符（\r\n → \n）
2. 按连续两个及以上换行符（\n\n）分割为段落
3. 对每个段落：
   a. 段落长度 ≤ 512 字符 → 直接作为一个 chunk
   b. 段落长度 > 512 字符 → 以 512 字符为窗口、步长 462（=512-50）滑动切割
4. 每个 chunk 分配 UUID v4 作为 id，记录来源文件名
```

### 3.2 签名

```rust
pub fn chunk_text(text: &str, source_file: &str, max_chars: usize, overlap: usize) -> Vec<KbChunk>;
```

调用示例：`chunk_text(&fc.content, &source, 512, 50)`

---

## 4. Embedding 管线 (`rag/embedding.rs`)

### 4.1 模型加载

```rust
#[derive(Clone)]
pub struct EmbeddingModel {
    session: Arc<Mutex<Session>>,   // Mutex 必需：ort v2 Session 不是 Sync
    tokenizer: Arc<Tokenizer>,
}

impl EmbeddingModel {
    pub fn load(model_path: &str, tokenizer_path: &str) -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|e| format!("SessionBuilder 失败: {}", e))?
            .commit_from_file(model_path)
            .map_err(|e| format!("加载 ONNX 模型失败: {}", e))?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| format!("加载 tokenizer 失败: {}", e))?;

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
            tokenizer: Arc::new(tokenizer),
        })
    }
}
```

> **注意**：`ort` v2 的 `Session::builder()` API 不同于 v1 的 `SessionBuilder::new()`。v2 使用 builder 模式，链式调用。

### 4.2 向量化流程

```rust
pub fn embed(&self, texts: &[String], text_type: &str) -> Result<Vec<Vec<f32>>, String>;
```

**处理步骤：**

1. **Query 前缀**：若 `text_type == "query"`，在文本前加 BGE 前缀 `"为这个句子生成表示以用于检索相关文章："`。文档端不加。
2. **Tokenize**：调用 `tokenizer.encode_batch(processed, true)`（第二个参数 `add_special_tokens`）
3. **Tensor 构造**：从 encoding 提取 `input_ids`、`attention_mask`、`token_type_ids`，构造 `ndarray::Array2<i64>`
4. **ONNX 推理**：`session.run(ort::inputs![input_ids_tensor, mask_tensor, type_ids_tensor])`
5. **输出提取**：从 outputs[0] 提取 `last_hidden_state`，shape 为 `[batch, seq_len, hidden_dim]`
6. **Mean Pooling**：沿 token 维度用 attention_mask 做加权平均 → `[batch, hidden_dim]`
7. **L2 归一化**：每行向量除以其 L2 范数

### 4.3 运行时路径解析

模型文件位于 `src-tauri/models/`，运行时通过 `app.path().resource_dir()` 获取。dev 模式下该函数返回 `target/debug/`，模型文件在 `target/debug/models/`（由 Tauri 资源配置自动复制）。

```rust
// 在 setup() 中：
let dir = app_handle.path().resource_dir()?;
let model = dir.join("models").join("model.onnx");
let tok = dir.join("models").join("tokenizer.json");
```

---

## 5. 向量存储与检索 (`rag/store.rs`)

### 5.1 存储文件

```
%APPDATA%/desktoppet/kb_store.json
```

**为什么是 JSON 而不是 SQLite/二进制：**
- 个人 KB 规模小（<1000 文件），JSON 足够
- 用户可读、可手动删除，方便排查问题
- 启动时一次性加载到内存，检索时零磁盘 IO

**KbStore 方法：**

```rust
impl KbStore {
    fn store_path() -> PathBuf;                          // 获取 kb_store.json 完整路径
    pub fn save_to_disk(&self) -> Result<(), String>;    // 序列化并写入磁盘
    pub fn load_from_disk() -> Result<KbStore, String>;  // 从磁盘加载并反序列化
}

// 检索
pub fn search(chunks: &[KbChunk], query_vec: &[f32], top_k: usize) -> Vec<(KbChunk, f32)>;
```

### 5.2 内存缓存策略

- 启动时后台线程加载 `kb_store.json` → `AppState.kb_store`
- 检索时直接从 `Arc<AppState>` 读内存
- 重建索引时替换内存中的 `KbStore` + 同步写回磁盘

### 5.3 余弦相似度检索

实现使用 `ndarray::ArrayView1` 做向量点积和范数计算，遍历所有 chunk 计算余弦相似度，排序后取 top_k。性能：10000 个 chunk 下 < 5ms。

---

## 6. send_chat 变更详情

### 6.1 知识库检索逻辑（send_chat 中新增）

```rust
if is_rag {
    // 1. 获取 embedding 模型和 kb_store
    let emb_guard = app_state_arc.embedding_model.lock().unwrap();
    let kb_guard = app_state_arc.kb_store.lock().unwrap();

    if let (Some(ref model), Some(ref kb_store)) = (emb_guard.as_ref(), kb_guard.as_ref()) {
        // 2. 向量化查询
        let query_vecs = model.embed(&[message.clone()], "query").unwrap_or_default();
        if let Some(query_vec) = query_vecs.into_iter().next() {
            // 3. 检索 Top-3，相似度阈值 0.3
            for (chunk, score) in &search(&kb_store.chunks, &query_vec, 3) {
                if *score > 0.3 {
                    kb_context.push_str(&format!(
                        "---\n[来源: {}]\n{}\n", chunk.source_file, chunk.text
                    ));
                    kb_sources.push(chunk.source_file.clone());
                }
            }
        }
    }

    // 4. 全部不相关时的降级提示
    if kb_sources.is_empty() && emb_guard.is_some() && kb_guard.is_some() {
        kb_context = String::from(
            "（知识库中未找到与用户问题相关的内容）\n\
             请在回复开头加上：\"知识库里没找到相关内容，以下是来自大模型的回答：\""
        );
    }
}
```

### 6.2 上下文管理

- 聊天模式：消息 push 到 `conversation`
- 知识库模式：消息 push 到 `rag_conversation`
- 两种模式各自独立维护上下文，切换时调用 `clear_conversation` 清空

### 6.3 System Prompt 构造

```rust
let mut system_prompt = SYSTEM_PROMPT.replace("{pet_name}", &pet_name);
if is_rag && !kb_context.is_empty() {
    system_prompt.push_str("\n\n参考以下知识库内容回答用户问题：\n");
    system_prompt.push_str(&kb_context);
}
```

### 6.4 流结束 emit

```rust
let _ = app.emit("pet-chat-done", ChatDoneEvent {
    sources: kb_sources,  // KB 模式非空，聊天模式为空 vec
});
```

---

## 7. 新增 Tauri 命令

### 7.1 build_knowledge_base

防重入保护（`is_indexing` 原子标志），取 embedding 模型后 spawn `tauri::async_runtime::spawn` 异步执行 `build_index`。完成后更新 `kb_store` 或通过 `pet-chat-error` 通知前端失败原因。

### 7.2 get_kb_status

返回 `KbStatus`，额外包含 `model_loaded` 字段，前端据此区分"等待模型加载"和"未建索引"两种状态。

### 7.3 open_index_dir

打开 `%APPDATA%/desktoppet/` 目录（Windows 用 `explorer` 命令）。

---

## 8. 构建系统 (`build.rs`)

`build.rs` 在编译时自动下载 ONNX Runtime 1.24.2：

1. 检查 exe 同级目录是否已有 `onnxruntime.dll` + `onnxruntime_providers_shared.dll`，有则跳过
2. 无则使用 PowerShell 从 `https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-win-x64-1.24.2.zip` 下载
3. `Expand-Archive` 解压，`Copy-Item` 复制 DLL 到 profile 目录
4. `Remove-Item` 清理临时 ZIP 和解压目录

**为什么不用 `ort` crate 的 `download-binaries` feature**：`load-dynamic` feature 会设置 `disable-linking`，导致 ort-sys 构建脚本在第一行就 return，完全跳过下载逻辑。两者互斥。

---

## 9. 线程模型

| 线程 | 启动方式 | 启动时机 | 退出方式 |
|------|---------|---------|---------|
| CPU 监控 | `std::thread::spawn` | `run()` 入口 | `shutdown_flag` 检查 |
| 键盘钩子 | `std::thread::spawn` | `InputMonitor::new()` | `PostThreadMessage(WM_QUIT)` |
| 状态机 tick | `std::thread::spawn` | setup() | `shutdown_flag` 检查 |
| 光标轮询 | `std::thread::spawn` | setup() | `shutdown_flag` 检查 |
| **模型加载** | `std::thread::spawn` | setup() | 自然结束 |
| **KB Store 加载** | `std::thread::spawn` | setup() | 自然结束 |
| **索引构建** | `tauri::async_runtime::spawn` | 按需 | 自然结束 |

> **模型加载为什么用 `std::thread::spawn`**：91MB 模型的 `commit_from_file()` 是同步阻塞操作，放在 Tokio 异步运行时中会阻塞 worker 线程。独立 OS 线程避免此问题。

---

## 10. 进程退出机制

所有退出路径（托盘 quit、右键菜单退出、CloseRequested、animation_finished("shutdown")）统一流程：

1. 设置 `shutdown_flag` → 信号通知后台线程退出
2. 发送 `WM_QUIT` 给键盘钩子线程的消息循环
3. 调用 `force_exit_process()`：
   - Windows：`TerminateProcess(GetCurrentProcess(), 0)` — 跳过 DLL 清理，避免 WebView2 的 `DLL_PROCESS_DETACH` 死锁
   - 其他平台：`std::process::exit(0)`

---

## 11. 通信协议总结

### 11.1 Commands

| Command | 类型 | 说明 |
|---------|------|------|
| `send_chat` | **修改** | 新增可选参数 `rag_mode: Option<bool>` |
| `build_knowledge_base` | **新增** | `{ folder: String }` → 后台索引 + 进度推送 |
| `get_kb_status` | **新增** | 返回 `KbStatus`（含 `model_loaded` 字段） |
| `open_index_dir` | **新增** | 打开 `%APPDATA%/desktoppet/` |

### 11.2 Events

| Event | 类型 | Payload |
|-------|------|---------|
| `kb-index-progress` | **新增** | `{ current, total, current_file }` |
| `kb-index-done` | **新增** | `{ file_count, chunk_count }` |
| `pet-chat-done` | **修改** | `()` → `{ sources: Vec<String> }` |

---

## 12. Cargo.toml 与 tauri.conf.json 变更

### 12.1 新增依赖

```toml
[dependencies]
ort = { version = "2.0.0-rc.12", features = ["load-dynamic"] }
tokenizers = "0.21"
ndarray = "0.17"
uuid = { version = "1", features = ["v4"] }

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_System_Performance",
    "Win32_System_Threading",        # GetCurrentProcess, TerminateProcess, ExitProcess
    "Win32_System_SystemInformation",
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",  # PostThreadMessageW, WM_QUIT, GetCursorPos
    "Win32_System_LibraryLoader",    # LoadLibraryW
] }
```

### 12.2 打包资源配置

```json
{
  "bundle": {
    "resources": [
      "models/*"
    ]
  }
}
```

将 `src-tauri/models/` 下的所有文件（`model.onnx`、`tokenizer.json`、`vocab.txt`、`config.json` 等）打包至安装目录。`onnxruntime.dll` 不在 models/ 中，由 build.rs 单独处理。

---

## 13. 错误处理矩阵

| 场景 | 处理 |
|------|------|
| 模型文件未找到 / 加载失败 | 启动时记录日志，`embedding_model` 保持 `None`，前端显示"等待 AI 模型加载完成..." |
| kb_store.json 不存在 | 视为未建索引，`get_kb_status` 返回 `indexed=false` |
| 索引中单个文件读取失败 | 跳过该文件，继续索引其余文件 |
| 索引过程中用户再次触发重建 | `is_indexing` 原子标志拦截，返回 "索引正在进行中" |
| 索引构建失败（模型错误等） | emit `pet-chat-error`，前端显示红色错误信息 + 重置 `kbIndexing` 状态 |
| 检索时 kb_store 为 None | send_chat 降级为纯 LLM 调用 |
| LLM API 错误 | 走现有 `pet-chat-error` 机制，不变 |
| ONNX Runtime DLL 缺失 | 模型加载失败 → 前端显示"等待 AI 模型加载完成..." → "重建索引"按钮禁用 |
| 进程退出 | `TerminateProcess` 强制退出，跳过 WebView2 DLL 清理 |

---

> v1.1.0 基于实际开发结果更新。详细踩坑记录参见 `RAG_TROUBLESHOOTING.md`。
