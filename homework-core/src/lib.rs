mod cache;
mod clients;
mod config;
mod fetch;
mod log;
mod models;

#[macro_export]
macro_rules! hw_log {
    ($($arg:tt)*) => {
        $crate::log::log_line(format!($($arg)*))
    };
}

pub use cache::{load_homework_cache, save_homework_cache};
pub use clients::YuketangClient;
pub use clients::yuketang::render_qr_png;
pub use config::{
    app_dir, config_path, load_config, resolve_config_path, save_config, AppConfig, PlatformConfig,
};
pub use fetch::{fetch_all_homework, save_yuketang_session, FetchResult};
pub use models::{
    homework_headline_counts, homework_stats_debug_report, pending_sorted_by_deadline,
    HomeworkItem, Urgency,
};
