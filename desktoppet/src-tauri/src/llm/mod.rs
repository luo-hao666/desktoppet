//! LLM Adapter 层
//! - 三个 Provider: DeepSeek / Kimi / Qwen，全用 OpenAI 兼容 chat/completions
//! - 流式输出通过 Tauri event 推送给前端
//! - THINK / TALK 状态由 Arc<AtomicBool> 跨线程同步

pub mod context;
pub mod providers;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system" | "user" | "assistant"
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageAttachment {
    pub base64: String,
    pub mime: String,
}

#[derive(Clone, Serialize)]
pub struct ChatChunkEvent {
    pub delta: String,
}

#[derive(Clone, Serialize)]
pub struct ChatErrorEvent {
    pub message: String,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
        app_handle: AppHandle,
        is_thinking: Arc<AtomicBool>,
        is_talking: Arc<AtomicBool>,
    ) -> Result<String, String>;

    async fn chat_stream_with_images(
        &self,
        messages: Vec<ChatMessage>,
        images: Vec<ImageAttachment>,
        model: &str,
        app_handle: AppHandle,
        is_thinking: Arc<AtomicBool>,
        is_talking: Arc<AtomicBool>,
    ) -> Result<String, String>;
}

pub const SYSTEM_PROMPT: &str = r#"你是一只桌面宠物，名字叫 {pet_name}。你的性格是活泼可爱、偶尔傲娇。
规则：
1. 回复要简短，一般不超过 100 字，像聊天而不是写文章
2. 语气要可爱、亲切，可以适当使用拟声词和颜文字（但不要过多）
3. 如果用户给你看图片或文件，认真阅读后再回应
4. 不要提自己是 AI，你就是一只真实的桌面宠物
5. 用户叫你做事时（如翻译、解释代码），可以用稍微正式一点的语气回答
6. 但如果用户只是闲聊，保持轻松可爱
"#;

/// 通用 SSE 流式请求处理（OpenAI 兼容格式）
/// 返回完整的 assistant 文本（用于上下文记忆）
pub async fn stream_request(
    api_key: &str,
    endpoint: &str,
    body: serde_json::Value,
    app_handle: AppHandle,
    is_thinking: Arc<AtomicBool>,
    is_talking: Arc<AtomicBool>,
) -> Result<String, String> {
    use futures::StreamExt;
    use tauri::Emitter;

    let client = reqwest::Client::new();

    // 调试：打印发送的请求体摘要（安全截断，避免 UTF-8 边界 panic）
    #[cfg(debug_assertions)]
    {
        let body_str = body.to_string();
        let preview: String = body_str.chars().take(500).collect();
        eprintln!("[stream_request] endpoint: {}", endpoint);
        eprintln!("[stream_request] body ({} chars): {}...", body_str.len(), preview);
    }

    let response = client
        .post(endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            is_thinking.store(false, Ordering::SeqCst);
            is_talking.store(false, Ordering::SeqCst);
            format!("请求失败: {}", e)
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        is_thinking.store(false, Ordering::SeqCst);
        is_talking.store(false, Ordering::SeqCst);
        return Err(format!("HTTP {}: {}", status, text));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut accumulated = String::new();
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
                return Ok(accumulated);
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(content) = parsed["choices"][0]["delta"]["content"].as_str() {
                        if !content.is_empty() {
                            if first_chunk {
                                first_chunk = false;
                                is_thinking.store(false, Ordering::SeqCst);
                                is_talking.store(true, Ordering::SeqCst);
                            }
                            accumulated.push_str(content);
                            let _ = app_handle.emit(
                                "pet-chat-chunk",
                                ChatChunkEvent {
                                    delta: content.to_string(),
                                },
                            );
                        }
                    }
                    if let Some(reason) = parsed["choices"][0]["finish_reason"].as_str() {
                        if reason == "stop" || reason == "length" {
                            is_thinking.store(false, Ordering::SeqCst);
                            is_talking.store(false, Ordering::SeqCst);
                            return Ok(accumulated);
                        }
                    }
                }
            }
        }
    }

    is_thinking.store(false, Ordering::SeqCst);
    is_talking.store(false, Ordering::SeqCst);
    Ok(accumulated)
}

/// 构造多模态 user message：text + image_url 数组
pub fn build_multimodal_user_content(
    text: &str,
    images: &[ImageAttachment],
) -> serde_json::Value {
    use serde_json::json;
    let mut parts = vec![json!({ "type": "text", "text": text })];
    for img in images {
        parts.push(json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:{};base64,{}", img.mime, img.base64)
            }
        }));
    }
    serde_json::Value::Array(parts)
}
