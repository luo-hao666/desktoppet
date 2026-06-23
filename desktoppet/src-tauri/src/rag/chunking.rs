use uuid::Uuid;

use super::store::KbChunk;

/// 将文本切分为若干 KbChunk，按双换行分段 + 滑动窗口
pub fn chunk_text(text: &str, source_file: &str, max_chars: usize, overlap: usize) -> Vec<KbChunk> {
    let text = text.replace("\r\n", "\n");

    let mut chunks = Vec::new();
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    for para in paragraphs {
        let chars: Vec<char> = para.chars().collect();
        if chars.len() <= max_chars {
            chunks.push(KbChunk {
                id: Uuid::new_v4().to_string(),
                text: para.to_string(),
                source_file: source_file.to_string(),
                embedding: Vec::new(),
            });
        } else {
            let step = max_chars - overlap;
            let mut start = 0usize;
            while start < chars.len() {
                let end = (start + max_chars).min(chars.len());
                let chunk_text: String = chars[start..end].iter().collect();
                chunks.push(KbChunk {
                    id: Uuid::new_v4().to_string(),
                    text: chunk_text,
                    source_file: source_file.to_string(),
                    embedding: Vec::new(),
                });
                if end >= chars.len() {
                    break;
                }
                start += step;
            }
        }
    }
    chunks
}
