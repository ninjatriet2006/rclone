use crate::functions::lang::TRANSLATIONS;

pub fn translate(key: &str) -> String {
    let trans = TRANSLATIONS.read().unwrap();
    if let Some(val) = trans.get(key) {
        let mut val_str = val.clone();
        if cfg!(target_os = "macos") {
            val_str = val_str.replace("Alt+", "Ctrl+");
        }
        return val_str;
    }
    key.to_string()
}

pub fn translate_desc(field_name: &str, english_desc: &str) -> String {
    let trans = TRANSLATIONS.read().unwrap();
    if let Some(val) = trans.get(field_name) {
        return val.clone();
    }

    let mut translated = english_desc.to_string();
    if translated.contains("OAuth Client ID") {
        translated = translated.replace("OAuth Client ID", "OAuth Client ID của ứng dụng");
    }
    if translated.contains("OAuth Client Secret") {
        translated = translated.replace("OAuth Client Secret", "OAuth Client Secret của ứng dụng");
    }
    if translated.contains("Password") {
        translated = translated.replace("Password", "Mật khẩu");
    }
    translated
}

pub fn translate_tip(tip_key: &str) -> String {
    translate(tip_key)
}
