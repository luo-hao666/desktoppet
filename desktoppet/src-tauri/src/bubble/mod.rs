//! 自动气泡系统
//!
//! - 加载 bubbles.json
//! - 在状态切换时按规则触发
//! - 变量替换（{pet_name} / {time}）
//! - 冷却控制

use chrono::{Local, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BubbleRule {
    pub state: String,
    /// "on_enter" | "on_return_from"
    pub trigger: String,
    pub text: Vec<String>,
    pub cooldown_seconds: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BubblePayload {
    pub text: String,
    pub duration_ms: u64,
}

pub struct BubbleManager {
    rules: Vec<BubbleRule>,
    /// 上次触发时间：(state, trigger) -> Instant
    last_trigger: Mutex<HashMap<(String, String), Instant>>,
}

impl BubbleManager {
    /// 从 bubbles.json 加载规则
    pub fn load(pet_folder: &str) -> Result<Self, String> {
        let path = Path::new(pet_folder).join("bubbles.json");
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("读取 bubbles.json 失败: {}", e))?;
        let rules: Vec<BubbleRule> = serde_json::from_str(&content)
            .map_err(|e| format!("解析 bubbles.json 失败: {}", e))?;
        Ok(Self {
            rules,
            last_trigger: Mutex::new(HashMap::new()),
        })
    }

    pub fn empty() -> Self {
        Self {
            rules: Vec::new(),
            last_trigger: Mutex::new(HashMap::new()),
        }
    }

    /// 在状态切换时被调用，返回应触发的气泡（如果有）
    ///
    /// - new_state 进入时：查 trigger=on_enter 的规则
    /// - prev_state 离开时：查 trigger=on_return_from 的规则
    pub fn on_state_change(
        &self,
        new_state: &str,
        prev_state: &str,
        pet_name: &str,
    ) -> Option<BubblePayload> {
        // 优先 on_return_from（让"睡醒"等过场气泡先出）
        if let Some(payload) = self.try_trigger(prev_state, "on_return_from", pet_name) {
            return Some(payload);
        }
        // 然后 on_enter
        if let Some(payload) = self.try_trigger(new_state, "on_enter", pet_name) {
            return Some(payload);
        }
        None
    }

    fn try_trigger(&self, state: &str, trigger: &str, pet_name: &str) -> Option<BubblePayload> {
        let rule = self.rules.iter().find(|r| r.state == state && r.trigger == trigger)?;
        if rule.text.is_empty() {
            return None;
        }

        // 冷却检查
        let key = (state.to_string(), trigger.to_string());
        let now = Instant::now();
        {
            let mut last = self.last_trigger.lock().ok()?;
            if let Some(prev_time) = last.get(&key) {
                let elapsed = now.duration_since(*prev_time).as_secs();
                if elapsed < rule.cooldown_seconds {
                    return None;
                }
            }
            last.insert(key, now);
        }

        // 随机选一个文案
        let idx = pseudo_random_index(rule.text.len());
        let raw = &rule.text[idx];
        let text = render_template(raw, pet_name);

        Some(BubblePayload {
            text,
            duration_ms: rule.duration_ms,
        })
    }
}

/// 简易伪随机：用系统时间纳秒取模
fn pseudo_random_index(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    nanos % len
}

/// 模板变量替换：{pet_name} / {time}
fn render_template(template: &str, pet_name: &str) -> String {
    let now = Local::now();
    let hour = now.hour();
    let time_str = format_chinese_time(hour, now.minute());

    template
        .replace("{pet_name}", pet_name)
        .replace("{time}", &time_str)
}

fn format_chinese_time(hour: u32, _minute: u32) -> String {
    // 简化版："凌晨2点" / "上午9点" / "下午3点" / "晚上10点"
    let (period, h12) = match hour {
        0..=4 => ("凌晨", if hour == 0 { 12 } else { hour }),
        5..=11 => ("上午", hour),
        12 => ("中午", 12),
        13..=17 => ("下午", hour - 12),
        18..=23 => ("晚上", if hour == 18 { 6 } else { hour - 12 }),
        _ => ("", hour),
    };
    format!("{}{}点", period, h12)
}
