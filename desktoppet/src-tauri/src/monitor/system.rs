use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// CPU 使用率（简化实现：使用全局采样）
/// 在后台线程中每秒采样一次，主线程读取最新值
static CPU_PERCENT: AtomicU32 = AtomicU32::new(0);

/// 全局关闭信号，通知 CPU monitor 线程退出
static MONITOR_SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// 通知所有 monitor 线程退出
pub fn signal_shutdown() {
    eprintln!("[monitor::system] signal_shutdown() called");
    MONITOR_SHUTDOWN.store(true, Ordering::SeqCst);
}

/// 获取当前 CPU 使用率
pub fn get_cpu_percent() -> f32 {
    f32::from_bits(CPU_PERCENT.load(Ordering::Relaxed))
}

/// 启动 CPU 监测后台线程
pub fn start_cpu_monitor() {
    std::thread::spawn(|| {
        #[cfg(windows)]
        {
            cpu_monitor_loop_windows();
        }
        #[cfg(not(windows))]
        {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    });
}

#[cfg(windows)]
fn cpu_monitor_loop_windows() {
    use std::time::Duration;

    // 使用 GetSystemTimes 计算 CPU 使用率
    use windows::Win32::System::Threading::GetSystemTimes;
    use windows::Win32::Foundation::FILETIME;

    let mut prev_idle = 0u64;
    let mut prev_kernel = 0u64;
    let mut prev_user = 0u64;
    let mut first = true;

    eprintln!("[monitor::system] CPU monitor thread started");
    loop {
        if MONITOR_SHUTDOWN.load(Ordering::SeqCst) {
            eprintln!("[monitor::system] CPU monitor thread exiting");
            break;
        }

        unsafe {
            let mut idle_time = FILETIME::default();
            let mut kernel_time = FILETIME::default();
            let mut user_time = FILETIME::default();

            if GetSystemTimes(
                Some(&mut idle_time),
                Some(&mut kernel_time),
                Some(&mut user_time),
            ).is_ok() {
                let idle = filetime_to_u64(&idle_time);
                let kernel = filetime_to_u64(&kernel_time);
                let user = filetime_to_u64(&user_time);

                if !first {
                    let idle_diff = idle - prev_idle;
                    let kernel_diff = kernel - prev_kernel;
                    let user_diff = user - prev_user;

                    let total = kernel_diff + user_diff;
                    let busy = total - idle_diff;

                    let percent = if total > 0 {
                        (busy as f64 / total as f64 * 100.0) as f32
                    } else {
                        0.0
                    };

                    CPU_PERCENT.store(percent.to_bits(), Ordering::Relaxed);
                }

                prev_idle = idle;
                prev_kernel = kernel;
                prev_user = user;
                first = false;
            }
        }

        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(windows)]
fn filetime_to_u64(ft: &windows::Win32::Foundation::FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

/// 获取前台窗口进程名
pub fn get_foreground_process_name() -> String {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0 == std::ptr::null_mut() {
                return String::new();
            }

            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return String::new();
            }

            if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let mut buf = [0u16; 260];
                let mut size = buf.len() as u32;

                use windows::Win32::System::Threading::QueryFullProcessImageNameW;
                use windows::Win32::System::Threading::PROCESS_NAME_FORMAT;

                if QueryFullProcessImageNameW(
                    handle,
                    PROCESS_NAME_FORMAT(0),
                    windows::core::PWSTR(buf.as_mut_ptr()),
                    &mut size,
                ).is_ok() {
                    let path = String::from_utf16_lossy(&buf[..size as usize]);
                    // 提取文件名
                    if let Some(name) = path.rsplit('\\').next() {
                        return name.to_string();
                    }
                    return path;
                }
            }
        }
    }
    String::new()
}
