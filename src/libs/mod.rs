pub(crate) mod logs;

#[allow(unused)]
pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
#[allow(unused)]
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
#[allow(unused)]
pub const TIME_FORMAT: &str = "[year]-[month]-[day] [hour]:[minute]:[second]";
pub const TIME_MILLISECOND_FORMAT: &str =
    "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]";

/// app所在目录
pub fn app_dir() -> String {
    std::env::current_exe()
        .unwrap_or(std::path::PathBuf::new())
        .parent()
        .unwrap_or(std::path::Path::new(work_dir().as_str()))
        .to_str()
        .unwrap_or(work_dir().as_str())
        .to_string()
}

/// 获取工作目录
pub fn work_dir() -> String {
    std::env::current_dir()
        .unwrap_or(std::path::PathBuf::new())
        .to_str()
        .unwrap_or(".")
        .to_string()
}
