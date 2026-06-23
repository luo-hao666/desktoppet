# RAG 知识库功能 — 踩坑记录与技术总结

> 日期: 2026-05-29 | 项目: DesktopPet 桌宠

---

## 概述

RAG（检索增强生成）是桌宠项目的核心功能之一，允许用户选择本地文件夹，宠物自动索引文档内容，并在知识库模式下基于私有文档回答问题。

实现过程中遇到了若干棘手问题，涉及 ONNX Runtime DLL 兼容性、构建系统配置、进程退出死锁等多个层面。本文档记录这些问题及解决方案。

---

## 时间线

| 阶段 | 问题 | 耗时 | 状态 |
|------|------|------|------|
| 1 | 模型加载后 `Session::builder()` 挂起 | ~4h | ✅ 已解决 |
| 2 | `std::process::exit(0)` 无法终止进程 | ~3h | ✅ 已解决 |
| 3 | `ort` crate 的 `download-binaries` 与 `load-dynamic` 互斥 | ~1h | ✅ 已解决 |
| 4 | 进程退出后 Chrome_WidgetWin_0 错误 + 残留进程 | ~2h | ✅ 已解决 |
| 5 | 前端错误消息不显示，用户看不到失败原因 | ~0.5h | ✅ 已解决 |
| 6 | `target/` 编译目录膨胀至 13.9GB | ~0.5h | ✅ 已解决 |

---

## 问题 1：模型加载时 ONNX Runtime DLL 挂起

### 现象

`EmbeddingModel::load()` 调用 `Session::builder()?.commit_from_file()` 后永不返回，进程卡死。无论在主线程还是后台线程调用都一样。

### 排查过程

1. 怀疑线程死锁 → 在主线程、任何线程 spawn 之前调用，依然挂起 ✗
2. 怀疑 DLL 路径错误 → 路径正确，文件存在 ✗
3. 怀疑 `libloading` 问题 → `ort` crate 内部就是标准 `LoadLibraryW` ✗
4. 对比 DLL 文件大小：旧 DLL 13.5MB，官方 1.24.2 版本 14MB → **文件不同！**

### 根因

`models/` 目录中的 `onnxruntime.dll` 版本有问题。该 DLL 在 `DllMain` 或 TLS 回调初始化期间挂起（与 [ONNX Runtime GitHub Issue #25670](https://github.com/microsoft/onnxruntime/issues/25670) 描述的现象一致：TLS callback 在 `LoadLibrary` 时访问空指针导致崩溃/挂起）。

### 解决方案

在 `build.rs` 中自动从微软 GitHub Releases 下载 **ONNX Runtime 1.24.2**（与 `ort` crate v2.0.0-rc.12 兼容的版本），解压后将 `onnxruntime.dll` 和 `onnxruntime_providers_shared.dll` 复制到 exe 同级目录，然后清理临时文件。

关键代码（`build.rs`）：
```rust
// 使用 PowerShell 下载并解压 ONNX Runtime
let zip_url = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-win-x64-1.24.2.zip";
// → Invoke-WebRequest 下载 → Expand-Archive 解压 → Copy-Item DLL → Remove-Item 清理
```

### 教训

- `ort` crate 的 `load-dynamic` feature 会启用 `disable-linking`，导致构建脚本跳过 `download-binaries`。这两个 feature **不能同时工作**。
- 如果使用 `load-dynamic`，必须自行提供 ONNX Runtime DLL。
- 不要随便从网上找 DLL 放进去——到官方 GitHub Releases 下载匹配版本。

---

## 问题 2：进程退出后无法终止

### 现象

右键菜单点"退出"后，命令行卡在运行中，进程不结束。Ctrl+C 也无法清理，留下残留进程。

### 排查过程

1. 发现两个后台线程（CPU 监控、键盘钩子）没有退出机制 → 添加 `shutdown_flag` ✓
2. 信号发出后线程退出了，但进程仍存活
3. `app.exit(0)` 调用后进程不退出
4. `std::process::exit(0)` 调用后进程仍不退出！
5. 直接调用 Windows API `ExitProcess(0)` → **依然不退出！**
6. 最终发现是 WebView2 的 `DLL_PROCESS_DETACH` 清理回调死锁

### 根因

`ExitProcess` 会触发所有 DLL 的 `DLL_PROCESS_DETACH` 回调，WebView2 的清理代码在此期间死锁，导致进程永远无法终止。

### 解决方案

使用 `TerminateProcess` 替代 `ExitProcess`。`TerminateProcess` 跳过所有 DLL 清理回调，直接终止进程。

```rust
#[cfg(windows)]
fn force_exit_process() -> ! {
    unsafe {
        use windows::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
        let _ = TerminateProcess(GetCurrentProcess(), 0);
    }
    std::process::exit(0); // 回退
}
```

所有退出路径（托盘 quit、右键菜单退出、CloseRequested、animation_finished("shutdown")）统一调用此函数。

### 教训

- `ExitProcess` ≠ 无条件立即退出。DLL 的 `DLL_PROCESS_DETACH` 可能挂起。
- 对于 GUI 应用（尤其是 WebView2），`TerminateProcess` 是更可靠的退出方式。
- 需要给后台线程一个短暂的清理窗口（设置 shutdown_flag → 等待几百毫秒 → TerminateProcess）。

---

## 问题 3：编译目录膨胀至 13.9GB

### 现象

项目文件夹 13.9GB，代码本身只有 ~200MB。

### 根因

- `target/debug/deps/` 6.3GB — Rust debug 编译产物（含大量 .pdb 调试符号）
- `target/debug/incremental/` 1.2GB — 增量编译缓存
- `target/debug/onnxruntime_extract/` 380MB — build.rs 下载的 ONNX Runtime ZIP 解压后未清理
- `target/release/` 1.6GB — release 构建产物

### 解决方案

1. `cargo clean` 清理所有编译产物（释放 ~11GB）
2. 修复 `build.rs`：下载解压后自动删除临时 ZIP 和解压目录

### 教训

- build.rs 中创建的任何临时文件都需要显式清理
- 定期 `cargo clean` 无害，只是下次编译慢一些

---

## 问题 4：前端错误消息不显示

### 现象

用户在设置中选择知识库文件夹后，索引创建失败，但前端没有任何错误提示，只显示"等待 AI 模型加载完成..."或"尚未建索引"。

### 根因

1. `build_knowledge_base` 命令返回的同步错误虽然被 `catch` 了，但只打了 `console.warn`，未显示在 UI 中
2. 异步索引过程中的错误通过 `pet-chat-error` 事件发送，但设置面板未监听此事件
3. 模型加载状态未反馈给前端

### 解决方案

- `PetSettings.vue` 新增 `kbError` 状态变量，显示在索引状态区域（红色文字）
- 监听 `pet-chat-error` 事件，过滤索引相关错误并展示
- `KbStatus` 新增 `model_loaded` 字段，前端据此显示"等待 AI 模型加载完成..."或"尚未建索引"
- 模型未加载时禁用"重建索引"按钮

---

## 技术要点总结

### 依赖选型

| 组件 | 选型 | 注意事项 |
|------|------|---------|
| 推理引擎 | ONNX Runtime 1.24.2 | 通过 build.rs 自动下载，不需要用户手动安装 |
| Rust 绑定 | `ort` crate v2.0.0-rc.12 | 必须使用 `load-dynamic` feature（避免链接时 CRT 不匹配） |
| 分词器 | `tokenizers` crate v0.21 | HuggingFace 标准 tokenizer |
| 向量运算 | `ndarray` crate v0.17 | 余弦相似度 + mean pooling |

### 线程模型

| 线程 | 启动方式 | 退出方式 |
|------|---------|---------|
| 模型加载 | `std::thread::spawn` | 自然结束（加载完成或失败） |
| 索引构建 | `tauri::async_runtime::spawn` | 自然结束 |
| KB Store 加载 | `std::thread::spawn` | 自然结束 |

### 为什么模型加载用 `std::thread::spawn` 而非 `tauri::async_runtime::spawn`

91MB 的 ONNX 模型加载是同步阻塞操作（`Session::builder()?.commit_from_file()`），放在 Tokio 异步运行时中会阻塞 worker 线程，影响其他异步任务。使用独立 OS 线程避免此问题。

### 为什么 `EmbeddingModel` 使用 `Arc<Mutex<Session>>`

`ort` v2 的 `Session` 不是 `Sync`（因为 ONNX Runtime 的 C API 内部有线程局部状态），必须用 `Mutex` 包装以确保线程安全。

### 为什么 `kb_store` 和 `embedding_model` 用 `Arc<Mutex<Option<T>>>`

需要多个地方同时持有引用（setup 闭包、命令处理函数等），而 `tauri::State` 只提供 `&` 引用。外层 `Arc` 允许 clone 共享，内层 `Mutex` 允许内部可变。

---

## 给后续开发者的建议

1. **ONNX Runtime DLL 不要手动管理**：让 build.rs 自动下载，确保版本和平台匹配。
2. **退出流程要彻底**：WebView2 应用用 `TerminateProcess`，不要指望 `ExitProcess` 能正常退出。
3. **`target/` 目录随时可删**：`cargo clean` 只清编译缓存，不影响代码。
4. **异步错误要两条路都通**：同步返回 error 和异步 emit event 都要在前端有对应的处理。
5. **91MB 的模型文件不要放 Git**：用 Git LFS 或 build.rs 自动下载。
