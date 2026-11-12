//! キャンバス領域の切り出しと PNG 保存。

use std::path::Path;

use eframe::egui::{ColorImage, Pos2, Rect, Vec2};

/// スクリーンショットからキャンバス矩形だけを切り出す。
///
/// `rect` はビューポート座標の pt。画面外・範囲外は**先にクランプ**する
/// （`ColorImage::region` は範囲外で panic するため）。
pub fn crop(image: &ColorImage, rect: Rect, pixels_per_point: f32) -> Result<ColorImage, String> {
    if !rect.is_positive() {
        return Err("キャンバスが空です".to_owned());
    }

    let ppp = pixels_per_point.max(0.1);
    let full = Rect::from_min_size(
        Pos2::ZERO,
        Vec2::new(
            image.size[0] as f32 / ppp,
            image.size[1] as f32 / ppp,
        ),
    );
    let clipped = rect.intersect(full);
    if clipped.width() < 1.0 || clipped.height() < 1.0 {
        return Err("キャンバスが画面外にあるため書き出せません".to_owned());
    }

    Ok(image.region(&clipped, Some(ppp)))
}

/// premultiplied な `ColorImage` を straight alpha の RGBA に直す。
pub fn to_rgba(image: &ColorImage) -> image::RgbaImage {
    let (width, height) = (image.size[0] as u32, image.size[1] as u32);
    let mut bytes = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for pixel in &image.pixels {
        let [r, g, b, a] = pixel.to_srgba_unmultiplied();
        bytes.extend_from_slice(&[r, g, b, a]);
    }
    image::RgbaImage::from_raw(width, height, bytes).expect("バッファ長は幅×高さ×4")
}

/// PNG として書き出す。親ディレクトリが無ければ作る。
pub fn save_png(image: &ColorImage, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("保存先を作れません: {err}"))?;
    }
    to_rgba(image)
        .save(path)
        .map_err(|err| format!("PNG を保存できません: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;

    fn image(width: usize, height: usize) -> ColorImage {
        ColorImage::filled([width, height], Color32::WHITE)
    }

    #[test]
    fn crop_takes_requested_region() {
        let image = image(100, 100);
        let cropped = crop(&image, Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(30.0, 40.0)), 1.0)
            .expect("crop");
        assert_eq!(cropped.size, [30, 40]);
    }

    #[test]
    fn crop_uses_pixels_per_point() {
        let image = image(200, 200);
        // 50pt × ppp2 = 100px
        let cropped = crop(&image, Rect::from_min_size(Pos2::ZERO, Vec2::new(50.0, 50.0)), 2.0)
            .expect("crop");
        assert_eq!(cropped.size, [100, 100]);
    }

    #[test]
    fn crop_clamps_to_image_bounds() {
        let image = image(60, 60);
        let cropped = crop(&image, Rect::from_min_size(Pos2::new(40.0, 40.0), Vec2::new(100.0, 100.0)), 1.0)
            .expect("crop");
        assert_eq!(cropped.size, [20, 20]);
    }

    #[test]
    fn crop_rejects_empty_and_offscreen() {
        let image = image(10, 10);
        assert!(crop(&image, Rect::NOTHING, 1.0).is_err());
        assert!(
            crop(&image, Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::new(10.0, 10.0)), 1.0)
                .is_err()
        );
    }

    #[test]
    fn rgba_conversion_unmultiplies_alpha() {
        let mut image = image(1, 2);
        image.pixels[0] = Color32::from_rgba_unmultiplied(200, 100, 50, 128);
        image.pixels[1] = Color32::WHITE;

        let rgba = to_rgba(&image);
        assert_eq!(rgba.dimensions(), (1, 2));
        let first = rgba.get_pixel(0, 0).0;
        let second = rgba.get_pixel(0, 1).0;
        assert_eq!(second, [255, 255, 255, 255]);
        // 半透明は straight alpha に戻る（丸め誤差は許容）
        assert!(first[3] > 120 && first[3] < 136, "{first:?}");
        assert!(first[0] > 190, "{first:?}");
    }

    #[test]
    fn save_png_creates_parent_directory() {
        let dir = std::env::temp_dir().join(format!("dood-test-{}", std::process::id()));
        let path = dir.join("nested/out.png");
        let _ = std::fs::remove_dir_all(&dir);

        save_png(&image(4, 3), &path).expect("save");
        let meta = std::fs::metadata(&path).expect("file exists");
        assert!(meta.len() > 0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
