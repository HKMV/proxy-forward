pub(crate) mod logs;

pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
pub const TIME_MILLISECOND_FORMAT: &str =
    "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]";

/// app所在目录
pub fn app_dir() -> String {
    std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(std::path::Path::new(work_dir().as_str()))
        .to_str()
        .unwrap_or(work_dir().as_str())
        .to_string()
}

/// 获取工作目录
pub fn work_dir() -> String {
    std::env::current_dir()
        .unwrap_or_default()
        .to_str()
        .unwrap_or(".")
        .to_string()
}
