use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fs;
use std::sync::RwLock;
use crate::functions::app_config::AppConfig;

lazy_static! {
    pub static ref TRANSLATIONS: RwLock<HashMap<String, String>> = RwLock::new(HashMap::new());
}

pub mod init_languages;
pub mod translate;

pub use init_languages::init_languages;
pub use translate::{translate, translate_desc, translate_tip};

pub fn load_translation(lang_name: &str) {
    let lang_dir = AppConfig::config_dir().join("lang");
    let file_path = lang_dir.join(format!("{}.yaml", lang_name));

    if let Ok(content) = fs::read_to_string(&file_path) {
        if let Ok(map) = serde_yaml::from_str::<HashMap<String, String>>(&content) {
            let mut trans = TRANSLATIONS.write().unwrap();
            *trans = map;
            return;
        }
    }

    let mut fallback = HashMap::new();
    if lang_name == "vn" {
        fallback.insert("unikey_tip".to_string(), "💡 Mẹo: Nếu Unikey tự chuyển dấu sai (ví dụ Telex), hãy tắt bộ gõ tiếng Việt (chuyển sang chữ E) trong hệ điều hành.".to_string());
        fallback.insert(
            "remote".to_string(),
            "Remote nguồn cần mã hóa (ví dụ: Telebox:ThuMucGoc)".to_string(),
        );
    } else {
        fallback.insert("unikey_tip".to_string(), "💡 Tip: If your IME interferes with typing, temporarily switch your OS layout to English mode.".to_string());
        fallback.insert(
            "remote".to_string(),
            "Remote to encrypt/decrypt. Normally should contain a ':' and a path".to_string(),
        );
    }
    let mut trans = TRANSLATIONS.write().unwrap();
    *trans = fallback;
}

pub fn get_available_languages() -> Vec<String> {
    let lang_dir = AppConfig::config_dir().join("lang");
    let mut langs = Vec::new();

    if let Ok(entries) = fs::read_dir(lang_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "yaml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    langs.push(stem.to_string());
                }
            }
        }
    }

    if langs.is_empty() {
        langs.push("vn".to_string());
        langs.push("eng".to_string());
    }

    langs.sort();
    langs
}
