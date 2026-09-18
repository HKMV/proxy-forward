//! UI 入口：组装 view + controller，跑事件循环。

pub mod controller;
pub mod view;

use crate::core::config::AppConfig;
use crate::core::stats::Stats;
use fltk::app;
use fltk::enums::FrameType;
use fltk::prelude::*;
use fltk_theme::{ThemeType, WidgetTheme};
use std::collections::VecDeque;

pub fn run() -> anyhow::Result<()> {
    let app = app::App::default();
    // 重写框型绘制：RFlatBox=6px 圆角；DownBox=2px 微圆角平底（复选框走这条路画 ✓）
    app::set_frame_type_cb(FrameType::RFlatBox, view::draw_flat_round, 0, 0, 0, 0);
    app::set_frame_type_cb(FrameType::DownBox, view::draw_flat_round_small, 0, 0, 0, 0);
    let (log_tx, log_rx) = app::channel::<String>();
    crate::libs::logs::init_default_with_ui(move |line| log_tx.send(line))?;

    // 主题：启动时应用（fltk 全局配色，运行中切换不可靠，重启生效）
    let config = AppConfig::init().unwrap_or_default();
    let dark = match config.theme.as_str() {
        "dark" => true,
        "light" => false,
        _ => matches!(dark_light::detect(), Ok(dark_light::Mode::Dark)),
    };
    // 基础主题修系统件（滚动条等），上层配色全部手动覆盖
    WidgetTheme::new(if dark { ThemeType::Dark } else { ThemeType::Greybird }).apply();
    app::set_font_size(14);
    // 注意：不要全局 set_visible_focus(false)，否则输入框光标不闪；
    // 焦点框在各按钮上单独关闭
    let pal = view::palette(dark);
    view::set_current_pal(pal);
    view::set_theme_sel(view::theme_index(&config));

    let stats = Stats::shared();
    let model = controller::new_model();
    let daemon_slot = controller::new_daemon_slot();

    let mut v = view::build(&config, pal);

    // 初始规则行
    for rule in config.rules.clone() {
        controller::add_rule(&v.rows_pack, &model, rule);
    }
    controller::resize_pack(&v.rows_pack, &model);
    view::refresh_theme_btns(&mut v.theme_btns, view::theme_index(&config), pal);

    controller::wire(&mut v, &model, &daemon_slot, &stats);
    controller::start_stats_timer(stats.clone(), v.stats.clone());
    controller::start_cursor_blink(model.clone(), v.listen_input.clone());

    v.win.show();
    view::platform_window_fixups(&v.win);

    // 隐藏自测：PROXY_FORWARD_UI_TEST=1 时模拟删除第一行 + 点主题按钮，验证后退出
    #[cfg(debug_assertions)]
    if std::env::var("PROXY_FORWARD_UI_TEST").is_ok() {
        let model = model.clone();
        let pack = v.rows_pack.clone();
        let mut btns = v.theme_btns.clone();
        app::add_timeout3(0.5, move |_| {
            tracing::info!(
                "UITEST: rows = {}, pack children = {}",
                model.borrow().len(),
                pack.children()
            );
            // 模拟删除回调的完整逻辑
            let entry = {
                let mut m = model.borrow_mut();
                if m.is_empty() { None } else { Some(m.remove(0)) }
            };
            if let Some(entry) = entry {
                app::delete_widget(entry.widget);
                {
                    let m = model.borrow();
                    for (idx, e) in m.iter().enumerate() {
                        let mut w = e.widget.clone();
                        w.set_pos(0, idx as i32 * (view::ROW_H + view::ROW_GAP));
                    }
                }
                controller::resize_pack(&pack, &model);
            }
            // 程序化点击主题按钮 1，打印颜色验证选中态
            btns[1].do_callback();
            let model2 = model.clone();
            let pack2 = pack.clone();
            let btns2 = btns.clone();
            app::add_timeout3(1.0, move |_| {
                tracing::info!(
                    "UITEST after delete: rows = {}, pack children = {}",
                    model2.borrow().len(),
                    pack2.children()
                );
                for (i, b) in btns2.iter().enumerate() {
                    tracing::info!("UITEST theme btn[{i}] color = {:?}", b.color());
                }
                app::add_timeout3(0.5, |_| app::quit());
            });
        });
    }

    // 主循环：驱动 UI + 收日志
    let mut log_lines: VecDeque<String> = VecDeque::new();
    while app.wait() {
        let mut dirty = false;
        while let Some(line) = log_rx.recv() {
            if log_lines.len() >= 1000 {
                log_lines.pop_front();
            }
            log_lines.push_back(line);
            dirty = true;
        }
        if dirty {
            // ponytail: 全量 join 重建文本，1000 行上限下开销可忽略
            let text = log_lines
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let mut buf = v.log_buf.clone();
            buf.set_text(&text);
            v.log_disp.scroll(buf.count_lines(0, buf.length()), 0);
        }
    }
    Ok(())
}
