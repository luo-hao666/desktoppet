pub mod chunking;
pub mod embedding;
pub mod store;

use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::file_handler::process_file;
use crate::rag::chunking::chunk_text;
use crate::rag::embedding::EmbeddingModel;
use crate::rag::store::{KbChunk, KbMetadata, KbStore};

use crate::AppState;

// ===== Event Payloads =====

#[derive(Debug, Clone, Serialize)]
pub struct KbIndexProgressEvent {
    pub current: usize,
    pub total: usize,
    pub current_file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct KbIndexDoneEvent {
    pub file_count: usize,
    pub chunk_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct KbStatus {
    pub indexed: bool,
    pub file_count: usize,
    pub chunk_count: usize,
    pub last_indexed: String,
    pub kb_folder: String,
    pub model_loaded: bool,
}

#[derive(Clone, Serialize)]
pub struct ChatDoneEvent {
    pub sources: Vec<String>,
}

// ===== Supported File Extensions =====

fn is_supported(ext: &str) -> bool {
    matches!(
        ext,
        "txt"
            | "md"
            | "pdf"
            | "docx"
            | "pptx"
            | "py"
            | "js"
            | "ts"
            | "jsx"
            | "tsx"
            | "json"
            | "html"
            | "css"
            | "rs"
            | "java"
            | "go"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "yaml"
            | "yml"
            | "toml"
            | "xml"
            | "sql"
            | "sh"
            | "bat"
            | "ps1"
            | "log"
            | "csv"
            | "vue"
            | "svelte"
    )
}

/// 递归扫描文件夹，收集所有支持类型的文件路径
fn scan_supported_files(folder: &str) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    let entries =
        std::fs::read_dir(folder).map_err(|e| format!("读取目录失败: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取条目失败: {}", e))?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(scan_supported_files(&path.to_string_lossy())?);
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if is_supported(&ext) {
                files.push(path);
            }
        }
    }
    Ok(files)
}

/// 异步构建知识库索引并推送进度，返回完整的 KbStore
pub async fn build_index(
    folder: &str,
    model: &EmbeddingModel,
    app_handle: AppHandle,
) -> Result<KbStore, String> {
    let file_paths = scan_supported_files(folder)?;
    let total = file_paths.len();

    // 规范化文件夹路径（去除尾部斜杠，确保 strip_prefix 正确）
    let folder_normalized = folder.trim_end_matches('\\').trim_end_matches('/');

    let mut all_chunks: Vec<KbChunk> = Vec::new();
    let mut indexed_file_count = 0usize;

    for (i, path) in file_paths.iter().enumerate() {
        let _ = app_handle.emit(
            "kb-index-progress",
            KbIndexProgressEvent {
                current: i + 1,
                total,
                current_file: path.to_string_lossy().to_string(),
            },
        );

        match process_file(&path.to_string_lossy()) {
            Ok(fc) if fc.file_type == "text" => {
                let source = path
                    .to_string_lossy()
                    .strip_prefix(folder_normalized)
                    .unwrap_or(&path.to_string_lossy())
                    .trim_start_matches('\\')
                    .trim_start_matches('/')
                    .to_string();
                let chunks = chunk_text(&fc.content, &source, 512, 50);
                if !chunks.is_empty() {
                    indexed_file_count += 1;
                }
                all_chunks.extend(chunks);
            }
            _ => continue,
        }
    }

    // 批量向量化（每批 32 条）
    for batch in all_chunks.chunks_mut(32) {
        let texts: Vec<String> = batch.iter().map(|c| c.text.clone()).collect();
        let embeddings = model.embed(&texts, "document")?;
        for (chunk, emb) in batch.iter_mut().zip(embeddings) {
            chunk.embedding = emb;
        }
    }

    let metadata = KbMetadata {
        file_count: indexed_file_count,
        chunk_count: all_chunks.len(),
        last_indexed: chrono::Local::now().to_rfc3339(),
        kb_folder: folder.to_string(),
    };
    let store = KbStore {
        metadata,
        chunks: all_chunks,
    };

    store.save_to_disk()?;

    let _ = app_handle.emit(
        "kb-index-done",
        KbIndexDoneEvent {
            file_count: store.metadata.file_count,
            chunk_count: store.metadata.chunk_count,
        },
    );

    Ok(store)
}

// ===== Tauri Commands =====

#[tauri::command]
pub async fn build_knowledge_base(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    folder: String,
) -> Result<(), String> {
    let app_state_arc = state.inner().clone();

    if app_state_arc.is_indexing.swap(true, Ordering::SeqCst) {
        return Err("索引正在进行中".to_string());
    }

    let model = {
        app_state_arc
            .embedding_model
            .lock()
            .unwrap()
            .clone()
            .ok_or("Embedding 模型未初始化".to_string())?
    };

    let kb_ref = Arc::clone(&app_state_arc.kb_store);
    let is_indexing_ref = Arc::clone(&app_state_arc.is_indexing);
    let app_clone = app.clone();
    let folder_clone = folder.clone();

    tauri::async_runtime::spawn(async move {
        match build_index(&folder_clone, &model, app_clone.clone()).await {
            Ok(store) => {
                *kb_ref.lock().unwrap() = Some(store);
            }
            Err(e) => {
                let _ = app_clone.emit(
                    "pet-chat-error",
                    crate::llm::ChatErrorEvent {
                        message: format!("索引构建失败: {}", e),
                    },
                );
            }
        }
        is_indexing_ref.store(false, Ordering::SeqCst);
    });

    Ok(())
}

#[tauri::command]
pub async fn get_kb_status(state: State<'_, Arc<AppState>>) -> Result<KbStatus, String> {
    let kb_guard = state.kb_store.lock().unwrap();
    let config = state.config.lock().unwrap();
    let model_loaded = state.embedding_model.lock().unwrap().is_some();

    match kb_guard.as_ref() {
        Some(store) if model_loaded => Ok(KbStatus {
            indexed: true,
            file_count: store.metadata.file_count,
            chunk_count: store.metadata.chunk_count,
            last_indexed: store.metadata.last_indexed.clone(),
            kb_folder: store.metadata.kb_folder.clone(),
            model_loaded: true,
        }),
        _ => Ok(KbStatus {
            indexed: false,
            file_count: 0,
            chunk_count: 0,
            last_indexed: String::new(),
            kb_folder: config.kb_folder.clone().unwrap_or_default(),
            model_loaded,
        }),
    }
}

#[tauri::command]
pub async fn open_index_dir() -> Result<(), String> {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir = std::path::PathBuf::from(appdata).join("desktoppet");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("打开目录失败: {}", e))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = dir;
        return Err("当前仅支持 Windows".to_string());
    }
    Ok(())
}

/// 清空当前知识库文件夹的索引（删除 kb_store.json + 清空内存）
#[tauri::command]
pub async fn clear_kb_index(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if state.is_indexing.load(Ordering::SeqCst) {
        return Err("索引正在进行中，请等待完成后再清空".to_string());
    }
    KbStore::delete_from_disk()?;
    *state.kb_store.lock().unwrap() = None;
    Ok(())
}

/// 清空所有索引文件（包括可能的残留文件）
#[tauri::command]
pub async fn clear_all_indexes(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if state.is_indexing.load(Ordering::SeqCst) {
        return Err("索引正在进行中，请等待完成后再清空".to_string());
    }
    KbStore::delete_all_from_disk()?;
    *state.kb_store.lock().unwrap() = None;
    Ok(())
}
