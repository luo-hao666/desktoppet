use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use std::collections::VecDeque;

#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// 键盘钩子线程的 Windows 线程 ID，用于退出时发送 WM_QUIT
#[cfg(windows)]
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// 通知键盘钩子线程退出（向钩子线程发送 WM_QUIT）
pub fn signal_shutdown() {
    #[cfg(windows)]
    {
        let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
        eprintln!("[monitor::input] signal_shutdown() called, hook_thread_id={}", tid);
        if tid != 0 {
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
                let _ = PostThreadMessageW(tid, WM_QUIT, None, None);
                eprintln!("[monitor::input] WM_QUIT posted to thread {}", tid);
            }
        }
    }
}

/// 获取键鼠空闲秒数
pub fn get_idle_seconds() -> u64 {
    #[cfg(windows)]
    {
        unsafe {
            let mut lii = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if GetLastInputInfo(&mut lii).as_bool() {
                let tick = windows::Win32::System::SystemInformation::GetTickCount();
                let idle_ms = tick.wrapping_sub(lii.dwTime);
                return (idle_ms / 1000) as u64;
            }
        }
        0
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// 键盘事件监控器
/// 使用低级别键盘钩子统计键盘事件频率（仅计数，不记录内容）
pub struct InputMonitor {
    /// 键盘事件时间戳队列（最近 N 秒的事件时间）
    key_events: Arc<std::sync::Mutex<VecDeque<Instant>>>,
}

impl InputMonitor {
    pub fn new() -> Self {
        let key_events = Arc::new(std::sync::Mutex::new(VecDeque::new()));

        #[cfg(windows)]
        {
            let events_clone = Arc::clone(&key_events);
            // 启动键盘钩子线程
            std::thread::spawn(move || {
                Self::run_keyboard_hook(events_clone);
            });
        }

        Self { key_events }
    }

    /// 获取最近 2 秒内的键盘事件数
    pub fn get_recent_key_count(&self) -> u32 {
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(2);

        if let Ok(mut events) = self.key_events.lock() {
            // 清除超过 2 秒的旧事件
            while let Some(front) = events.front() {
                if *front < cutoff {
                    events.pop_front();
                } else {
                    break;
                }
            }
            events.len() as u32
        } else {
            0
        }
    }

    #[cfg(windows)]
    fn run_keyboard_hook(key_events: Arc<std::sync::Mutex<VecDeque<Instant>>>) {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowsHookExW, GetMessageW, CallNextHookEx,
            WH_KEYBOARD_LL, HHOOK, MSG,
        };
        use windows::Win32::Foundation::{WPARAM, LPARAM, LRESULT};

        // 用 thread-local 存储 key_events 引用
        thread_local! {
            static KEY_EVENTS: std::cell::RefCell<Option<Arc<std::sync::Mutex<VecDeque<Instant>>>>> = std::cell::RefCell::new(None);
        }

        KEY_EVENTS.with(|cell| {
            *cell.borrow_mut() = Some(key_events);
        });

        unsafe extern "system" fn hook_proc(
            n_code: i32,
            w_param: WPARAM,
            l_param: LPARAM,
        ) -> LRESULT {
            if n_code >= 0 {
                KEY_EVENTS.with(|cell| {
                    if let Some(ref events) = *cell.borrow() {
                        if let Ok(mut queue) = events.lock() {
                            queue.push_back(Instant::now());
                        }
                    }
                });
            }
            unsafe { CallNextHookEx(HHOOK::default(), n_code, w_param, l_param) }
        }

        unsafe {
            HOOK_THREAD_ID.store(
                windows::Win32::System::Threading::GetCurrentThreadId(),
                Ordering::SeqCst,
            );

            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(hook_proc),
                None,
                0,
            );

            if hook.is_ok() {
                eprintln!("[monitor::input] keyboard hook installed, entering message loop (thread_id={})",
                    windows::Win32::System::Threading::GetCurrentThreadId());
                // 消息循环保持钩子存活
                // GetMessageW 在收到 WM_QUIT 时返回 FALSE，自动退出循环
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    // 保持消息循环
                }
                eprintln!("[monitor::input] message loop exited, hook thread terminating");
            } else {
                eprintln!("[monitor::input] SetWindowsHookExW failed");
            }
        }
    }
}
