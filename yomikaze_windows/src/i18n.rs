// translations. the english text is the key, anything missing falls back to english.
// the same json files are used by the ios app.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

// (setting value, name shown in the picker). "system" follows the windows language.
pub const LANGUAGES: [(&str, &str); 7] = [
    ("system", "System"),
    ("en", "English"),
    ("hu", "Magyar"),
    ("es", "Español"),
    ("ja", "日本語"),
    ("zh", "中文"),
    ("pirate", "Pirate"),
];

const CODES: [&str; 5] = ["hu", "es", "ja", "zh", "pirate"];

// 0 is english, 1.. index into CODES
static CURRENT: AtomicUsize = AtomicUsize::new(0);

fn tables() -> &'static Vec<HashMap<String, String>> {
    static TABLES: OnceLock<Vec<HashMap<String, String>>> = OnceLock::new();
    TABLES.get_or_init(|| {
        [
            include_str!("../lang/hu.json"),
            include_str!("../lang/es.json"),
            include_str!("../lang/ja.json"),
            include_str!("../lang/zh.json"),
            include_str!("../lang/pirate.json"),
        ]
        .iter()
        .map(|json| serde_json::from_str(json).unwrap_or_default())
        .collect()
    })
}

pub fn set_language(setting: &str) {
    let code = if setting == "system" { system_language() } else { setting.to_string() };
    let index = CODES.iter().position(|c| *c == code).map(|i| i + 1).unwrap_or(0);
    CURRENT.store(index, Ordering::Relaxed);
}

pub fn is_chinese() -> bool {
    CURRENT.load(Ordering::Relaxed) == CODES.iter().position(|c| *c == "zh").map_or(usize::MAX, |i| i + 1)
}

fn system_language() -> String {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Globalization::GetUserDefaultLocaleName;
        let mut buffer = [0u16; 85];
        let len = GetUserDefaultLocaleName(&mut buffer);
        if len > 1 {
            let name = String::from_utf16_lossy(&buffer[..(len as usize - 1)]);
            return name.split('-').next().unwrap_or("en").to_lowercase();
        }
    }
    "en".to_string()
}

pub fn t(text: &str) -> &str {
    match CURRENT.load(Ordering::Relaxed) {
        0 => text,
        i => tables()[i - 1].get(text).map(|s| s.as_str()).unwrap_or(text),
    }
}

// fills {name} placeholders after translating
pub fn tf(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = t(template).to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), &value.to_string());
    }
    out
}
