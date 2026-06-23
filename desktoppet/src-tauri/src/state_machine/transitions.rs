use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::states::PetState;
use crate::monitor::SystemSnapshot;

pub struct EvalContext<'a> {
    pub current: PetState,
    pub snapshot: &'a SystemSnapshot,
    /// 演示模式强制状态
    pub force: Option<PetState>,
    /// LLM 等待中
    pub is_thinking: Arc<AtomicBool>,
    /// LLM 流式输出中
    pub is_talking: Arc<AtomicBool>,
    /// CPU >= 90% 已持续秒数
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

    // 3. THINK 保护
    if ctx.is_thinking.load(Ordering::SeqCst) {
        return PetState::Think;
    }

    // 4. TALK 保护
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

    // 6. SLEEPING：键鼠空闲 >= 10 分钟
    if snap.idle_seconds >= 600 {
        return PetState::Sleeping;
    }

    // 7. SWEATING：CPU >= 90% 持续 30 秒
    if snap.cpu_percent >= 90.0 && ctx.cpu_high_seconds >= 30 {
        return PetState::Sweating;
    }
    // SWEATING 退出缓冲：CPU < 70% 但不足 10 秒，保持 SWEATING
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
    // TYPING 退出缓冲：按键不活跃但不足 10 秒，保持 TYPING
    if ctx.current == PetState::Typing && ctx.exit_pending_seconds < 10 {
        return PetState::Typing;
    }

    // 9. 默认
    PetState::Idle
}
