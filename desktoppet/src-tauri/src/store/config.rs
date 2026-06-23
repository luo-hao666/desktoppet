use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub provider: String,
    pub api_keys: HashMap<String, String>,
    pub model: String,
    pub pet_folder: String,
    pub pet_size: u32,
    pub pet_position: Option<Position>,
    pub auto_start: bool,
    pub pet_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kb_folder: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: "deepseek".to_string(),
            api_keys: HashMap::new(),
            model: "deepseek-chat".to_string(),
            pet_folder: String::new(),
            pet_size: 128,
            pet_position: None,
            auto_start: false,
            pet_name: "小咪".to_string(),
            kb_folder: None,
        }
    }
}

impl AppConfig {
    /// 取当前 provider 对应的 API Key
    pub fn current_api_key(&self) -> &str {
        self.api_keys
            .get(&self.provider)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// 获取配置文件路径
    pub fn config_path() -> PathBuf {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(appdata).join("desktoppet").join("config.json")
    }

    /// 从文件加载配置
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    serde_json::from_str(&content).unwrap_or_default()
                }
                Err(_) => Self::default(),
            }
        } else {
            Self::default()
        }
    }

    /// 保存配置到文件
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建配置目录失败: {}", e))?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("序列化配置失败: {}", e))?;
        std::fs::write(&path, content)
            .map_err(|e| format!("写入配置文件失败: {}", e))?;
        Ok(())
    }
}
