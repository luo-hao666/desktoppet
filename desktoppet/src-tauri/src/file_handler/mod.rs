//! 文件处理：根据扩展名分发到 text/pdf/office/image

pub mod image;
pub mod office;
pub mod pdf;
pub mod text;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct FileContent {
    pub file_type: String, // "text" | "image"
    pub content: String,   // 文本内容 或 base64
    pub mime: Option<String>,
    pub filename: String,
}

/// 文件处理入口
pub fn process_file(path: &str) -> Result<FileContent, String> {
    let p = std::path::Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let filename = p
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("unknown")
        .to_string();

    match ext.as_str() {
        // 纯文本
        "txt" | "md" | "py" | "js" | "ts" | "jsx" | "tsx" | "json" | "html" | "css" | "rs"
        | "java" | "go" | "c" | "cpp" | "h" | "hpp" | "yaml" | "yml" | "toml" | "xml"
        | "sql" | "sh" | "bat" | "ps1" | "log" | "csv" | "vue" | "svelte" => {
            let content = text::read_text(path)?;
            Ok(FileContent {
                file_type: "text".to_string(),
                content,
                mime: None,
                filename,
            })
        }

        "pdf" => {
            let content = pdf::extract_pdf_text(path)?;
            Ok(FileContent {
                file_type: "text".to_string(),
                content,
                mime: None,
                filename,
            })
        }

        "docx" | "pptx" => {
            let content = office::extract_office_text(path, &ext)?;
            Ok(FileContent {
                file_type: "text".to_string(),
                content,
                mime: None,
                filename,
            })
        }

        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => {
            let (b64, mime) = image::encode_image(path)?;
            Ok(FileContent {
                file_type: "image".to_string(),
                content: b64,
                mime: Some(mime),
                filename,
            })
        }

        _ => Err(format!("不支持的文件类型: .{}", ext)),
    }
}
