use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::LazyLock;

pub const SECTIONS: &[&str] = &[
    "stats",
    "skills",
    "currencies",
    "slayers",
    "dungeons",
    "pets",
    "collections",
    "minions",
    "equipment",
    "accessories",
    "garden",
    "mining",
    "foraging",
    "hunting",
    "dailies",
    "quests",
    "rift",
    "crimson_isle",
    "chocolate_factory",
    "fishing",
    "bestiary",
    "travel",
    "profile_upgrades",
    "events",
    "orders",
    "museum",
    "api_fields",
    "storage",
    "data_contract",
    "systems",
];
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub transient: bool,
    pub retry_after_seconds: Option<u64>,
}
impl AppError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            transient: false,
            retry_after_seconds: None,
        }
    }
    pub fn retry(code: &str, message: &str, seconds: u64) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            transient: true,
            retry_after_seconds: Some(seconds),
        }
    }
    pub fn value(&self) -> Value {
        json!({"code":self.code,"message":self.message,"retryAfterSeconds":self.retry_after_seconds})
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for AppError {}
pub type Result<T> = std::result::Result<T, AppError>;
pub fn at<'a>(v: &'a Value, path: &str) -> &'a Value {
    path.split('.')
        .fold(v, |v, k| v.get(k).unwrap_or(&Value::Null))
}
pub fn has(v: &Value, path: &str) -> bool {
    let mut v = v;
    for k in path.split('.') {
        if let Some(next) = v.get(k) {
            v = next
        } else {
            return false;
        }
    }
    true
}
pub fn n(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}
pub trait JsonInteger {
    fn integer(&self) -> Option<i64>;
    fn unsigned(&self) -> Option<u64>;
}
impl JsonInteger for Value {
    fn integer(&self) -> Option<i64> {
        self.as_i64().or_else(|| {
            n(self)
                .filter(|n| n.fract() == 0. && *n >= i64::MIN as f64 && *n < i64::MAX as f64)
                .map(|n| n as i64)
        })
    }
    fn unsigned(&self) -> Option<u64> {
        self.as_u64().or_else(|| {
            n(self)
                .filter(|n| n.fract() == 0. && *n >= 0. && *n < u64::MAX as f64)
                .map(|n| n as u64)
        })
    }
}
pub fn nonnegative(v: &Value) -> Option<f64> {
    n(v).filter(|n| *n >= 0.)
}
pub fn obj(v: &Value) -> serde_json::Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}
pub fn arr(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}
static REGEX_CACHE: LazyLock<
    std::sync::RwLock<std::collections::HashMap<String, std::sync::Arc<Regex>>>,
> = LazyLock::new(|| std::sync::RwLock::new(std::collections::HashMap::new()));
pub fn re(pattern: &str, s: &str) -> bool {
    if let Some(r) = REGEX_CACHE.read().unwrap().get(pattern) {
        return r.is_match(s);
    }
    let Ok(r) = Regex::new(pattern) else {
        return false;
    };
    let matched = r.is_match(s);
    let mut cache = REGEX_CACHE.write().unwrap();
    if cache.len() < 256 {
        cache.insert(pattern.to_owned(), std::sync::Arc::new(r));
    }
    matched
}
static FORMAT: LazyLock<Regex> = LazyLock::new(|| Regex::new("(?i)§[0-9a-fk-or]").unwrap());
pub fn text(v: &Value, max: usize) -> Option<String> {
    v.as_str().map(|s| {
        FORMAT
            .replace_all(s, "")
            .chars()
            .filter(|c| !c.is_control())
            .take(max)
            .collect()
    })
}
pub fn numeric_map(v: &Value, limit: usize) -> Value {
    Value::Object(
        obj(v)
            .into_iter()
            .take(limit)
            .filter(|(k, v)| re(r"^[a-zA-Z0-9_:.-]{1,96}$", k) && n(v).is_some())
            .collect(),
    )
}
pub fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
pub fn iso(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_default()
}
pub fn parse_time(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.timestamp_millis())
}
pub fn section(data: Value, present: bool, warnings: Vec<String>) -> Value {
    json!({"status":if !present{"unavailable"}else if warnings.is_empty(){"available"}else{"partial"},"data":data,"warnings":warnings})
}
pub fn merge(mut base: Value, extra: Value) -> Value {
    if let (Some(a), Some(b)) = (base.as_object_mut(), extra.as_object()) {
        a.extend(b.clone());
    }
    base
}
pub fn uuid(s: &str) -> Option<String> {
    if !re(
        "(?i)^(?:[a-f0-9]{32}|[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12})$",
        s,
    ) {
        return None;
    }
    Some(s.replace('-', "").to_lowercase())
}
pub fn validate_player(s: &str) -> Result<()> {
    if uuid(s).is_some() || re("^[a-zA-Z0-9_]{1,16}$", s) {
        Ok(())
    } else {
        Err(AppError::new(
            "INVALID_PLAYER",
            "Provide a Minecraft username or UUID, not a URL.",
        ))
    }
}
