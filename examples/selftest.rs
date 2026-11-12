//! 固定シーンと Material Design 3 ツールバー UI を描いて PNG に落とす確認用の例。
//!
//! `cargo run --example selftest` で次の2枚を生成する:
//! - `target/selftest.png`        … 1100×800pt（既定のウィンドウサイズ。ツールバーは1段）
//! - `target/selftest-narrow.png` … 480×360pt（最小サイズ。ツールバーが折り返される）
//!
//! ウィンドウは開かず、egui_kittest のヘッドレス描画（Metal）を使う。

use std::path::PathBuf;

use dood::{app::DoodApp, scene::Item};
use eframe::egui::{Color32, Vec2, pos2};

fn main() {
    render(Vec2::new(1100.0, 800.0), "target/selftest.png");
    render(Vec2::new(480.0, 360.0), "target/selftest-narrow.png");
}

fn render(size: Vec2, path: &str) {
    let mut app_opt: Option<DoodApp> = None;

    let mut harness = egui_kittest::Harness::builder()
        .with_size(size)
        .with_pixels_per_point(2.0)
        .build_ui(|ui| {
            let app = app_opt.get_or_insert_with(|| make_app(ui.ctx()));
            app.render_ui(ui);
        });

    harness.run();
    let image = harness.render().expect("headless render");
    std::fs::create_dir_all("target").ok();
    image.save(path).expect("save png");
    println!("wrote {path} ({}×{})", image.width(), image.height());
}

fn make_app(ctx: &eframe::egui::Context) -> DoodApp {
    let ink = Color32::from_rgb(0x1a, 0x1a, 0x1a);
    let red = Color32::from_rgb(0xd6, 0x27, 0x28);

    let mut app = DoodApp::with_ctx(ctx, PathBuf::from("dood.png"));
    app.push_item(Item::Stroke {
        points: vec![
            pos2(80.0, 120.0),
            pos2(140.0, 105.0),
            pos2(190.0, 115.0),
            pos2(220.0, 165.0),
            pos2(205.0, 210.0),
            pos2(180.0, 195.0),
            pos2(185.0, 130.0),
            pos2(235.0, 70.0),
            pos2(290.0, 65.0),
            pos2(330.0, 110.0),
            pos2(340.0, 175.0),
            pos2(325.0, 185.0),
            pos2(325.0, 145.0),
            pos2(365.0, 125.0),
            pos2(400.0, 145.0),
            pos2(410.0, 180.0),
        ],
        color: ink,
        width: 4.0,
    });
    app.push_item(Item::Line {
        a: pos2(150.0, 345.0),
        b: pos2(345.0, 345.0),
        color: ink,
        width: 4.0,
    });
    app.push_item(Item::Rect {
        a: pos2(320.0, 255.0),
        b: pos2(980.0, 400.0),
        color: red,
        width: 4.0,
    });
    app.push_item(Item::Text {
        pos: pos2(345.0, 275.0),
        text: "dood".to_owned(),
        size: 24.0,
        color: ink,
    });
    app.push_item(Item::Arrow {
        a: pos2(455.0, 285.0),
        b: pos2(600.0, 285.0),
        color: ink,
        width: 4.0,
    });
    app.push_item(Item::Text {
        pos: pos2(645.0, 275.0),
        text: "雑な図が書けるツール".to_owned(),
        size: 24.0,
        color: ink,
    });
    // 複数行テキスト（Shift+Enter で改行）の見た目確認用。
    app.push_item(Item::Text {
        pos: pos2(345.0, 420.0),
        text: "Shift+Enter で改行できる。\n日本語 English 混在もこの通り。".to_owned(),
        size: 24.0,
        color: ink,
    });
    app
}
