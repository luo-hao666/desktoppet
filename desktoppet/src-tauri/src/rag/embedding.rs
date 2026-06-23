use std::sync::{Arc, Mutex};

use ndarray::{Array2, Axis};
use ort::session::Session;
use ort::value::TensorRef;
use tokenizers::Tokenizer;

#[derive(Clone)]
pub struct EmbeddingModel {
    session: Arc<Mutex<Session>>,
    tokenizer: Arc<Tokenizer>,
}

impl EmbeddingModel {
    pub fn load(model_path: &str, tokenizer_path: &str) -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|e| format!("SessionBuilder 失败: {}", e))?
            .commit_from_file(model_path)
            .map_err(|e| format!("加载 ONNX 模型失败: {}", e))?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| format!("加载 tokenizer 失败: {}", e))?;

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
            tokenizer: Arc::new(tokenizer),
        })
    }

    pub fn embed(&self, texts: &[String], text_type: &str) -> Result<Vec<Vec<f32>>, String> {
        let prefix = "为这个句子生成表示以用于检索相关文章：";
        let processed: Vec<String> = texts
            .iter()
            .map(|t| {
                if text_type == "query" {
                    format!("{}{}", prefix, t)
                } else {
                    t.clone()
                }
            })
            .collect();

        let encodings = self
            .tokenizer
            .encode_batch(processed, true)
            .map_err(|e| format!("tokenize 失败: {}", e))?;

        let batch_size = encodings.len();
        let max_len = encodings.iter().map(|e| e.len()).max().unwrap_or(1);

        let mut input_ids = Array2::<i64>::zeros((batch_size, max_len));
        let mut attention_mask = Array2::<i64>::zeros((batch_size, max_len));
        let token_type_ids = Array2::<i64>::zeros((batch_size, max_len));

        for (i, enc) in encodings.iter().enumerate() {
            let ids = enc.get_ids();
            let mask = enc.get_attention_mask();
            for (j, &id) in ids.iter().enumerate() {
                input_ids[[i, j]] = id as i64;
            }
            for (j, &m) in mask.iter().enumerate() {
                attention_mask[[i, j]] = m as i64;
            }
        }

        // Run ONNX inference
        let input_ids_tensor =
            TensorRef::from_array_view(input_ids.view())
                .map_err(|e| format!("创建 input_ids tensor 失败: {}", e))?;
        let mask_tensor =
            TensorRef::from_array_view(attention_mask.view())
                .map_err(|e| format!("创建 attention_mask tensor 失败: {}", e))?;
        let type_ids_tensor =
            TensorRef::from_array_view(token_type_ids.view())
                .map_err(|e| format!("创建 token_type_ids tensor 失败: {}", e))?;

        let mut session = self.session.lock().map_err(|e| format!("锁定 session 失败: {}", e))?;
        let outputs = session
            .run(ort::inputs![input_ids_tensor, mask_tensor, type_ids_tensor])
            .map_err(|e| format!("ONNX 推理失败: {}", e))?;

        // Extract last_hidden_state: returns (&Shape, &[f32]) with shape [batch, seq_len, hidden_dim]
        let (shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("提取 tensor 失败: {}", e))?;

        let shape_vec: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
        let hidden_dim = shape_vec[2];
        let flat =
            ndarray::ArrayView3::from_shape((shape_vec[0], shape_vec[1], hidden_dim), data)
                .map_err(|e| format!("重塑输出失败: {}", e))?;

        // Mean pooling with attention mask + L2 normalization
        let mask_f32 = attention_mask.mapv(|x| x as f32);
        let mask_expanded = mask_f32.view().insert_axis(Axis(2)); // [batch, seq_len, 1]

        let masked = &flat * &mask_expanded;
        let sum = masked.sum_axis(Axis(1)); // [batch, hidden_dim]
        let counts = mask_expanded.sum_axis(Axis(1)); // [batch, 1]
        let counts_clamped = counts.mapv(|c| if c < 1e-8 { 1.0 } else { c });
        let mean = &sum / &counts_clamped; // [batch, hidden_dim]

        // L2 normalization
        let norms = mean.map_axis(Axis(1), |row| (row.mapv(|x| x * x).sum()).sqrt());
        let norms_clamped = norms.mapv(|n| if n < 1e-8 { 1.0 } else { n });
        let normalized = &mean / &norms_clamped.insert_axis(Axis(1));

        // Convert to Vec<Vec<f32>>
        let mut result = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let row = normalized.index_axis(Axis(0), i);
            result.push(row.iter().copied().collect());
        }

        Ok(result)
    }
}
