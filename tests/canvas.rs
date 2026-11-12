//! 実際に描画した画素を検証する統合テスト。
//!
//! `egui_kittest` はウィンドウを開かずに Metal で描画する（GPU アダプタが必要）。
//! キャンバス矩形はテスト側で明示する（`ui.max_rect()` は余白を含むため）。

use dood::{
    render,
    scene::{Item, Scene},
};
use eframe::egui::{Color32, Pos2, Rect, Vec2, pos2};

const SIZE: Vec2 = Vec2::new(200.0, 100.0);

fn render_scene(scene: &Scene) -> image::RgbaImage {
    let canvas = Rect::from_min_size(Pos2::ZERO, SIZE);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_ui(|ui| {
            render::render_scene(ui.painter(), canvas, scene);
        });
    harness.run();
    harness.render().expect("headless render")
}

fn luma(pixel: &image::Rgba<u8>) -> u32 {
    let [r, g, b, _] = pixel.0;
    (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000
}

#[test]
fn empty_canvas_is_white() {
    let image = render_scene(&Scene::default());
    assert_eq!(image.dimensions(), (200, 100));
    for (index, pixel) in image.pixels().enumerate() {
        assert_eq!(pixel.0[3], 255, "{index} 番目が不透明でない: {:?}", pixel.0);
        assert!(luma(pixel) > 250, "{index} 番目が白くない: {:?}", pixel.0);
    }
}

#[test]
fn drawn_line_leaves_ink_on_canvas() {
    let mut scene = Scene::default();
    scene.push(Item::Line {
        a: pos2(20.0, 40.0),
        b: pos2(180.0, 40.0),
        color: Color32::BLACK,
        width: 6.0,
    });

    let image = render_scene(&scene);
    let darkest = (10..190)
        .flat_map(|x| (36..45).map(move |y| (x, y)))
        .map(|(x, y)| luma(image.get_pixel(x, y)))
        .min()
        .expect("範囲は空でない");
    assert!(darkest < 80, "線が描かれていない (darkest={darkest})");

    // 線から離れた場所は白いまま
    assert!(luma(image.get_pixel(20, 80)) > 250);
}

#[test]
fn shapes_are_visible_in_their_given_colors() {
    let mut scene = Scene::default();
    scene.push(Item::Rect {
        a: pos2(10.0, 10.0),
        b: pos2(60.0, 50.0),
        color: Color32::from_rgb(0xd6, 0x27, 0x28),
        width: 4.0,
    });
    scene.push(Item::Ellipse {
        center: pos2(140.0, 50.0),
        radius: Vec2::new(40.0, 25.0),
        color: Color32::from_rgb(0x1c, 0x5c, 0xd6),
        width: 4.0,
    });

    let image = render_scene(&scene);
    let has_red = image
        .pixels()
        .any(|p| p.0[0] > 150 && p.0[1] < 110 && p.0[2] < 110);
    let has_blue = image
        .pixels()
        .any(|p| p.0[2] > 150 && p.0[0] < 110 && p.0[1] < 140);
    assert!(has_red, "矩形の赤が見えない");
    assert!(has_blue, "楕円の青が見えない");
}

#[test]
fn text_stays_inside_its_wrap_width() {
    let mut scene = Scene::default();
    scene.push(Item::Text {
        pos: pos2(10.0, 10.0),
        text: "wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww".to_owned(),
        size: 24.0,
        color: Color32::BLACK,
        wrap_width: Some(80.0),
    });

    let image = render_scene(&scene);
    // 箱（左 10pt + 幅 80pt）より右にはインクが無い。
    for y in 0..100 {
        for x in 100..200 {
            assert!(
                luma(image.get_pixel(x, y)) > 240,
                "({x}, {y}) に折り返し外のインクがある: {:?}",
                image.get_pixel(x, y).0
            );
        }
    }
    // 折り返したテキスト自体は描かれている。
    assert!(
        (0..100).any(|y| (10..100).any(|x| luma(image.get_pixel(x, y)) < 100)),
        "テキストが描かれていない"
    );
}
