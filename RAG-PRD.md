# RAG 本地知识库问答 — 产品需求文档

> 版本: 1.1.0 | 日期: 2026-05-29 | 状态: 已实现

> v1.1.0 变更：根据实际开发结果更新模型规格、构建流程、状态字段、退出流程等细节。

---

## 1. 产品概述

### 1.1 功能定位

为桌面宠物新增"本地知识库问答"能力。用户在设置中指定一个本地文件夹作为知识库，宠物自动索引其中的文档内容。当用户在聊天面板切换至"知识库模式"后提问时，系统从本地索引中检索相关内容，结合 LLM 生成基于私有知识库的回答。

### 1.2 核心价值

- 让宠物能够回答"我的文档里写了什么"，而非只能通用聊天
- 所有文档处理在本地完成，不上传至任何云端服务
- 与现有聊天功能无缝共存，通过模式切换区分两种使用场景

### 1.3 设计原则

- 完全本地索引与检索，隐私零泄露
- 轻量化，适合笔记本部署
- 打包即用，ONNX Runtime DLL 由构建脚本自动下载，最终用户无需配置额外环境
- 不影响现有聊天、动画、状态机等功能

---

## 2. 用户故事

| 场景 | 描述 |
|------|------|
| 查阅笔记 | 用户有一堆学习笔记（md/txt），切到知识库模式问"我之前记的 React 生命周期怎么说的？" |
| 公司文档 | 用户把公司制度 PDF 放进知识库文件夹，问"年假规定是几天？" |
| 项目文档 | 用户对项目文档建索引，问"这个项目的 API 端口号是多少？" |
| 一般聊天 | 用户切回聊天模式问"今天天气怎么样"，宠物正常聊天，不触发检索 |
| 知识库未命中 | 用户问"周末去哪玩"，知识库里全是技术文档，检索无果，LLM 告知无相关内容后自由回答 |

---

## 3. 功能需求

### 3.1 模式切换

1. 聊天面板输入框下方新增一个**左右滑动开关**，左侧标注"聊天"，右侧标注"知识库"
2. 滑块停在左侧时，面板处于**聊天模式**，行为与现有聊天完全一致
3. 滑块滑到右侧时，面板切换至**知识库模式**，此后发送的每条消息都会先检索本地知识库
4. 切换模式时**自动清空当前对话上下文**（两个模式各自独立，互不干扰）
5. 关闭聊天窗口后，模式**重置为"聊天"**，下次打开默认回到聊天模式
6. 清空对话按钮和关窗行为不变，均清除当前模式下的上下文

### 3.2 知识库不可用时的保护

以下任一条件不满足时，知识库模式不可用：

1. 设置中已配置知识库文件夹路径
2. 知识库已完成索引构建（`kb_store.json` 存在且有效）
3. Embedding 模型已加载完成

任一条件不满足时的行为：
- 前端设置面板中显示具体状态（"等待 AI 模型加载完成..." / "尚未建索引 — 请先选择文件夹并点击保存"）
- 索引失败时显示红色错误信息
- 模型未加载或未建索引时，**"重建索引"按钮禁用**

### 3.3 知识库配置（设置面板）

在设置面板中新增"知识库"区域，包含以下项目：

| 项目 | 说明 |
|------|------|
| 知识库文件夹 | 文本输入框 + "浏览"按钮，选择本地文件夹路径 |
| 索引状态文字 | 显示"已索引 X 个文件，共 Y 个片段 · 上次索引 YYYY-MM-DD HH:MM"；从未建索引且模型未就绪时显示"等待 AI 模型加载完成..."，模型就绪但未建索引时显示"尚未建索引 — 请先选择文件夹并点击保存" |
| 错误信息 | 索引失败时显示红色错误文字（如"索引启动失败: Embedding 模型未初始化"） |
| 重建索引按钮 | 手动触发全量重建索引，模型未加载时禁用 |
| 打开索引目录按钮 | 在文件资源管理器中打开 `%APPDATA%/desktoppet/` 目录 |

### 3.4 索引构建

1. **初次建索引**：用户选好知识库文件夹后，点击保存时若文件夹路径变更则**自动触发**建索引
2. **后续更新**：文件夹内容变更后，需用户手动点击"重建索引"
3. **索引进度**：建索引过程中实时显示进度（如"正在索引... 15/50 个文件"）
4. **支持的文件类型**：txt, md, pdf, docx, pptx, py, js, ts, jsx, tsx, json, html, css, rs, java, go, c, cpp, h, hpp, yaml, yml, toml, xml, sql, sh, bat, ps1, log, csv, vue, svelte（复用现有 file_handler 模块）
5. **分块策略**：每块最多 512 字符（约 250-300 个中文字），相邻块重叠 50 字符，防止关键信息在边界处被切断
6. **向量化**：使用本地 ONNX Embedding 模型将所有文本块转为向量
7. **存储**：向量索引和文本块存储至 `%APPDATA%/desktoppet/kb_store.json`，应用启动时在后台线程加载至内存
8. **文件计数**：仅统计实际产生文本块的文件数（无法提取文本的文件不计入）

### 3.5 知识检索

1. 知识库模式下，用户发送消息时自动触发检索
2. 将用户问题向量化（添加 BGE query 前缀），与索引中所有文本块计算余弦相似度
3. 返回相似度最高的 Top-3 个文本块（附带来源文件名）
4. 相似度阈值 0.3，低于此值的片段不采用

### 3.6 答案生成

1. 检索到的 Top-3 文本块注入 system prompt，格式如下：

```
参考以下知识库内容回答用户问题：
---
[来源: xxxx.txt]
文本块1内容
---
[来源: yyyy.md]
文本块2内容
---
[来源: zzzz.pdf]
文本块3内容
---
```

2. 如果检索到的内容与问题**不相关**（相似度全部低于 0.3），LLM 回复开头须加：
   > "知识库里没找到相关内容，以下是来自大模型的回答："

3. 检索到的内容与问题**相关**时，LLM 基于检索内容生成自然对话式回答

4. 回答末尾以灰色小字展示参考来源：`参考：xxx.md, yyy.pdf`

### 3.7 上下文管理

- 聊天模式和知识库模式**各自维护独立的对话上下文**
- 切换模式时自动清空上一模式的上下文
- 清空对话按钮清空当前模式的上下文
- 关闭聊天窗口时清除当前模式的上下文
- 对话历史上限：每种模式独立保持最多 10 轮（复用现有 `ConversationContext`）

---

## 4. 技术方案

### 4.1 整体架构

```
┌─ 设置面板 ─────────────────────────────────────┐
│  选择文件夹 → 保存时自动建索引 → 显示进度+状态      │
│  手动点"重建"→ 全量重建索引                       │
│  "打开索引目录" → 资源管理器打开 %APPDATA%/desktoppet/ │
│  错误信息：红色文字显示具体失败原因                 │
└────────────────────────────────────────────────┘
                      ↓
┌─ 知识库索引管线 (Rust) ──────────────────────────┐
│  扫描文件 → 提取文本 → 段落分块 → 向量化 → 存本地    │
│  (复用 file_handler)  (chunking)  (embedding)      │
└────────────────────────────────────────────────┘
                      ↓
┌─ 聊天面板 ──────────────────────────────────────┐
│  滑块切"知识库" → 发消息                           │
│    → 问题向量化 → 余弦相似度检索 Top-3              │
│    → 注入 system prompt → 调用 LLM → 流式返回       │
│    → 回答末尾追加参考来源                           │
│  滑块切"聊天" → 发消息                              │
│    → 完全走现有流程，不做检索                        │
└────────────────────────────────────────────────┘
```

### 4.2 技术选型

| 组件 | 选型 | 实际版本 | 说明 |
|------|------|---------|------|
| Embedding 模型 | BGE 系列 ONNX 模型 | model.onnx (~91MB) | BGE 中文语义模型 |
| 推理引擎 | ONNX Runtime | 1.24.2 | 由 build.rs 自动从 GitHub Releases 下载 |
| Rust 绑定 | `ort` crate | 2.0.0-rc.12 | `load-dynamic` feature，运行时动态加载 DLL |
| 分词器 | HuggingFace tokenizers | 0.21 | Rust 原生 crate，加载 tokenizer.json |
| 向量存储 | JSON 文件 + 内存缓存 | — | 启动时加载到内存，检索时纯内存计算 |
| 检索算法 | 余弦相似度 | — | `ndarray` 0.17 实现 |
| 分块方式 | 段落分割 + 滑动窗口 | — | 按双换行分段，512 字符上限，50 字符重叠 |

### 4.3 新增 Rust 依赖 (`Cargo.toml`)

```toml
ort = { version = "2.0.0-rc.12", features = ["load-dynamic"] }
tokenizers = "0.21"
ndarray = "0.17"
uuid = { version = "1", features = ["v4"] }
```

### 4.4 构建时自动下载 ONNX Runtime DLL

`build.rs` 在编译时自动从微软 GitHub 下载 ONNX Runtime 1.24.2：

- 下载地址：`https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-win-x64-1.24.2.zip`
- 解压后将 `onnxruntime.dll` 和 `onnxruntime_providers_shared.dll` 复制到 exe 同级目录
- 下载后自动清理临时 ZIP 和解压目录
- 已下载过的 DLL 不重复下载（检测文件已存在则跳过）

### 4.5 新增 Rust 模块结构

```
src-tauri/src/rag/
├── mod.rs          # 模块入口 + 3 个 Tauri 命令 + build_index 管线
├── chunking.rs     # 文档分块（段落分割 + 滑动窗口）
├── embedding.rs    # ONNX 模型加载 + 文本向量化（mean pooling + L2 归一化）
└── store.rs        # kb_store.json 读写 + 余弦相似度检索
```

### 4.6 现有模块复用

| 现有模块 | 复用方式 |
|----------|----------|
| `file_handler::process_file()` | 索引时提取各类文件文本内容（不改动） |
| `llm::providers::*` | 知识库模式下的 LLM 调用，与聊天模式走同一套 Provider |
| `llm::context::ConversationContext` | 两种模式各维护一个独立实例 |
| `store::config::AppConfig` | 新增 `kb_folder` 字段，沿用现有存取机制 |
| `lib.rs::send_chat()` | 新增 `rag_mode` 参数，在 rag_mode=true 时先检索再调用 LLM |

---

## 5. 界面设计变更

### 5.1 聊天面板 (PetChat.vue)

- 输入框下方新增模式切换滑块（聊天 ↔ 知识库）
- 知识库模式下 assistant 消息气泡下方展示参考来源

### 5.2 设置面板 (PetSettings.vue)

- "知识库"区域：文件夹选择、索引状态、错误信息、重建索引按钮、打开索引目录按钮
- 状态颜色：绿色（已索引）、灰色（未建索引/等待模型）、红色（错误）
- 模型未加载时"重建索引"按钮禁用

---

## 6. 数据流

### 6.1 索引流程

```
用户选择文件夹 + 点保存（设置面板）
  │
  ▼
Rust: build_knowledge_base(folder)  [后台异步线程]
  │
  ├─ 1. 递归扫描文件夹，收集所有支持类型的文件路径
  │
  ├─ 2. 逐个调用 process_file(path) 提取文本
  │     → 每处理一个文件，emit kb-index-progress 事件给前端
  │
  ├─ 3. 对每个文件的文本调用 chunk_text(text)
  │     → 按段落分割，512 字符/块，50 字符重叠
  │
  ├─ 4. 批量调用 embedding_model.embed(chunks, "document")
  │     → 每批 32 条，文本块转为向量
  │
  └─ 5. 保存为 kb_store.json → 更新 AppState.kb_store
        → emit kb-index-done
```

### 6.2 检索与生成流程

```
用户在知识库模式下发送消息
  │
  ▼
Rust: send_chat(message, rag_mode: true)
  │
  ├─ 1. 向量化用户问题: query_vec = model.embed([message], "query")
  │     → query 文本自动加 BGE 前缀
  │
  ├─ 2. 余弦相似度检索 → Top-3（阈值 0.3）
  │
  ├─ 3. 相关片段注入 system prompt（附来源文件名）
  │
  ├─ 4. 调用 LLM Provider → 流式返回
  │
  └─ 5. 流结束后 emit pet-chat-done { sources: [...] }
```

---

## 7. API / 接口设计

### 7.1 新增 Tauri Commands

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `build_knowledge_base` | `folder: String` | `()` | 异步扫描并建索引，通过 event 推送进度和结果 |
| `get_kb_status` | — | `KbStatus` | 返回当前索引状态和模型加载状态 |
| `open_index_dir` | — | `()` | 在文件资源管理器中打开 `%APPDATA%/desktoppet/` |

**KbStatus 类型**：
```rust
struct KbStatus {
    indexed: bool,           // 是否已建过索引
    file_count: usize,       // 已索引文件数
    chunk_count: usize,      // 文本块总数
    last_indexed: String,    // 上次索引时间（ISO 8601）
    kb_folder: String,       // 当前配置的知识库文件夹
    model_loaded: bool,      // Embedding 模型是否已加载（v1.1 新增）
}
```

### 7.2 修改现有 Tauri Command

| Command | 修改内容 |
|---------|----------|
| `send_chat` | 新增参数 `rag_mode: Option<bool>`。为 true 时先检索本地 KB 再构造请求 |

### 7.3 新增 Tauri Events (Rust → Vue)

| Event | Payload | 时机 |
|-------|---------|------|
| `kb-index-progress` | `{ current: usize, total: usize, current_file: String }` | 建索引过程中每处理完一个文件推送 |
| `kb-index-done` | `{ file_count: usize, chunk_count: usize }` | 索引完成 |
| `pet-chat-done` | `{ sources: Vec<String> }` | 流结束，KB 模式下非空，聊天模式下为空数组（v1.1 修改，原为 `()`） |

---

## 8. 非功能需求

### 8.1 性能

| 指标 | 实际 |
|------|------|
| 索引速度 | 取决于文件数量和模型推理速度（91MB 模型首次推理需要预热） |
| 单次检索 | 1000 个文本块以下 < 200ms |
| 模型加载 | 应用启动时在独立 OS 线程中异步加载，不阻塞 UI |
| 内存增量 | KB store 常驻内存 < 50MB（1 万块以内） |

### 8.2 安全与隐私

- 所有文档内容仅在本地处理，索引过程不上传任何第三方
- 检索结果仅注入至 LLM 请求（与普通聊天一致，走用户配置的 Provider）
- 索引文件 `kb_store.json` 存于用户 AppData 目录，不暴露至公共路径

### 8.3 体积

| 增量项 | 大小 |
|--------|------|
| model.onnx | ~91MB |
| tokenizer.json | ~450KB |
| vocab.txt | ~128KB |
| ONNX Runtime DLL | ~14MB（build.rs 自动下载） |
| **合计（不含编译产物）** | **约 106MB** |

---

## 9. 开发任务

| 编号 | 任务 | 涉及文件 | 状态 |
|------|------|----------|------|
| 1 | 准备 ONNX 模型文件 | `src-tauri/models/` | ✅ |
| 2 | 添加 Rust 依赖 | `Cargo.toml` | ✅ |
| 3 | 实现文档分块模块 | `src-tauri/src/rag/chunking.rs` | ✅ |
| 4 | 实现文本向量化模块 | `src-tauri/src/rag/embedding.rs` | ✅ |
| 5 | 实现向量存储与检索模块 | `src-tauri/src/rag/store.rs` | ✅ |
| 6 | 实现 RAG 模块入口 + Tauri 命令 | `src-tauri/src/rag/mod.rs` | ✅ |
| 7 | build.rs 自动下载 ONNX Runtime DLL | `src-tauri/build.rs` | ✅ |
| 8 | 修改 send_chat 支持 rag_mode | `src-tauri/src/lib.rs` | ✅ |
| 9 | 模式切换独立管理两种上下文 | `src-tauri/src/lib.rs` | ✅ |
| 10 | AppConfig 增加 kb_folder 字段 | `src-tauri/src/store/config.rs` | ✅ |
| 11 | 设置面板增加 KB 配置区域 | `src/components/PetSettings.vue` | ✅ |
| 12 | 聊天面板增加模式切换 + 参考来源 | `src/components/PetChat.vue` | ✅ |
| 13 | 进程退出机制修复（TerminateProcess） | `src-tauri/src/lib.rs` | ✅ |

---

## 10. 风险与对策

| 风险 | 对策 | 实际结果 |
|------|------|---------|
| ONNX Runtime DLL 版本不兼容致加载挂起 | 由 build.rs 自动下载经过验证的 1.24.2 版本 | ✅ 通过 build.rs 自动下载解决 |
| `ort` crate 的 `load-dynamic` 于 `download-binaries` 互斥 | 在 build.rs 中用 PowerShell 手动下载 | ✅ build.rs 中实现 |
| 大文件首次索引耗时长 | 异步执行 + 实时进度推送 + 可中途关闭设置面板 | ✅ |
| 索引文件 `kb_store.json` 过大 | 启动时后台线程加载，加载完成前知识库模式不可用 | ✅ |
| WebView2 清理回调死锁导致无法退出 | 使用 `TerminateProcess` 替代 `ExitProcess` | ✅ |
| 编译目录膨胀 | `cargo clean` + build.rs 自动清理临时文件 | ✅ |
| 索引失败时用户看不到错误 | KbStatus 新增 model_loaded 字段 + 前端红色错误显示 | ✅ |

---

## 附录 A：配置变更摘要

`AppConfig` 新增字段：

```rust
pub struct AppConfig {
    // ... 现有字段不变 ...
    pub kb_folder: Option<String>,  // 知识库文件夹路径，None 表示未配置
}
```

---

## 附录 B：术语表

| 术语 | 说明 |
|------|------|
| RAG | Retrieval-Augmented Generation，检索增强生成。先检索再让 LLM 基于检索结果回答 |
| Embedding / 向量化 | 将文本转换为一串数字（向量），语义相近的文本向量也相近 |
| ONNX Runtime | 轻量级 AI 模型推理引擎，纯 C/C++ 实现，无需 Python |
| BGE | BAAI 开源的中文 Embedding 模型系列 |
| 余弦相似度 | 衡量两个向量方向有多接近的数学方法，值域 [-1, 1]，越接近 1 越相似 |
| 分块 (Chunking) | 将长文档切成小段，每段作为一个检索单位 |
| System Prompt | 发给 LLM 的系统级指令，定义角色行为和规则 |
| Top-K | 检索返回相似度最高的 K 个结果，此处 K=3 |
| Mean Pooling | 将 token 级别的向量通过注意力掩码加权平均得到句子级别向量 |
| L2 Normalization | 将向量长度归一化为 1，确保余弦相似度计算稳定 |

---

> v1.1.0 基于实际开发结果更新。详细踩坑记录参见 `RAG_TROUBLESHOOTING.md`。
