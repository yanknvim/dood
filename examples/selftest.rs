//! 固定シーンを描いて PNG に落とす確認用の例。
//!
//! `cargo run --example selftest` で `target/selftest.png` を生成する。
//! ウィンドウは開かず、egui_kittest のヘッドレス描画（Metal）を使う。

use dood::{
    render,
    scene::{Item, Scene},
};
use eframe::egui::{pos2, Color32, Vec2};

fn main() {
    let mut scene = Scene::default();
    let ink = Color32::from_rgb(0x1a, 0x1a, 0x1a);

    scene.push(Item::Stroke {
        points: vec![
            pos2(20.0, 30.0),
            pos2(70.0, 10.0),
            pos2(120.0, 60.0),
            pos2(170.0, 20.0),
        ],
        color: Color32::from_rgb(0xd6, 0x27, 0x28),
        width: 4.0,
    });
    scene.push(Item::Rect {
        a: pos2(30.0, 90.0),
        b: pos2(150.0, 160.0),
        color: ink,
        width: 3.0,
    });
    scene.push(Item::Ellipse {
        center: pos2(240.0, 125.0),
        radius: Vec2::new(70.0, 40.0),
        color: Color32::from_rgb(0x1c, 0x5c, 0xd6),
        width: 3.0,
    });
    scene.push(Item::Arrow {
        a: pos2(180.0, 180.0),
        b: pos2(320.0, 210.0),
        color: Color32::from_rgb(0x1f, 0x8a, 0x3d),
        width: 3.0,
    });
    scene.push(Item::Text {
        pos: pos2(30.0, 200.0),
        text: "Label ラベル".to_owned(),
        size: 24.0,
        color: ink,
    });

    let mut harness = egui_kittest::Harness::builder()
        .with_size(Vec2::new(400.0, 300.0))
        .with_pixels_per_point(2.0)
        .build_ui(|ui| {
            static PROVIDER: std::sync::Once = std::sync::Once::new();
            PROVIDER.call_once(|| {
                ui.ctx()
                    .set_missing_glyph_policy(eframe::egui::MissingGlyphPolicy::Tofu);
                ui.ctx()
                    .add_font_provider(std::sync::Arc::new(
                        egui_system_font_provider::SystemFontProvider::new()
                            .with_locale(Some("ja")),
                    ));
            });
            let rect = eframe::egui::Rect::from_min_size(
                eframe::egui::Pos2::ZERO,
                Vec2::new(400.0, 300.0),
            );
            render::render_scene(ui.painter(), rect, &scene);
        });

    harness.run();
    let image = harness.render().expect("headless render");
    std::fs::create_dir_all("target").ok();
    image.save("target/selftest.png").expect("save png");
    println!(
        "wrote target/selftest.png ({}×{})",
        image.width(),
        image.height()
    );
}
