//! Utilities for logs.

use once_cell::sync::Lazy;
use std::sync::RwLock;

static LOG_LEVEL: Lazy<RwLock<log::Level>> =
    Lazy::new(|| RwLock::new(log::Level::Trace));

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
