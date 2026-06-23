//! 多轮对话上下文管理

use super::ChatMessage;

pub struct ConversationContext {
    messages: Vec<ChatMessage>,
    /// 最大保留轮数（一轮 = user + assistant，共 2 条）
    max_turns: usize,
}

impl ConversationContext {
    pub fn new(max_turns: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_turns,
        }
    }

    pub fn push_user(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content,
        });
        self.trim();
    }

    pub fn push_assistant(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content,
        });
        self.trim();
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    fn trim(&mut self) {
        let max_messages = self.max_turns * 2;
        if self.messages.len() > max_messages {
            let drain_count = self.messages.len() - max_messages;
            self.messages.drain(0..drain_count);
        }
    }
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new(10) // 默认保留 10 轮
    }
}
