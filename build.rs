fn main() {
    // Windows：把图标嵌进 exe（资源管理器/任务栏显示）
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winres::WindowsResource::new()
            .set_icon("assets/app-icon.ico")
            .compile()
            .expect("embed icon");
    }
}
