//! 通知の表示位置と、書き出し PNG に写り込まないことを検証する。
//!
//! `egui_kittest` のヘッドレス描画（Metal）を使う。通知は背景の貼り付けで出す。

use std::{path::PathBuf, sync::Arc};

use dood::{app::DoodApp, theme};
use eframe::egui::{Color32, ColorImage, Event, Key, Modifiers, Vec2};

type AppState = Option<DoodApp>;

// ツールバーが1段に収まる幅にする。
const WINDOW: Vec2 = Vec2::new(1100.0, 800.0);

fn temp_out(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dood-notification-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.join(name)
}

fn build_harness(out: PathBuf) -> egui_kittest::Harness<'static, AppState> {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(WINDOW)
        .with_pixels_per_point(1.0)
        .with_accessibility_check(false)
        .build_ui_state(
            move |ui, state: &mut AppState| {
                let app = state.get_or_insert_with(|| DoodApp::with_ctx(ui.ctx(), out.clone()));
                app.render_ui(ui);
            },
            None,
        );
    harness.run();
    harness
}

/// キャンバスの上端（ツールバーの下端）。左端に枠線が出る最初の行。
fn canvas_top(frame: &image::RgbaImage) -> usize {
    let [r, g, b, _] = theme::OUTLINE_VARIANT.to_srgba_unmultiplied();
    (0..frame.height() as usize)
        .find(|&y| {
            let pixel = frame.get_pixel(10, y as u32);
            pixel.0[0].abs_diff(r) <= 2
                && pixel.0[1].abs_diff(g) <= 2
                && pixel.0[2].abs_diff(b) <= 2
        })
        .expect("キャンバスの上端")
}

fn is_notification_color(pixel: &image::Rgba<u8>) -> bool {
    let [r, g, b, _] = theme::INVERSE_SURFACE.to_srgba_unmultiplied();
    pixel.0[0].abs_diff(r) <= 2 && pixel.0[1].abs_diff(g) <= 2 && pixel.0[2].abs_diff(b) <= 2
}

#[test]
fn notification_sits_below_the_toolbar_and_is_not_exported() {
    let out = temp_out("export.png");
    let _ = std::fs::remove_file(&out);
    let mut harness = build_harness(out.clone());

    // ⌘V で背景を貼ると通知が出る（画像はウィンドウと同じ大きさでリサイズを起こさない）。
    let size = [WINDOW.x as usize, WINDOW.y as usize];
    let image = ColorImage::new(size, vec![Color32::RED; size[0] * size[1]]);
    harness.event(Event::PasteImage(Arc::new(image)));
    harness.run();
    harness.run();

    let frame = harness.render().expect("headless render");
    // ツールバーの文字も近い色になるため、幅のある帯（通知の塗り）で判定する。
    let mut row_counts = vec![0usize; frame.height() as usize];
    for (_, y, pixel) in frame.enumerate_pixels() {
        if is_notification_color(pixel) {
            row_counts[y as usize] += 1;
        }
    }
    let solid_rows: Vec<usize> = row_counts
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 50)
        .map(|(y, _)| y)
        .collect();
    assert!(!solid_rows.is_empty(), "通知が描かれていない");
    let top = *solid_rows.first().expect("通知の先頭行");
    let bottom = *solid_rows.last().expect("通知の最終行");
    let canvas_top = canvas_top(&frame);
    assert!(
        top >= canvas_top,
        "通知がツールバーに重なっている (top={top}, canvas_top={canvas_top})"
    );
    let left = frame
        .enumerate_pixels()
        .filter(|(_, y, pixel)| {
            (top..=bottom).contains(&(*y as usize)) && is_notification_color(pixel)
        })
        .map(|(x, _, _)| x as usize)
        .min()
        .expect("通知の画素がある");
    assert!(
        left > WINDOW.x as usize / 2,
        "通知が右上にない (left={left})"
    );

    // 通知が出ている間に ⌘S。通知を描かないフレームを挟んで撮影される。
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    harness.run();

    let saved = image::open(&out)
        .expect("保存された PNG を開ける")
        .to_rgba8();
    assert!(
        !saved.pixels().any(is_notification_color),
        "通知が書き出し PNG に写り込んでいる"
    );
    assert!(
        saved
            .pixels()
            .any(|pixel| pixel.0[0] > 200 && pixel.0[1] < 80 && pixel.0[2] < 80),
        "背景が書き出されていない"
    );

    let _ = std::fs::remove_file(&out);
}
