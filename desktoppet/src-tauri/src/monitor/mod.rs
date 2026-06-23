pub mod input;
pub mod system;

use chrono::Timelike;
use serde::Serialize;

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

/// 采集一次系统快照
pub fn capture_system_snapshot(input_monitor: &input::InputMonitor) -> SystemSnapshot {
    let idle_seconds = input::get_idle_seconds();
    let recent_key_count = input_monitor.get_recent_key_count();
    let cpu_percent = system::get_cpu_percent();
    let local_hour = chrono::Local::now().hour();
    let foreground_process = system::get_foreground_process_name();

    SystemSnapshot {
        idle_seconds,
        recent_key_count,
        cpu_percent,
        local_hour,
        foreground_process,
    }
}
