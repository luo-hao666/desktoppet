use ndarray::ArrayView1;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 单个索引块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbChunk {
    pub id: String,
    pub text: String,
    pub source_file: String,
    pub embedding: Vec<f32>,
}

/// 知识库元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbMetadata {
    pub file_count: usize,
    pub chunk_count: usize,
    pub last_indexed: String,
    pub kb_folder: String,
}

/// 完整的知识库存储文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbStore {
    pub metadata: KbMetadata,
    pub chunks: Vec<KbChunk>,
}

impl KbStore {
    /// kb_store.json 完整路径
    fn store_path() -> PathBuf {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(appdata).join("desktoppet").join("kb_store.json")
    }

    /// 序列化并写入磁盘
    pub fn save_to_disk(&self) -> Result<(), String> {
        let path = Self::store_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }
        let json = serde_json::to_string(self).map_err(|e| format!("序列化失败: {}", e))?;
        std::fs::write(&path, json).map_err(|e| format!("写入文件失败: {}", e))?;
        Ok(())
    }

    /// 从磁盘加载并反序列化
    pub fn load_from_disk() -> Result<KbStore, String> {
        let path = Self::store_path();
        let json =
            std::fs::read_to_string(&path).map_err(|e| format!("读取索引文件失败: {}", e))?;
        serde_json::from_str(&json).map_err(|e| format!("解析索引文件失败: {}", e))
    }

    /// 删除磁盘上的 kb_store.json
    pub fn delete_from_disk() -> Result<(), String> {
        let path = Self::store_path();
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("删除索引文件失败: {}", e))?;
        }
        Ok(())
    }

    /// 删除 %APPDATA%/desktoppet/ 下所有 kb_store 相关文件
    pub fn delete_all_from_disk() -> Result<(), String> {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(appdata).join("desktoppet");
        if dir.exists() {
            // 删除 kb_store.json
            let kb_path = dir.join("kb_store.json");
            if kb_path.exists() {
                std::fs::remove_file(&kb_path)
                    .map_err(|e| format!("删除索引文件失败: {}", e))?;
            }
            // 如果有其他索引相关文件也一并清理
            for entry in std::fs::read_dir(&dir).map_err(|e| format!("读取目录失败: {}", e))? {
                let entry = entry.map_err(|e| format!("读取条目失败: {}", e))?;
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("kb_") && name_str.ends_with(".json") {
                    std::fs::remove_file(entry.path())
                        .map_err(|e| format!("删除 {} 失败: {}", name_str, e))?;
                }
            }
        }
        Ok(())
    }
}

/// 余弦相似度检索 — 返回 top_k 个最相似的 chunk 及相似度分数
pub fn search(chunks: &[KbChunk], query_vec: &[f32], top_k: usize) -> Vec<(KbChunk, f32)> {
    let query = ArrayView1::from(query_vec);
    let query_norm = (query.dot(&query)).sqrt();

    let mut scored: Vec<(usize, f32)> = chunks
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.embedding.is_empty())
        .map(|(i, c)| {
            let emb = ArrayView1::from(&c.embedding);
            let emb_norm = (emb.dot(&emb)).sqrt();
            let cosine = if query_norm > 0.0 && emb_norm > 0.0 {
                query.dot(&emb) / (query_norm * emb_norm)
            } else {
                0.0
            };
            (i, cosine)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scored
        .into_iter()
        .take(top_k)
        .map(|(i, s)| (chunks[i].clone(), s))
        .collect()
}
