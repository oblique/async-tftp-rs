use lazy_static::lazy_static;
use std::sync::RwLock;

lazy_static! {
    static ref LOG_LEVEL: RwLock<log::Level> = RwLock::new(log::Level::Trace);
}

macro_rules! log {
    ($($arg:tt)*) => {{
        log::log!($crate::log::get_log_level(), $($arg)*);
    }}
}

/// Get log level of this crate.
pub fn get_log_level() -> log::Level {
    *LOG_LEVEL.read().unwrap()
}

/// Set log level of this crate.
///
/// **Default:** `log::Level::Trace`
pub fn set_log_level(level: log::Level) {
    *LOG_LEVEL.write().unwrap() = level;
}
