mod bubble;
mod file_handler;
mod llm;
mod monitor;
mod rag;
mod state_machine;
mod store;

use bubble::BubbleManager;
use file_handler::{process_file as do_process_file, FileContent};
use llm::context::ConversationContext;
use llm::providers::{DeepSeekProvider, KimiProvider, QwenProvider};
use llm::{ChatErrorEvent, ChatMessage, ImageAttachment, LlmProvider, SYSTEM_PROMPT};
use state_machine::{evaluate_state, EvalContext, PetState};
use store::AppConfig;

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};

// ===== Debug Logging =====

macro_rules! log_shutdown {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        eprintln!("{}", msg);
        let log_path = std::env::var("APPDATA")
            .unwrap_or_else(|_| ".".to_string());
        let log_dir = std::path::PathBuf::from(&log_path).join("desktoppet");
        let _ = std::fs::create_dir_all(&log_dir);
        let log_file = log_dir.join("shutdown.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
        {
            let _ = writeln!(f, "{}", msg);
        }
    }};
}

/// 强制终止当前进程。
/// TerminateProcess 绕过 DLL_PROCESS_DETACH，避免 WebView2 清理死锁。
#[cfg(windows)]
fn force_exit_process() -> ! {
    log_shutdown!("[shutdown] force_exit via TerminateProcess");
    std::io::stderr().flush().ok();
    unsafe {
        use windows::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
        let _ = TerminateProcess(GetCurrentProcess(), 0);
    }
    // 理论上不会执行到这里，但万一 TerminateProcess 失败，回退
    std::process::exit(0);
}

#[cfg(not(windows))]
fn force_exit_process() -> ! {
    std::io::stderr().flush().ok();
    std::process::exit(0);
}

// ===== Event Payloads =====

#[derive(Clone, Serialize)]
struct StateChangedEvent {
    state: String,
    previous: String,
}

// ===== App State =====

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub current_state: Mutex<PetState>,
    pub prev_state: Mutex<PetState>,
    pub is_thinking: Arc<AtomicBool>,
    pub is_talking: Arc<AtomicBool>,
    pub force_state: Mutex<Option<PetState>>,
    pub input_monitor: monitor::input::InputMonitor,
    pub bubble_manager: Mutex<BubbleManager>,
    pub conversation: Mutex<ConversationContext>,
    pub rag_conversation: Mutex<ConversationContext>,
    pub kb_store: Arc<Mutex<Option<rag::store::KbStore>>>,
    pub embedding_model: Arc<Mutex<Option<rag::embedding::EmbeddingModel>>>,
    pub is_indexing: Arc<AtomicBool>,
    pub shutdown_flag: Arc<AtomicBool>,
}

// ===== State Info for Demo Mode =====

#[derive(Clone, Serialize)]
struct StateInfo {
    id: String,
    label: String,
    description: String,
}

fn get_all_state_infos() -> Vec<StateInfo> {
    vec![
        StateInfo {
            id: "appear".into(),
            label: "APPEAR — 入场动画".into(),
            description: "应用启动时播放".into(),
        },
        StateInfo {
            id: "idle".into(),
            label: "IDLE — 默认待机".into(),
            description: "默认状态，呼吸微动".into(),
        },
        StateInfo {
            id: "clicked".into(),
            label: "CLICKED — 点击反应".into(),
            description: "单击宠物触发".into(),
        },
        StateInfo {
            id: "think".into(),
            label: "THINK — 思考中".into(),
            description: "等待 LLM 回复".into(),
        },
        StateInfo {
            id: "talk".into(),
            label: "TALK — 说话中".into(),
            description: "LLM 流式输出中".into(),
        },
        StateInfo {
            id: "sleeping".into(),
            label: "SLEEPING — 睡眠".into(),
            description: "键鼠空闲 ≥ 10 分钟".into(),
        },
        StateInfo {
            id: "typing".into(),
            label: "TYPING — 打字中".into(),
            description: "2 秒内键盘事件 ≥ 5 次".into(),
        },
        StateInfo {
            id: "worried".into(),
            label: "WORRIED — 深夜提醒".into(),
            description: "23:00-05:00 + 键盘活跃".into(),
        },
        StateInfo {
            id: "sweating".into(),
            label: "SWEATING — CPU 告警".into(),
            description: "CPU ≥ 90% 持续 30 秒".into(),
        },
        StateInfo {
            id: "shutdown".into(),
            label: "SHUTDOWN — 退出".into(),
            description: "退出动画".into(),
        },
    ]
}

// ===== Helper =====

fn parse_state(s: &str) -> Result<PetState, String> {
    match s {
        "appear" => Ok(PetState::Appear),
        "idle" => Ok(PetState::Idle),
        "clicked" => Ok(PetState::Clicked),
        "think" => Ok(PetState::Think),
        "talk" => Ok(PetState::Talk),
        "sleeping" => Ok(PetState::Sleeping),
        "typing" => Ok(PetState::Typing),
        "worried" => Ok(PetState::Worried),
        "sweating" => Ok(PetState::Sweating),
        "shutdown" => Ok(PetState::Shutdown),
        _ => Err(format!("未知状态: {}", s)),
    }
}

/// 切换状态并发出事件 + 触发气泡
/// 调用前必须已经持有锁并写入了新状态
fn emit_state_change_with_bubble(
    app: &AppHandle,
    app_state: &Arc<AppState>,
    new_state: PetState,
    prev_state: PetState,
) {
    let _ = app.emit(
        "pet-state-changed",
        StateChangedEvent {
            state: new_state.as_str().to_string(),
            previous: prev_state.as_str().to_string(),
        },
    );

    // 触发气泡
    let pet_name = {
        let cfg = app_state.config.lock().unwrap();
        cfg.pet_name.clone()
    };
    let bubble = {
        let manager = app_state.bubble_manager.lock().unwrap();
        manager.on_state_change(new_state.as_str(), prev_state.as_str(), &pet_name)
    };
    if let Some(payload) = bubble {
        let _ = app.emit("pet-bubble", payload);
    }
}

// ===== Tauri Commands =====

#[tauri::command]
async fn get_pet_state(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let current = state.current_state.lock().unwrap();
    Ok(current.as_str().to_string())
}

#[tauri::command]
async fn get_config(state: State<'_, Arc<AppState>>) -> Result<AppConfig, String> {
    let config = state.config.lock().unwrap();
    Ok(config.clone())
}

#[tauri::command]
async fn save_config(app: AppHandle, state: State<'_, Arc<AppState>>, config: AppConfig) -> Result<(), String> {
    config.save()?;
    let mut current_config = state.config.lock().unwrap();
    *current_config = config.clone();
    drop(current_config);
    // 通知前端配置已更新（用于实时同步 pet_size 等 UI 设置）
    let _ = app.emit("pet-config-changed", config);
    Ok(())
}

#[tauri::command]
async fn save_pet_position(state: State<'_, Arc<AppState>>, x: i32, y: i32) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();
    config.pet_position = Some(store::config::Position { x, y });
    config.save()?;
    Ok(())
}

#[tauri::command]
async fn notify_click(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let app_state_arc = state.inner().clone();

    let mut current = app_state_arc.current_state.lock().unwrap();
    let mut prev = app_state_arc.prev_state.lock().unwrap();

    if !current.is_transient() && *current != PetState::Think && *current != PetState::Talk {
        let from = *current;
        *prev = from;
        *current = PetState::Clicked;
        drop(current);
        drop(prev);

        emit_state_change_with_bubble(&app, &app_state_arc, PetState::Clicked, from);
    }

    Ok(())
}

#[tauri::command]
async fn animation_finished(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    state_name: String,
) -> Result<(), String> {
    let app_state_arc = state.inner().clone();

    let mut current = app_state_arc.current_state.lock().unwrap();
    let prev = app_state_arc.prev_state.lock().unwrap();

    match state_name.as_str() {
        "appear" if *current == PetState::Appear => {
            let from = *current;
            *current = PetState::Idle;
            drop(current);
            drop(prev);
            emit_state_change_with_bubble(&app, &app_state_arc, PetState::Idle, from);
        }
        "clicked" if *current == PetState::Clicked => {
            let restore_to = *prev;
            let from = *current;
            *current = restore_to;
            drop(current);
            drop(prev);
            emit_state_change_with_bubble(&app, &app_state_arc, restore_to, from);
        }
        "shutdown" if *current == PetState::Shutdown => {
            log_shutdown!("[shutdown] animation_finished('shutdown') — frontend callback received");

            let was_already_set = app_state_arc.shutdown_flag.swap(true, Ordering::SeqCst);
            log_shutdown!("[shutdown] shutdown_flag was_already_set={}", was_already_set);

            monitor::system::signal_shutdown();
            monitor::input::signal_shutdown();

            force_exit_process();
        }
        _ => {}
    }

    Ok(())
}

#[tauri::command]
async fn force_state(
    _app: AppHandle,
    state: State<'_, Arc<AppState>>,
    target: String,
) -> Result<(), String> {
    let target_state = parse_state(&target)?;
    // 拒绝瞬态状态：APPEAR / CLICKED / SHUTDOWN 由专用流程触发
    // 演示模式只允许切换循环状态，避免误触发不可逆操作
    if target_state.is_transient() {
        return Err(format!(
            "演示模式不能切到瞬态状态 {}（APPEAR/CLICKED/SHUTDOWN 由专用命令处理）",
            target_state.as_str()
        ));
    }
    let mut force = state.force_state.lock().unwrap();
    *force = Some(target_state);
    Ok(())
}

#[tauri::command]
async fn resume_auto_state(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut force = state.force_state.lock().unwrap();
    *force = None;
    Ok(())
}

#[tauri::command]
async fn get_all_states() -> Result<Vec<StateInfo>, String> {
    // 演示模式只暴露非瞬态状态：APPEAR/CLICKED/SHUTDOWN 不能强制切换
    Ok(get_all_state_infos()
        .into_iter()
        .filter(|s| {
            let st = parse_state(&s.id).ok();
            match st {
                Some(s) => !s.is_transient(),
                None => false,
            }
        })
        .collect())
}

#[tauri::command]
async fn start_talking(_state: State<'_, Arc<AppState>>) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
async fn end_talking(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.is_thinking.store(false, Ordering::SeqCst);
    state.is_talking.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
async fn trigger_shutdown(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let app_state_arc = state.inner().clone();

    log_shutdown!("[shutdown] trigger_shutdown called");

    // 立即设置退出标志，通知所有后台线程退出
    app_state_arc.shutdown_flag.store(true, Ordering::SeqCst);
    monitor::system::signal_shutdown();
    monitor::input::signal_shutdown();

    let mut current = app_state_arc.current_state.lock().unwrap();
    let from = *current;
    *current = PetState::Shutdown;
    drop(current);

    emit_state_change_with_bubble(&app, &app_state_arc, PetState::Shutdown, from);

    // 兜底：如果前端动画回调未触发 animation_finished，5 秒后强制退出
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(5));
        force_exit_process();
    });

    Ok(())
}

#[tauri::command]
async fn get_default_pet_folder() -> Result<String, String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir)
        .parent()
        .ok_or("无法定位项目根目录")?
        .join("src")
        .join("assets")
        .join("pets")
        .join("default");
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
async fn reload_bubbles(state: State<'_, Arc<AppState>>, pet_folder: String) -> Result<(), String> {
    let new_manager = BubbleManager::load(&pet_folder)?;
    let mut manager = state.bubble_manager.lock().unwrap();
    *manager = new_manager;
    Ok(())
}

/// Vue 启动时调用，触发当前状态的 on_enter 气泡
/// 解决 APPEAR 状态启动时没机会触发气泡的问题
#[tauri::command]
async fn trigger_initial_bubble(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let app_state_arc = state.inner().clone();
    let current = {
        let c = app_state_arc.current_state.lock().unwrap();
        *c
    };
    let pet_name = {
        let cfg = app_state_arc.config.lock().unwrap();
        cfg.pet_name.clone()
    };
    let bubble = {
        let manager = app_state_arc.bubble_manager.lock().unwrap();
        manager.on_state_change(current.as_str(), current.as_str(), &pet_name)
    };
    if let Some(payload) = bubble {
        let _ = app.emit("pet-bubble", payload);
    }
    Ok(())
}

// ===== LLM Commands =====

#[tauri::command]
async fn send_chat(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    message: String,
    images: Option<Vec<ImageAttachment>>,
    appended_text: Option<String>,
    rag_mode: Option<bool>,
) -> Result<(), String> {
    let app_state_arc = state.inner().clone();
    let is_rag = rag_mode.unwrap_or(false);

    // === 知识库检索（仅在 KB 模式下执行）===
    let mut kb_sources: Vec<String> = Vec::new();
    let mut kb_context = String::new();

    if is_rag {
        let emb_guard = app_state_arc.embedding_model.lock().unwrap();
        let kb_guard = app_state_arc.kb_store.lock().unwrap();

        if let (Some(ref model), Some(ref kb_store)) = (emb_guard.as_ref(), kb_guard.as_ref()) {
            let query_vecs = model.embed(&[message.clone()], "query").unwrap_or_default();
            if let Some(query_vec) = query_vecs.into_iter().next() {
                for (chunk, score) in &rag::store::search(&kb_store.chunks, &query_vec, 3) {
                    if *score > 0.3 {
                        kb_context.push_str(&format!(
                            "---\n[来源: {}]\n{}\n",
                            chunk.source_file, chunk.text
                        ));
                        kb_sources.push(chunk.source_file.clone());
                    }
                }
            }
        }

        if kb_sources.is_empty() && emb_guard.is_some() && kb_guard.is_some() {
            kb_context = String::from(
                "（知识库中未找到与用户问题相关的内容）\n\
                 请在回复开头加上：\"知识库里没找到相关内容，以下是来自大模型的回答：\"",
            );
        }
    }

    // 取出 provider / api_key / model / pet_name
    let (provider_name, api_key, model, pet_name) = {
        let cfg = app_state_arc.config.lock().unwrap();
        (
            cfg.provider.clone(),
            cfg.current_api_key().to_string(),
            cfg.model.clone(),
            cfg.pet_name.clone(),
        )
    };

    if api_key.is_empty() {
        let msg = format!("Provider {} 尚未配置 API Key", provider_name);
        let _ = app.emit("pet-chat-error", ChatErrorEvent { message: msg.clone() });
        return Err(msg);
    }

    // 进入 THINK 状态
    app_state_arc.is_thinking.store(true, Ordering::SeqCst);
    app_state_arc.is_talking.store(false, Ordering::SeqCst);

    // 把当前用户消息（含 appended_text）push 进对应模式的上下文
    let user_message_full = match &appended_text {
        Some(text) if !text.is_empty() => format!(
            "{}\n\n以下是被处理文件的文本内容：\n---\n{}\n---",
            message, text
        ),
        _ => message.clone(),
    };
    if is_rag {
        let mut conv = app_state_arc.rag_conversation.lock().unwrap();
        conv.push_user(user_message_full.clone());
    } else {
        let mut conv = app_state_arc.conversation.lock().unwrap();
        conv.push_user(user_message_full.clone());
    }

    // 构造发送给 API 的 messages：system + history
    let mut api_messages: Vec<ChatMessage> = Vec::new();
    let mut system_prompt = SYSTEM_PROMPT.replace("{pet_name}", &pet_name);
    if is_rag && !kb_context.is_empty() {
        system_prompt.push_str("\n\n参考以下知识库内容回答用户问题：\n");
        system_prompt.push_str(&kb_context);
    }
    api_messages.push(ChatMessage {
        role: "system".to_string(),
        content: system_prompt,
    });
    if is_rag {
        let conv = app_state_arc.rag_conversation.lock().unwrap();
        for m in conv.messages() {
            api_messages.push(m.clone());
        }
    } else {
        let conv = app_state_arc.conversation.lock().unwrap();
        for m in conv.messages() {
            api_messages.push(m.clone());
        }
    }

    let images_list = images.unwrap_or_default();
    eprintln!("[send_chat] images count: {}, model: {}, provider: {}", images_list.len(), model, provider_name);
    let provider_name_for_dispatch = provider_name.clone();
    let app_for_task = app.clone();
    let app_state_for_task = Arc::clone(&app_state_arc);
    let kb_sources_for_emit = kb_sources.clone();

    // 异步发送（不阻塞 command）
    tauri::async_runtime::spawn(async move {
        // Drop guard: 无论正常退出还是 panic，都确保清理 is_thinking/is_talking
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
        let _guard = ThinkingGuard {
            is_thinking: Arc::clone(&app_state_for_task.is_thinking),
            is_talking: Arc::clone(&app_state_for_task.is_talking),
        };

        let provider: Box<dyn LlmProvider> = match provider_name_for_dispatch.as_str() {
            "deepseek" => Box::new(DeepSeekProvider::new(api_key)),
            "kimi" => Box::new(KimiProvider::new(api_key)),
            "qwen" => Box::new(QwenProvider::new(api_key)),
            _ => {
                let msg = format!("未知 Provider: {}", provider_name_for_dispatch);
                let _ = app_for_task.emit("pet-chat-error", ChatErrorEvent { message: msg });
                return; // _guard drops here → clears flags
            }
        };

        let result = if !images_list.is_empty() {
            provider
                .chat_stream_with_images(
                    api_messages,
                    images_list,
                    &model,
                    app_for_task.clone(),
                    Arc::clone(&app_state_for_task.is_thinking),
                    Arc::clone(&app_state_for_task.is_talking),
                )
                .await
        } else {
            provider
                .chat_stream(
                    api_messages,
                    &model,
                    app_for_task.clone(),
                    Arc::clone(&app_state_for_task.is_thinking),
                    Arc::clone(&app_state_for_task.is_talking),
                )
                .await
        };

        match result {
            Ok(assistant_text) => {
                if !assistant_text.is_empty() {
                    if is_rag {
                        let mut conv = app_state_for_task.rag_conversation.lock().unwrap();
                        conv.push_assistant(assistant_text);
                    } else {
                        let mut conv = app_state_for_task.conversation.lock().unwrap();
                        conv.push_assistant(assistant_text);
                    }
                }
            }
            Err(e) => {
                let _ = app_for_task
                    .emit("pet-chat-error", ChatErrorEvent { message: e });
            }
        }

        let _ = app_for_task.emit(
            "pet-chat-done",
            rag::ChatDoneEvent {
                sources: kb_sources_for_emit,
            },
        );
    });

    Ok(())
}

#[tauri::command]
async fn clear_conversation(
    state: State<'_, Arc<AppState>>,
    rag_mode: Option<bool>,
) -> Result<(), String> {
    let is_rag = rag_mode.unwrap_or(false);
    if is_rag {
        let mut conv = state.rag_conversation.lock().unwrap();
        conv.clear();
    } else {
        let mut conv = state.conversation.lock().unwrap();
        conv.clear();
    }
    Ok(())
}

#[tauri::command]
async fn process_file(path: String) -> Result<FileContent, String> {
    do_process_file(&path)
}

/// 设置窗口鼠标穿透：true=穿透到桌面（不接受鼠标），false=正常接受鼠标
#[tauri::command]
async fn set_passthrough(app: AppHandle, ignore: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_ignore_cursor_events(ignore)
            .map_err(|e| format!("set_ignore_cursor_events 失败: {}", e))?;
    }
    Ok(())
}

/// 强制锁定窗口非穿透（面板可见时用）。返回当前 force_block 状态。
/// 由 Vue 侧在打开/关闭任意面板时调用
static FORCE_BLOCK_PASSTHROUGH: AtomicBool = AtomicBool::new(false);

// 宠物全局位置（屏幕坐标），由前端在 mount 后和 resize 后调用 update_pet_bounds 设置
use std::sync::atomic::AtomicI32;
static PET_CENTER_X: AtomicI32 = AtomicI32::new(0);
static PET_CENTER_Y: AtomicI32 = AtomicI32::new(0);
static PET_RADIUS: AtomicI32 = AtomicI32::new(0);

#[tauri::command]
async fn set_force_block_passthrough(app: AppHandle, block: bool) -> Result<(), String> {
    FORCE_BLOCK_PASSTHROUGH.store(block, Ordering::SeqCst);
    if block {
        // 面板打开时立即关闭穿透
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_ignore_cursor_events(false);
        }
    }
    Ok(())
}

#[tauri::command]
async fn update_pet_bounds(center_x: i32, center_y: i32, radius: i32) -> Result<(), String> {
    PET_CENTER_X.store(center_x, Ordering::Relaxed);
    PET_CENTER_Y.store(center_y, Ordering::Relaxed);
    PET_RADIUS.store(radius, Ordering::Relaxed);
    Ok(())
}

/// 移动主窗口到指定屏幕坐标（物理像素）
#[tauri::command]
async fn move_window_to(app: AppHandle, x: i32, y: i32) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_position(tauri::PhysicalPosition { x, y })
            .map_err(|e| format!("set_position 失败: {}", e))?;
    }
    Ok(())
}

/// 获取主窗口当前屏幕坐标（物理像素）
#[tauri::command]
async fn get_window_position(app: AppHandle) -> Result<store::config::Position, String> {
    if let Some(window) = app.get_webview_window("main") {
        let pos = window
            .outer_position()
            .map_err(|e| format!("outer_position 失败: {}", e))?;
        Ok(store::config::Position { x: pos.x, y: pos.y })
    } else {
        Err("找不到主窗口".to_string())
    }
}

/// 启动光标位置轮询：50ms 检查一次鼠标是否在窗口区域内
/// 在窗口区域内 → 关闭穿透；在窗口外（且无面板强制） → 开启穿透
fn start_cursor_poll(app_handle: AppHandle, shutdown_flag: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut last_ignore = false;
        loop {
            if shutdown_flag.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));

            let force_block = FORCE_BLOCK_PASSTHROUGH.load(Ordering::SeqCst);

            #[cfg(windows)]
            {
                use windows::Win32::Foundation::POINT;
                use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

                let mut cursor = POINT { x: 0, y: 0 };
                let cursor_ok = unsafe { GetCursorPos(&mut cursor).is_ok() };
                if !cursor_ok {
                    continue;
                }

                // force_block = true 时永远不穿透（面板打开中）
                if force_block {
                    if last_ignore {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.set_ignore_cursor_events(false);
                        }
                        last_ignore = false;
                    }
                    continue;
                }

                // 获取宠物边界检查
                let cx = PET_CENTER_X.load(Ordering::Relaxed);
                let cy = PET_CENTER_Y.load(Ordering::Relaxed);
                let radius = PET_RADIUS.load(Ordering::Relaxed);

                let ignore = if radius == 0 {
                    false // 还没设过，保守不穿透
                } else {
                    let dx = (cursor.x - cx).abs();
                    let dy = (cursor.y - cy).abs();
                    // 鼠标在宠物外接矩形外 → 穿透
                    dx > radius || dy > radius
                };

                if ignore != last_ignore {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.set_ignore_cursor_events(ignore);
                    }
                    last_ignore = ignore;
                }
            }
            #[cfg(not(windows))]
            {
                let _ = app_handle;
                break;
            }
        }
    });
}

// ===== State Machine Loop =====

fn start_state_machine(app_handle: AppHandle, app_state: Arc<AppState>) {
    std::thread::spawn(move || {
        let mut cpu_high_seconds: u32 = 0;
        let mut exit_pending_seconds: u32 = 0;

        loop {
            std::thread::sleep(Duration::from_secs(1));

            if app_state.shutdown_flag.load(Ordering::SeqCst) {
                break;
            }

            let snapshot = monitor::capture_system_snapshot(&app_state.input_monitor);

            if snapshot.cpu_percent >= 90.0 {
                cpu_high_seconds = cpu_high_seconds.saturating_add(1);
            } else {
                cpu_high_seconds = 0;
            }

            let force = {
                let f = app_state.force_state.lock().unwrap();
                *f
            };

            // 读 + 评估 + 写：在同一锁守卫内完成
            let mut current_guard = app_state.current_state.lock().unwrap();
            let current = *current_guard;

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
                force,
                is_thinking: Arc::clone(&app_state.is_thinking),
                is_talking: Arc::clone(&app_state.is_talking),
                cpu_high_seconds,
                exit_pending_seconds,
            };

            let next = evaluate_state(&ctx);

            if next != current {
                *current_guard = next;
                drop(current_guard);
                exit_pending_seconds = 0;

                emit_state_change_with_bubble(&app_handle, &app_state, next, current);
            }
        }
    });
}

// ===== System Tray =====

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_hide = MenuItem::with_id(app, "show_hide", "显示/隐藏", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "open_settings", "设置", true, None::<&str>)?;
    let demo = MenuItem::with_id(app, "open_demo", "状态切换", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_hide, &settings, &demo, &quit])?;

    TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show_hide" => {
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
            "open_settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = app.emit("open-settings", ());
                }
            }
            "open_demo" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = app.emit("open-demo", ());
                }
            }
            "quit" => {
                log_shutdown!("[shutdown] tray 'quit' clicked");
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    // 提前通知 monitor 线程退出
                    monitor::system::signal_shutdown();
                    monitor::input::signal_shutdown();

                    if let Some(state) = app_handle.try_state::<Arc<AppState>>() {
                        let app_state_arc = state.inner().clone();
                        app_state_arc.shutdown_flag.store(true, Ordering::SeqCst);
                        let mut current = app_state_arc.current_state.lock().unwrap();
                        let from = *current;
                        *current = PetState::Shutdown;
                        drop(current);
                        emit_state_change_with_bubble(
                            &app_handle,
                            &app_state_arc,
                            PetState::Shutdown,
                            from,
                        );
                        // 兜底：如果前端没有在 5 秒内调用 animation_finished，强制退出
                        std::thread::spawn(|| {
                            std::thread::sleep(std::time::Duration::from_secs(5));
                            force_exit_process();
                        });
                    } else {
                        app_handle.exit(0);
                    }
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // 双击托盘图标显示窗口
            if let TrayIconEvent::DoubleClick { .. } = event {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
            // Fallback: 1x1 透明图标（理论上 default_window_icon 应已存在）
            tauri::image::Image::new_owned(vec![0, 0, 0, 0], 1, 1)
        }))
        .build(app)?;

    Ok(())
}

// ===== Entry Point =====

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = AppConfig::load();

    monitor::system::start_cpu_monitor();

    let app_state = Arc::new(AppState {
        config: Mutex::new(config),
        current_state: Mutex::new(PetState::Appear),
        prev_state: Mutex::new(PetState::Appear),
        is_thinking: Arc::new(AtomicBool::new(false)),
        is_talking: Arc::new(AtomicBool::new(false)),
        force_state: Mutex::new(None),
        input_monitor: monitor::input::InputMonitor::new(),
        bubble_manager: Mutex::new(BubbleManager::empty()),
        conversation: Mutex::new(ConversationContext::default()),
        rag_conversation: Mutex::new(ConversationContext::default()),
        kb_store: Arc::new(Mutex::new(None)),
        embedding_model: Arc::new(Mutex::new(None)),
        is_indexing: Arc::new(AtomicBool::new(false)),
        shutdown_flag: Arc::new(AtomicBool::new(false)),
    });

    let state_for_machine = Arc::clone(&app_state);
    let state_for_setup = Arc::clone(&app_state);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_pet_state,
            get_config,
            save_config,
            save_pet_position,
            notify_click,
            animation_finished,
            force_state,
            resume_auto_state,
            get_all_states,
            start_talking,
            end_talking,
            trigger_shutdown,
            get_default_pet_folder,
            reload_bubbles,
            trigger_initial_bubble,
            send_chat,
            clear_conversation,
            process_file,
            set_passthrough,
            set_force_block_passthrough,
            update_pet_bounds,
            move_window_to,
            get_window_position,
            rag::build_knowledge_base,
            rag::get_kb_status,
            rag::open_index_dir,
            rag::clear_kb_index,
            rag::clear_all_indexes,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            log_shutdown!("[setup] Tauri app setup started");

            // 启动时尝试加载默认角色的气泡
            let default_folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .map(|p| p.join("src").join("assets").join("pets").join("default"));
            if let Some(folder) = default_folder {
                let folder_str = folder.to_string_lossy().to_string();
                if let Ok(manager) = BubbleManager::load(&folder_str) {
                    if let Ok(mut bm) = state_for_setup.bubble_manager.lock() {
                        *bm = manager;
                    }
                }
            }

            // 系统托盘
            if let Err(e) = setup_tray(&handle) {
                eprintln!("[tray] 创建系统托盘失败: {}", e);
            }

            // 处理窗口关闭请求（用户从任务栏关闭 / taskkill / Alt+F4 等）
            if let Some(window) = handle.get_webview_window("main") {
                let app_state_close = Arc::clone(&state_for_setup);
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        log_shutdown!("[shutdown] CloseRequested received");
                        app_state_close.shutdown_flag.store(true, Ordering::SeqCst);
                        monitor::system::signal_shutdown();
                        monitor::input::signal_shutdown();
                        force_exit_process();
                    }
                });
                log_shutdown!("[setup] CloseRequested handler registered");
            }

            // 启动光标轮询（实现"非宠物区穿透到桌面"）
            start_cursor_poll(handle.clone(), Arc::clone(&state_for_setup.shutdown_flag));

            // === 加载 Embedding 模型 ===
            let emb_ref = Arc::clone(&state_for_setup.embedding_model);
            let app_handle_model = handle.clone();
            std::thread::spawn(move || {
                match app_handle_model.path().resource_dir() {
                    Ok(dir) => {
                        let model = dir.join("models").join("model.onnx");
                        let tok = dir.join("models").join("tokenizer.json");
                        log_shutdown!("[rag] loading model from {}", model.display());
                        match rag::embedding::EmbeddingModel::load(
                            &model.to_string_lossy(),
                            &tok.to_string_lossy(),
                        ) {
                            Ok(m) => {
                                *emb_ref.lock().unwrap() = Some(m);
                                log_shutdown!("[rag] model loaded successfully");
                            }
                            Err(e) => {
                                log_shutdown!("[rag] model load failed: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        log_shutdown!("[rag] resource_dir not available: {}", e);
                    }
                }
            });

            // === 异步加载 kb_store.json ===
            let kb_ref = Arc::clone(&state_for_setup.kb_store);
            std::thread::spawn(move || {
                match rag::store::KbStore::load_from_disk() {
                    Ok(store) => {
                        *kb_ref.lock().unwrap() = Some(store);
                    }
                    Err(_) => {
                        // kb_store.json 不存在是正常情况（未建索引）
                    }
                }
            });

            start_state_machine(handle, state_for_machine);
            log_shutdown!("[setup] Setup complete");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
