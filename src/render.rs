//! シーンの描画。画面表示とテスト（画素検証）で同じ関数を使う。

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2,
};

use crate::scene::{Item, Scene};

/// キャンバス全体（背景＋要素）を描く。
pub fn render_scene(painter: &Painter, rect: Rect, scene: &Scene) {
    let painter = painter.with_clip_rect(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, Color32::WHITE);

    if let Some(background) = &scene.bg {
        painter.image(
            background.texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    for item in &scene.items {
        draw_item_on(&painter, rect, item);
    }
}

/// 1要素だけを、キャンバス矩形を基準に描く。ドラッグ中のプレビューにも使う。
pub fn draw_item_on(painter: &Painter, rect: Rect, item: &Item) {
    let origin = rect.min;
    let at = |local: Pos2| origin + local.to_vec2();

    match item {
        Item::Stroke {
            points,
            color,
            width,
        } => match points.len() {
            0 => {}
            1 => {
                painter.circle_filled(at(points[0]), width * 0.5, *color);
            }
            _ => {
                let points = points.iter().map(|p| at(*p)).collect::<Vec<_>>();
                painter.add(Shape::line(points, Stroke::new(*width, *color)));
            }
        },
        Item::Line {
            a,
            b,
            color,
            width,
        } => {
            painter.line_segment([at(*a), at(*b)], Stroke::new(*width, *color));
        }
        Item::Arrow {
            a,
            b,
            color,
            width,
        } => {
            draw_arrow(painter, at(*a), at(*b), *color, *width);
        }
        Item::Rect {
            a,
            b,
            color,
            width,
        } => {
            let rect = Rect::from_two_pos(at(*a), at(*b));
            painter.add(Shape::rect_stroke(
                rect,
                CornerRadius::ZERO,
                Stroke::new(*width, *color),
                StrokeKind::Inside,
            ));
        }
        Item::Ellipse {
            center,
            radius,
            color,
            width,
        } => {
            painter.add(Shape::ellipse_stroke(
                at(*center),
                *radius,
                Stroke::new(*width, *color),
            ));
        }
        Item::Text {
            pos,
            text,
            size,
            color,
        } => {
            painter.text(
                at(*pos),
                Align2::LEFT_TOP,
                text,
                FontId::proportional(*size),
                *color,
            );
        }
    }
}

/// 矢印（線＋先端2本）。
pub fn draw_arrow(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32) {
    painter.line_segment([from, to], Stroke::new(width, color));

    let direction = to - from;
    let length = direction.length();
    if length < 1.0 {
        return;
    }
    let unit = direction / length;
    let head = (width * 3.5).max(8.0).min(length * 0.5);

    for sign in [-1.0_f32, 1.0] {
        let (sin, cos) = (0.45 * sign).sin_cos();
        let rotated = Vec2::new(unit.x * cos - unit.y * sin, unit.x * sin + unit.y * cos);
        painter.line_segment([to, to - rotated * head], Stroke::new(width, color));
    }
}
