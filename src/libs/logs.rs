use std::fs;
use tracing::level_filters::LevelFilter;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::libs::{APP_NAME, TIME_MILLISECOND_FORMAT};

/// 初始化日志
#[allow(unused)]
pub fn init(log_file: String, level: LevelFilter) -> anyhow::Result<()> {
    let current_dir = crate::libs::app_dir();
    let logs_dir = current_dir + "/logs/";
    fs::create_dir_all(logs_dir.clone())?;
    // let log_file_path = logs_dir.clone().to_string() + log_file;

    hook_panic_handler(logs_dir.clone(), log_file.clone());
    init_tracing(logs_dir, log_file, level);
    Ok(())
}

#[allow(unused)]
pub fn init_default() -> anyhow::Result<()> {
    init(APP_NAME.to_owned(), LevelFilter::DEBUG)
}

#[allow(unused)]
pub fn init_debug() -> anyhow::Result<()> {
    init_tracing("".to_string(), "".to_string(), LevelFilter::DEBUG);
    Ok(())
}

/// 拦截panic处理，保存panic信息到panic日志中
///
/// # Arguments
///
/// * `logs_dir`: 日志保存位置
/// * `app_name`: 应用名称
///
/// returns: ()
///
/// # Examples
///
/// ```
/// logs::setup_panic_handler(String::from("./logs/"), String::from("nal"));
/// ```
fn hook_panic_handler(logs_dir: String, app_name: String) {
    use std::backtrace;
    use std::fs::OpenOptions;
    use std::io::Write;
    use time::macros::offset;
    use time::OffsetDateTime;

    std::panic::set_hook(Box::new(move |info| {
        let backtrace = backtrace::Backtrace::force_capture();
        let payload = info.payload();
        let payload_str: Option<&str> = if let Some(s) = payload.downcast_ref::<&str>() {
            Some(s)
        } else if let Some(s) = payload.downcast_ref::<String>() {
            Some(s)
        } else {
            None
        };

        if let Some(payload_str) = payload_str {
            println!(
                "panic occurred: payload:{}, location: {:?}",
                payload_str,
                info.location()
            );
        } else {
            println!("panic occurred: location: {:?}", info.location());
        }

        let format = time::format_description::parse(TIME_MILLISECOND_FORMAT).unwrap();
        let current_time = OffsetDateTime::now_utc()
            .to_offset(offset!(+8))
            .format(&format)
            .unwrap_or_else(|e| {
                println!("get current time error: {:?}", e);
                "".to_string()
            });

        let _ = OpenOptions::new()
            .write(true)
            .append(true)
            .create(true) // 如果文件不存在，则创建文件
            .open(format!("{}{}.panic.log", logs_dir, app_name))
            .and_then(|mut f| {
                f.write_all(format!("{} {:?}\n{:#?}\n", current_time, info, backtrace).as_bytes())
            });
        println!("{}", "panic backtrace saved");
        std::process::exit(1);
    }));
}

fn init_tracing(logs_dir: String, log_file: String, level: LevelFilter) {
    let format = time::format_description::parse(TIME_MILLISECOND_FORMAT).unwrap();

    let tracing_level = level.into_level().unwrap();

    let builder = tracing_subscriber::fmt()
        .with_level(true)
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_thread_ids(true)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_test_writer()
        .with_max_level(tracing_level)
        .with_timer(tracing_subscriber::fmt::time::OffsetTime::new(
            time::macros::offset!(+8),
            format,
        ))
        .with_ansi(false);
    if cfg!(debug_assertions) {
        builder
            //调试模式输出到控制台
            .with_writer(
                //将 ERROR 及以上级别的日志输出到 stderr, 其他级别日志则输出到 stdout
                std::io::stdout
                    .with_filter(|meta| meta.level() > &Level::ERROR)
                    .or_else(std::io::stderr),
            )
            .finish()
            .init();
    } else {
        builder
            //非调试模式输出到日志文件
            .with_writer(
                tracing_appender::rolling::Builder::new()
                    .filename_prefix(log_file.clone())
                    .filename_suffix("log")
                    .max_log_files(7)
                    .rotation(tracing_appender::rolling::Rotation::DAILY)
                    .build(logs_dir.clone())
                    .unwrap(),
            )
            .finish()
            .init();
    }
}
