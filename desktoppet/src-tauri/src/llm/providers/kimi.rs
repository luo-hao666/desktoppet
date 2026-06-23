use async_trait::async_trait;
use serde_json::json;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

use crate::llm::{
    build_multimodal_user_content, stream_request, ChatMessage, ImageAttachment, LlmProvider,
};

const ENDPOINT: &str = "https://api.moonshot.cn/v1/chat/completions";

pub struct KimiProvider {
    api_key: String,
}

impl KimiProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[async_trait]
impl LlmProvider for KimiProvider {
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
        app_handle: AppHandle,
        is_thinking: Arc<AtomicBool>,
        is_talking: Arc<AtomicBool>,
    ) -> Result<String, String> {
        let body = json!({
            "model": model,
            "messages": messages.iter().map(|m| json!({
                "role": m.role,
                "content": m.content,
            })).collect::<Vec<_>>(),
            "stream": true,
        });
        stream_request(&self.api_key, ENDPOINT, body, app_handle, is_thinking, is_talking).await
    }

    async fn chat_stream_with_images(
        &self,
        messages: Vec<ChatMessage>,
        images: Vec<ImageAttachment>,
        model: &str,
        app_handle: AppHandle,
        is_thinking: Arc<AtomicBool>,
        is_talking: Arc<AtomicBool>,
    ) -> Result<String, String> {
        let mut api_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                json!({
                    "role": m.role,
                    "content": m.content,
                })
            })
            .collect();

        if let Some(last) = api_messages.last_mut() {
            let user_text = messages.last().map(|m| m.content.clone()).unwrap_or_default();
            let parts = build_multimodal_user_content(&user_text, &images);
            *last = json!({
                "role": "user",
                "content": parts,
            });
        }

        let body = json!({
            "model": model,
            "messages": api_messages,
            "stream": true,
        });
        stream_request(&self.api_key, ENDPOINT, body, app_handle, is_thinking, is_talking).await
    }
}
