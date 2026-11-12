//! Material Design 3 (M3) Baseline Light カラートークンとカスタムウィジェット。
//!
//! 参照: https://m3.material.io/styles/color/roles
//!       https://m3.material.io/components/segmented-buttons/specs
//!       https://m3.material.io/components/buttons/specs
//!       https://m3.material.io/components/toolbars/specs

use std::sync::OnceLock;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontId, FontInsert, FontPriority, InsertFontFamily,
    Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
    epaint::text::VariationCoords, pos2, text::Galley, vec2,
};

use crate::tool::Tool;

// ---- M3 Baseline Light Color Tokens ----------------------------------------

pub const PRIMARY: Color32 = Color32::from_rgb(0x67, 0x50, 0xA4);
pub const ON_PRIMARY: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const PRIMARY_CONTAINER: Color32 = Color32::from_rgb(0xEA, 0xDD, 0xFF);
pub const ON_PRIMARY_CONTAINER: Color32 = Color32::from_rgb(0x21, 0x00, 0x5D);

pub const SECONDARY: Color32 = Color32::from_rgb(0x62, 0x5B, 0x71);
pub const SECONDARY_CONTAINER: Color32 = Color32::from_rgb(0xE8, 0xDE, 0xF8);
pub const ON_SECONDARY_CONTAINER: Color32 = Color32::from_rgb(0x1D, 0x19, 0x2B);

pub const ERROR: Color32 = Color32::from_rgb(0xB3, 0x26, 0x1E);
pub const ON_ERROR: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const ERROR_CONTAINER: Color32 = Color32::from_rgb(0xF9, 0xDE, 0xDC);
pub const ON_ERROR_CONTAINER: Color32 = Color32::from_rgb(0x41, 0x0E, 0x0B);

pub const INVERSE_SURFACE: Color32 = Color32::from_rgb(0x32, 0x2F, 0x35);
pub const INVERSE_ON_SURFACE: Color32 = Color32::from_rgb(0xF5, 0xEF, 0xF7);
pub const INVERSE_PRIMARY: Color32 = Color32::from_rgb(0xD0, 0xBC, 0xFF);

pub const SURFACE: Color32 = Color32::from_rgb(0xFE, 0xF7, 0xFF);
pub const ON_SURFACE: Color32 = Color32::from_rgb(0x1D, 0x1B, 0x20);
pub const ON_SURFACE_VARIANT: Color32 = Color32::from_rgb(0x49, 0x45, 0x4F);

pub const SURFACE_CONTAINER_LOWEST: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const SURFACE_CONTAINER_LOW: Color32 = Color32::from_rgb(0xF7, 0xF2, 0xFA);
pub const SURFACE_CONTAINER: Color32 = Color32::from_rgb(0xF3, 0xED, 0xF7);
pub const SURFACE_CONTAINER_HIGH: Color32 = Color32::from_rgb(0xEC, 0xE6, 0xF0);
pub const SURFACE_CONTAINER_HIGHEST: Color32 = Color32::from_rgb(0xE6, 0xE0, 0xE9);

pub const OUTLINE: Color32 = Color32::from_rgb(0x79, 0x74, 0x7E);
pub const OUTLINE_VARIANT: Color32 = Color32::from_rgb(0xCA, 0xC4, 0xD0);

/// コントロール（セグメントボタン・丸型アイコンボタンなど）の輪郭。
///
/// M3 Baseline の `outline` (#79747E) は白背景に対して 4.5:1 あり、ツールバーで
/// 1pt の輪郭が長く連なると重く見える。背景の `surfaceContainer` (#F3EDF7) と
/// 同じ紫寄りの色相まで寄せ、境界と分かる範囲で明るくした値を使う
/// （白背景で約2.1:1）。面の分割線・キャンバス外周は `OUTLINE_VARIANT` のまま。
pub const CONTROL_OUTLINE: Color32 = Color32::from_rgb(0xBC, 0xAE, 0xC7);

/// ツールバー内の標準コントロール高さ（M3 Compact Density: 32pt）。
pub const CONTROL_HEIGHT: f32 = 32.0;
/// グループを載せるプレートの角丸。
pub const PLATE_RADIUS: u8 = 10;
/// プレート内のボタンの角丸（選択・ホバーの塗り）。
pub const BUTTON_RADIUS: u8 = 8;
/// プレートの内側余白。
pub const PLATE_PAD_X: f32 = 4.0;
pub const PLATE_PAD_Y: f32 = 2.0;
/// プレート内のボタン間隔。
pub const PLATE_ITEM_GAP: f32 = 2.0;
/// プレート1枚の高さ。ツールバーの行の高さでもある。
pub const ROW_HEIGHT: f32 = CONTROL_HEIGHT + PLATE_PAD_Y * 2.0;
pub const PILL_RADIUS: u8 = 16;

/// 描画ツールのアイコンボタン幅（文字ラベルは持たない）。
pub const TOOL_BUTTON_WIDTH: f32 = 36.0;
/// 線幅ボタンの幅。
pub const WIDTH_BUTTON_WIDTH: f32 = 36.0;
/// 枠なしアイコンボタン（元に戻す・やり直す・消去・ペースト・コピー・閉じる）の一辺。
pub const ICON_BUTTON_SIZE: f32 = 32.0;

/// カラースウォッチの当たり判定（正方形）の一辺。
pub const SWATCH_SIZE: f32 = 24.0;

// ---- Fonts -----------------------------------------------------------------

/// UI フォントに要求するウェイト。
///
/// システムによっては既定の欧文フォントが Light（egui 同梱の Ubuntu-Light など）で、
/// また Noto Sans CJK の可変フォントは既定が Thin（wght=100）なので、そのままでは細すぎる。
/// Medium（500）を要求し、可変フォントには `wght` 軸も明示する。
const UI_FONT_WEIGHT: f32 = 500.0;
// Thin / Light（100–300）を引くと UI が細すぎるため、Regular 以上を要求する。
const _: () = assert!(UI_FONT_WEIGHT >= 400.0);

#[derive(Clone)]
struct SystemUiFont {
    family: String,
    data: FontData,
}

/// システムの和文・欧文統合フォントを1つ読み、プロセス内で使い回す。
///
/// パス決め打ちでは NixOS などフォントの配置がビルドごとに変わる環境を拾えないため、
/// fontique（Linux は fontconfig、macOS は CoreText）でシステムフォントを引く。
/// 和文と欧文の両方を描けるフォントを優先し、無ければ最初の候補にフォールバックする。
fn system_ui_font() -> Option<&'static SystemUiFont> {
    static CACHE: OnceLock<Option<SystemUiFont>> = OnceLock::new();
    CACHE.get_or_init(load_system_ui_font).as_ref()
}

fn load_system_ui_font() -> Option<SystemUiFont> {
    let mut collection = fontique::Collection::new(fontique::CollectionOptions {
        shared: false,
        system_fonts: true,
    });
    let mut source_cache = fontique::SourceCache::new(fontique::SourceCacheOptions::default());
    let locale = fontique::Language::parse("ja").ok();

    let mut first_match: Option<fontique::QueryFont> = None;
    let mut full_match: Option<fontique::QueryFont> = None;
    {
        let mut query = collection.query(&mut source_cache);
        query.set_families([fontique::QueryFamily::Generic(
            fontique::GenericFamily::SansSerif,
        )]);
        query.set_attributes(fontique::Attributes {
            weight: fontique::FontWeight::MEDIUM,
            ..Default::default()
        });
        query.set_fallbacks(fontique::FallbackKey::new(
            fontique::Script::from_bytes(*b"Hani"),
            locale.as_ref(),
        ));
        query.matches_with(|font| {
            first_match.get_or_insert_with(|| font.clone());
            if can_render(font, 'A') && can_render(font, 'あ') {
                full_match = Some(font.clone());
                fontique::QueryStatus::Stop
            } else {
                fontique::QueryStatus::Continue
            }
        });
    }

    let font = full_match.or(first_match)?;
    let family = collection
        .family_name(font.family.0)
        .unwrap_or("system")
        .to_owned();
    let index = font.index;
    let (blob, _) = font.blob.into_raw_parts();
    let mut data = FontData::from_blob(blob, index);

    // 可変フォントは既定が Thin のことがあるため、ウェイト軸を明示する。
    if data.variation_axes().iter().any(|axis| axis.tag == "wght") {
        data = data.tweak(egui::FontTweak {
            coords: VariationCoords::new([(b"wght", UI_FONT_WEIGHT)]),
            ..Default::default()
        });
    }

    Some(SystemUiFont { family, data })
}

fn can_render(font: &fontique::QueryFont, c: char) -> bool {
    font.charmap().and_then(|charmap| charmap.map(c)).is_some()
}

/// システムの和文・欧文統合フォントを `Proportional` の最優先に登録する。
///
/// 見つからなければ何もしない。その場合も eframe の `system_font_fallback` が
/// 従来どおり和文を描く（字形・メトリクスは環境依存）。フォントは同梱しない。
pub fn configure_fonts(ctx: &egui::Context) -> Option<String> {
    let font = system_ui_font()?;
    ctx.add_font(FontInsert::new(
        &format!("dood-ui-{}", font.family),
        font.data.clone(),
        vec![InsertFontFamily {
            family: egui::FontFamily::Proportional,
            priority: FontPriority::Highest,
        }],
    ));
    Some(font.family.clone())
}

/// egui コンテキスト全体に M3 Baseline Light の Visuals を設定する。
pub fn apply_m3_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = SURFACE_CONTAINER;
    visuals.window_fill = SURFACE;
    visuals.override_text_color = Some(ON_SURFACE);
    visuals.widgets.noninteractive.bg_fill = SURFACE_CONTAINER;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, OUTLINE_VARIANT);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, ON_SURFACE);
    visuals.widgets.inactive.bg_fill = SURFACE_CONTAINER_HIGH;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, ON_SURFACE);
    visuals.widgets.hovered.bg_fill = SECONDARY_CONTAINER;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, ON_SECONDARY_CONTAINER);
    visuals.widgets.active.bg_fill = PRIMARY_CONTAINER;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, ON_PRIMARY_CONTAINER);
    visuals.selection.bg_fill = SECONDARY_CONTAINER;
    visuals.selection.stroke = Stroke::new(1.0, ON_SECONDARY_CONTAINER);
    ctx.set_visuals(visuals);
}

/// M3 State Layer の色合成（base の上に state_color を opacity で重ねる）。
pub fn blend_state(base: Color32, state_color: Color32, opacity: f32) -> Color32 {
    let [br, bg, bb, _] = base.to_srgba_unmultiplied();
    let [sr, sg, sb, _] = state_color.to_srgba_unmultiplied();
    let inv = 1.0 - opacity;
    let r = (br as f32 * inv + sr as f32 * opacity).round() as u8;
    let g = (bg as f32 * inv + sg as f32 * opacity).round() as u8;
    let b = (bb as f32 * inv + sb as f32 * opacity).round() as u8;
    Color32::from_rgb(r, g, b)
}

pub fn disabled_fg() -> Color32 {
    Color32::from_rgba_unmultiplied(0x1D, 0x1B, 0x20, 97) // 38% opacity
}

// ---- Text metrics ----------------------------------------------------------

const PILL_LABEL_SIZE: f32 = 12.5;
const ICON_SIZE: f32 = 14.0;
const ICON_GAP: f32 = 4.0;
const PILL_PAD_X: f32 = 10.0;
/// 線幅プレビューの横棒の長さ。
const WIDTH_PREVIEW_LENGTH: f32 = 18.0;

fn label_galley(ui: &Ui, text: &str, size: f32) -> std::sync::Arc<Galley> {
    ui.painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(size), ON_SURFACE)
}

/// テキストの実寸ボックスを `center_y` に上下中央揃えで描く。
///
/// `Align2::LEFT_CENTER` はフォントの行メトリクス（内部パディング）基準で中心を決めるため、
/// 和文・欧文が混ざるとボタン枠の中心からずれる。実寸の galley を中心に置く。
fn paint_label(
    painter: &Painter,
    left_x: f32,
    center_y: f32,
    galley: std::sync::Arc<Galley>,
    color: Color32,
) {
    let top_left = pos2(left_x, center_y - galley.size().y * 0.5);
    // `Painter::galley` は galley 内の色を優先するため、ボタン側の色で上書きする。
    painter.galley_with_override_text_color(top_left, galley, color);
}

/// アイコン付きピルボタンの横幅（ツールバーの折り返し計算に使う）。
pub fn pill_button_width(ui: &Ui, label: &str) -> f32 {
    let galley = label_galley(ui, label, PILL_LABEL_SIZE);
    (PILL_PAD_X * 2.0 + ICON_SIZE + ICON_GAP + galley.size().x).ceil()
}

// ---- Toolbar plate geometry ------------------------------------------------

/// プレートの幅（内側のコンテンツ幅から外側の寸法を求める）。
pub const fn plate_width(content_width: f32) -> f32 {
    content_width + PLATE_PAD_X * 2.0
}

/// 描画ツール6個を並べたプレートの幅。
pub const fn tools_group_width() -> f32 {
    let count = Tool::ALL.len();
    plate_width(
        TOOL_BUTTON_WIDTH * count as f32 + PLATE_ITEM_GAP * (count.saturating_sub(1)) as f32,
    )
}

/// 線幅3個を並べたプレートの幅。
pub const fn widths_group_width() -> f32 {
    let count = crate::tool::WIDTHS.len();
    plate_width(
        WIDTH_BUTTON_WIDTH * count as f32 + PLATE_ITEM_GAP * (count.saturating_sub(1)) as f32,
    )
}

/// 元に戻す・やり直すを並べたプレートの幅。
pub const fn history_group_width() -> f32 {
    plate_width(ICON_BUTTON_SIZE * 2.0 + PLATE_ITEM_GAP)
}

/// 消去ボタン1個のプレートの幅。
pub const fn clear_group_width() -> f32 {
    plate_width(ICON_BUTTON_SIZE)
}

// ---- M3 Toolbar Plates -----------------------------------------------------

/// ツールバーのグループをまとめる角丸プレート。
///
/// 各ボタンの1pt枠線の代わりに、グループ単位の面（`SURFACE_CONTAINER_HIGH`）で
/// まとまりを示す。中身は左から右へ、固定幅で並べる。
pub fn group_plate(ui: &mut Ui, width: f32, fill: Color32, add_contents: impl FnOnce(&mut Ui)) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(PLATE_RADIUS), fill);

    let inner = rect.shrink2(vec2(PLATE_PAD_X, PLATE_PAD_Y));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing = vec2(PLATE_ITEM_GAP, 0.0);
    add_contents(&mut child);
}

/// ツールバーのボタンが受け取る入力。
///
/// 枠なしウィンドウではツールバーの余白ドラッグがウィンドウ移動になる。
/// ボタン上からのドラッグをボタン側で受け止め、ウィンドウが動かないようにする。
fn button_sense() -> Sense {
    Sense::click_and_drag()
}

/// 選択・ホバー時の塗り。プレートの面を基準に state layer を重ねる。
fn selection_fill(selected: bool, hovered: bool, pressed: bool) -> Color32 {
    if !selected && !hovered && !pressed {
        return Color32::TRANSPARENT;
    }
    let base = if selected {
        SECONDARY_CONTAINER
    } else {
        SURFACE_CONTAINER_HIGH
    };
    let state = if selected {
        ON_SECONDARY_CONTAINER
    } else {
        ON_SURFACE
    };
    if pressed {
        blend_state(base, state, 0.12)
    } else if hovered {
        blend_state(base, state, 0.08)
    } else {
        SECONDARY_CONTAINER
    }
}

fn selection_fg(selected: bool, hovered: bool) -> Color32 {
    if selected {
        ON_SECONDARY_CONTAINER
    } else if hovered {
        ON_SURFACE
    } else {
        ON_SURFACE_VARIANT
    }
}

/// 描画ツールのアイコンのみのボタン。選択中は `secondaryContainer` で塗る。
pub fn tool_button(ui: &mut Ui, tool: Tool, selected: bool) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(TOOL_BUTTON_WIDTH, CONTROL_HEIGHT), button_sense());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let fg = selection_fg(selected, response.hovered());
    let fill = selection_fill(
        selected,
        response.hovered(),
        response.is_pointer_button_down_on(),
    );

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(BUTTON_RADIUS), fill);
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(15.0));
    draw_tool_icon(painter, icon_rect, tool, fg);

    response
}

/// 線幅ボタン。数値ラベルを持たず、実寸の太さの横棒だけで示す。
pub fn width_button(ui: &mut Ui, width_pt: f32, selected: bool) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(WIDTH_BUTTON_WIDTH, CONTROL_HEIGHT), button_sense());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let fg = selection_fg(selected, response.hovered());
    let fill = selection_fill(
        selected,
        response.hovered(),
        response.is_pointer_button_down_on(),
    );

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(BUTTON_RADIUS), fill);

    // 実際の太さが直感でわかる横棒を中央に置く。
    let bar_h = width_pt.clamp(2.0, 8.0);
    let bar_rect = Rect::from_center_size(rect.center(), vec2(WIDTH_PREVIEW_LENGTH, bar_h));
    painter.rect_filled(
        bar_rect,
        CornerRadius::same((bar_h * 0.5).round() as u8),
        fg,
    );

    response
}

/// 枠線なしのフラットなアイコンボタン（元に戻す・やり直す・ペースト・コピー）。
pub fn flat_icon_button(ui: &mut Ui, icon: ActionIcon, enabled: bool) -> Response {
    let sense = if enabled {
        button_sense()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(ICON_BUTTON_SIZE), sense);
    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let pressed = response.is_pointer_button_down_on();
    let fg = if !enabled {
        disabled_fg()
    } else if response.hovered() || pressed {
        ON_SURFACE
    } else {
        ON_SURFACE_VARIANT
    };
    let fill = if !enabled {
        Color32::TRANSPARENT
    } else if pressed {
        blend_state(SURFACE_CONTAINER_HIGH, ON_SURFACE, 0.12)
    } else if response.hovered() {
        blend_state(SURFACE_CONTAINER_HIGH, ON_SURFACE, 0.08)
    } else {
        Color32::TRANSPARENT
    };

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(BUTTON_RADIUS), fill);
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(15.0));
    draw_action_icon(painter, icon_rect, icon, fg);

    response
}

/// 消去ボタン。通常は無彩色、ホバー時だけ Error 色に寄せる。
pub fn clear_button(ui: &mut Ui, enabled: bool) -> Response {
    let sense = if enabled {
        button_sense()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(ICON_BUTTON_SIZE), sense);
    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let pressed = response.is_pointer_button_down_on();
    let highlighted = enabled && (response.hovered() || pressed);
    let fg = if !enabled {
        disabled_fg()
    } else if highlighted {
        ERROR
    } else {
        ON_SURFACE_VARIANT
    };
    let fill = if !enabled {
        Color32::TRANSPARENT
    } else if pressed {
        blend_state(SURFACE_CONTAINER_HIGH, ERROR, 0.12)
    } else if response.hovered() {
        blend_state(SURFACE_CONTAINER_HIGH, ERROR, 0.08)
    } else {
        Color32::TRANSPARENT
    };

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(BUTTON_RADIUS), fill);
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(15.0));
    draw_action_icon(painter, icon_rect, ActionIcon::Clear, fg);

    response
}

// ---- Window controls -------------------------------------------------------

/// ウィンドウを閉じるボタン。枠なしウィンドウなので、閉じる操作をツールバー右端にも置く。
pub fn close_button(ui: &mut Ui) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(ICON_BUTTON_SIZE), button_sense());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let pressed = response.is_pointer_button_down_on();
    let fill = if pressed {
        blend_state(SURFACE_CONTAINER, ERROR, 0.12)
    } else if response.hovered() {
        blend_state(SURFACE_CONTAINER, ERROR, 0.08)
    } else {
        Color32::TRANSPARENT
    };

    // 閉じる操作だと分かるよう、ホバー時だけ Error 色に寄せる。
    let color = if response.hovered() || pressed {
        ERROR
    } else {
        ON_SURFACE_VARIANT
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(BUTTON_RADIUS), fill);

    let stroke = Stroke::new(1.7, color);
    let half = 5.5;
    let center = rect.center();
    painter.line_segment(
        [center + vec2(-half, -half), center + vec2(half, half)],
        stroke,
    );
    painter.line_segment(
        [center + vec2(half, -half), center + vec2(-half, half)],
        stroke,
    );

    response
}

// ---- M3 Color Swatch -------------------------------------------------------

/// 円形のカラースウォッチ。選択時は外周の Primary リングと内側のチェックマーク（✓）を描く。
pub fn color_swatch(ui: &mut Ui, color: Color32, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(SWATCH_SIZE), button_sense());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let center = rect.center();
    let base_radius = if response.hovered() && !selected {
        9.5
    } else {
        8.5
    };
    let painter = ui.painter();

    // 選択時は外側に M3 Primary のフォーカスリングを表示
    if selected {
        painter.circle_stroke(center, base_radius + 2.5, Stroke::new(2.0, PRIMARY));
    } else if response.hovered() {
        painter.circle_stroke(center, base_radius + 2.0, Stroke::new(1.5, OUTLINE_VARIANT));
    }

    painter.circle_filled(center, base_radius, color);

    // 白色スウォッチでも背景に埋もれないよう枠線を入れる
    let is_light = is_light_color(color);
    let border_color = if is_light {
        CONTROL_OUTLINE
    } else {
        Color32::from_rgba_unmultiplied(0, 0, 0, 40)
    };
    painter.circle_stroke(center, base_radius, Stroke::new(1.0, border_color));

    if selected {
        let check_color = if is_light { ON_SURFACE } else { Color32::WHITE };
        draw_check_icon(painter, center, 4.0, check_color);
    }

    response
}

fn is_light_color(color: Color32) -> bool {
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    let luma = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
    luma > 200
}

// ---- M3 Action Buttons -----------------------------------------------------

#[derive(Clone, Copy)]
pub enum ActionIcon {
    Undo,
    Redo,
    Clear,
    Copy,
    Save,
    Paste,
}

/// 保存だけ残すテキスト付きの Filled ピルボタン。
///
/// ツールバーで文字を持つのはこのボタンだけで、ほかの操作はアイコンとツールチップで示す。
pub fn save_button(ui: &mut Ui, label: &str) -> Response {
    let text_galley = label_galley(ui, label, PILL_LABEL_SIZE);
    let width = pill_button_width(ui, label);

    let (rect, response) = ui.allocate_exact_size(vec2(width, CONTROL_HEIGHT), button_sense());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let fill = if response.is_pointer_button_down_on() {
        blend_state(PRIMARY, ON_PRIMARY, 0.14)
    } else if response.hovered() {
        blend_state(PRIMARY, ON_PRIMARY, 0.08)
    } else {
        PRIMARY
    };

    let radius = CornerRadius::same(PILL_RADIUS);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, fill);

    let icon_rect = Rect::from_center_size(
        pos2(rect.min.x + PILL_PAD_X + ICON_SIZE * 0.5, rect.center().y),
        Vec2::splat(ICON_SIZE),
    );
    draw_action_icon(painter, icon_rect, ActionIcon::Save, ON_PRIMARY);

    paint_label(
        painter,
        icon_rect.max.x + ICON_GAP,
        rect.center().y,
        text_galley,
        ON_PRIMARY,
    );

    response
}

// ---- Vector Icons (Painter Primitives) -------------------------------------

pub fn draw_tool_icon(painter: &Painter, rect: Rect, tool: Tool, color: Color32) {
    let c = rect.center();
    let stroke = Stroke::new(1.7, color);

    match tool {
        Tool::Pen => {
            // 斜めのペン軸＋ペン先＋小さな筆跡ライン
            let tip = pos2(rect.min.x + 2.0, rect.max.y - 2.5);
            let mid_l = pos2(rect.min.x + 4.5, rect.max.y - 7.5);
            let mid_r = pos2(rect.min.x + 7.5, rect.max.y - 4.5);
            let top_l = pos2(rect.max.x - 5.5, rect.min.y + 2.0);
            let top_r = pos2(rect.max.x - 2.5, rect.min.y + 5.0);

            painter.add(Shape::line(
                vec![tip, mid_l, top_l, top_r, mid_r, tip],
                Stroke::new(1.4, color),
            ));
            painter.line_segment(
                [
                    pos2(rect.min.x + 2.0, rect.max.y - 1.5),
                    pos2(rect.max.x - 2.0, rect.max.y - 1.5),
                ],
                Stroke::new(1.4, color),
            );
        }
        Tool::Line => {
            // 両端にアンカー点を持つ斜線
            let a = pos2(rect.min.x + 2.5, rect.max.y - 2.5);
            let b = pos2(rect.max.x - 2.5, rect.min.y + 2.5);
            painter.line_segment([a, b], stroke);
            painter.circle_filled(a, 2.0, color);
            painter.circle_filled(b, 2.0, color);
        }
        Tool::Arrow => {
            // 右上向きの矢印
            let a = pos2(rect.min.x + 2.0, rect.max.y - 2.0);
            let b = pos2(rect.max.x - 2.0, rect.min.y + 2.0);
            painter.line_segment([a, b], stroke);
            painter.line_segment([b, pos2(b.x - 5.5, b.y)], stroke);
            painter.line_segment([b, pos2(b.x, b.y + 5.5)], stroke);
        }
        Tool::Rect => {
            let r = Rect::from_center_size(c, vec2(12.0, 9.5));
            painter.rect_stroke(r, CornerRadius::same(1), stroke, StrokeKind::Inside);
        }
        Tool::Ellipse => {
            painter.add(Shape::ellipse_stroke(c, vec2(6.5, 4.8), stroke));
        }
        Tool::Text => {
            // セリフ付きの「T」アイコン
            let top_y = rect.min.y + 2.5;
            let bot_y = rect.max.y - 2.5;
            let left_x = rect.min.x + 2.5;
            let right_x = rect.max.x - 2.5;
            painter.line_segment([pos2(left_x, top_y), pos2(right_x, top_y)], stroke);
            painter.line_segment(
                [pos2(left_x, top_y), pos2(left_x, top_y + 2.5)],
                Stroke::new(1.4, color),
            );
            painter.line_segment(
                [pos2(right_x, top_y), pos2(right_x, top_y + 2.5)],
                Stroke::new(1.4, color),
            );
            painter.line_segment([pos2(c.x, top_y), pos2(c.x, bot_y)], stroke);
            painter.line_segment(
                [pos2(c.x - 2.5, bot_y), pos2(c.x + 2.5, bot_y)],
                Stroke::new(1.4, color),
            );
        }
    }
}

/// U ターン矢印の折れ線。左（ミラー時は右）端の矢印先端から上辺をたどり、
/// 反対側の端で半円を 16 分割した滑らかな弧を描いて下辺へ戻る。
fn u_turn_points(rect: Rect, mirror: bool) -> Vec<Pos2> {
    const SEGMENTS: usize = 16;
    const INSET: f32 = 2.5;

    let radius = (rect.height() * 0.5 - 3.0).clamp(3.0, 5.0);
    let cy = rect.center().y;
    let width = rect.width();
    let map_x = |x: f32| {
        if mirror {
            rect.max.x - x
        } else {
            rect.min.x + x
        }
    };

    let arc_cx = width - INSET - radius;
    let top_y = cy - radius;
    let bottom_y = cy + radius;

    let mut points = Vec::with_capacity(SEGMENTS + 3);
    points.push(pos2(map_x(INSET), top_y));
    points.push(pos2(map_x(arc_cx), top_y));
    for step in 1..=SEGMENTS {
        let t = step as f32 / SEGMENTS as f32;
        let angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * t;
        points.push(pos2(
            map_x(arc_cx + radius * angle.cos()),
            cy + radius * angle.sin(),
        ));
    }
    points.push(pos2(map_x(INSET), bottom_y));
    points
}

fn draw_u_turn_arrow(painter: &Painter, rect: Rect, color: Color32, mirror: bool) {
    const INSET: f32 = 2.5;
    const HEAD: f32 = 3.2;

    let stroke = Stroke::new(1.6, color);
    let points = u_turn_points(rect, mirror);
    let tip = points[0];

    painter.add(Shape::line(points, stroke));

    let map_x = |x: f32| {
        if mirror {
            rect.max.x - x
        } else {
            rect.min.x + x
        }
    };
    painter.line_segment(
        [tip, pos2(map_x(INSET + HEAD), tip.y - HEAD * 0.85)],
        stroke,
    );
    painter.line_segment(
        [tip, pos2(map_x(INSET + HEAD), tip.y + HEAD * 0.85)],
        stroke,
    );
}

pub fn draw_action_icon(painter: &Painter, rect: Rect, icon: ActionIcon, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    let c = rect.center();

    match icon {
        ActionIcon::Undo => draw_u_turn_arrow(painter, rect, color, false),
        ActionIcon::Redo => draw_u_turn_arrow(painter, rect, color, true),
        ActionIcon::Clear => {
            // ゴミ箱アイコン
            let lid_y = rect.min.y + 3.5;
            painter.line_segment(
                [pos2(rect.min.x + 2.0, lid_y), pos2(rect.max.x - 2.0, lid_y)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x - 2.0, lid_y - 1.5), pos2(c.x + 2.0, lid_y - 1.5)],
                stroke,
            );
            let body = Rect::from_min_max(
                pos2(rect.min.x + 3.5, lid_y + 0.5),
                pos2(rect.max.x - 3.5, rect.max.y - 2.0),
            );
            painter.rect_stroke(body, CornerRadius::same(1), stroke, StrokeKind::Inside);
        }
        ActionIcon::Copy => {
            // 重なった2枚のシート
            let back = Rect::from_min_max(
                pos2(rect.min.x + 2.0, rect.min.y + 1.5),
                pos2(rect.max.x - 4.5, rect.max.y - 4.5),
            );
            let front = Rect::from_min_max(
                pos2(rect.min.x + 4.5, rect.min.y + 4.5),
                pos2(rect.max.x - 2.0, rect.max.y - 1.5),
            );
            painter.add(Shape::line(
                vec![
                    pos2(back.min.x, back.max.y),
                    back.min,
                    pos2(back.max.x, back.min.y),
                ],
                stroke,
            ));
            painter.rect_stroke(front, CornerRadius::same(1), stroke, StrokeKind::Inside);
        }
        ActionIcon::Paste => {
            // クリップボード（貼り付け）
            let board = Rect::from_min_max(
                pos2(rect.min.x + 3.0, rect.min.y + 3.0),
                pos2(rect.max.x - 3.0, rect.max.y - 2.0),
            );
            painter.rect_stroke(board, CornerRadius::same(1), stroke, StrokeKind::Inside);
            painter.rect_filled(
                Rect::from_center_size(pos2(c.x, board.min.y), vec2(6.0, 3.0)),
                CornerRadius::same(1),
                color,
            );
        }
        ActionIcon::Save => {
            // 下向き矢印＋トレイ（書き出し・保存）
            let tray_top = rect.max.y - 5.5;
            let tray_bot = rect.max.y - 2.0;
            painter.add(Shape::line(
                vec![
                    pos2(rect.min.x + 2.0, tray_top),
                    pos2(rect.min.x + 2.0, tray_bot),
                    pos2(rect.max.x - 2.0, tray_bot),
                    pos2(rect.max.x - 2.0, tray_top),
                ],
                stroke,
            ));
            let arrow_bot = pos2(c.x, tray_bot - 2.5);
            painter.line_segment([pos2(c.x, rect.min.y + 1.5), arrow_bot], stroke);
            painter.line_segment([arrow_bot, pos2(c.x - 3.2, arrow_bot.y - 3.2)], stroke);
            painter.line_segment([arrow_bot, pos2(c.x + 3.2, arrow_bot.y - 3.2)], stroke);
        }
    }
}

pub fn draw_check_icon(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let p1 = pos2(center.x - size * 0.85, center.y + size * 0.05);
    let p2 = pos2(center.x - size * 0.2, center.y + size * 0.7);
    let p3 = pos2(center.x + size * 0.9, center.y - size * 0.65);
    painter.add(Shape::line(vec![p1, p2, p3], Stroke::new(1.8, color)));
}

/// 通知（トースト）の先頭に置くアイコン。成功はチェック、エラーは感嘆符。
pub fn draw_notification_icon(painter: &Painter, rect: Rect, error: bool, color: Color32) {
    if error {
        let center = rect.center();
        painter.circle_stroke(center, 6.5, Stroke::new(1.6, color));
        painter.line_segment(
            [
                pos2(center.x, center.y - 3.2),
                pos2(center.x, center.y + 1.2),
            ],
            Stroke::new(1.8, color),
        );
        painter.circle_filled(pos2(center.x, center.y + 3.8), 1.1, color);
    } else {
        draw_check_icon(painter, rect.center(), 4.6, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_widths_match_the_fixed_button_layout() {
        // ツール6 × 36 + 間隔2 × 5 + 左右余白4 × 2
        assert_eq!(tools_group_width(), 234.0);
        // 太さ3 × 36 + 間隔2 × 2 + 左右余白4 × 2
        assert_eq!(widths_group_width(), 120.0);
        // 履歴2 × 32 + 間隔2 + 左右余白4 × 2
        assert_eq!(history_group_width(), 74.0);
        // 消去1 × 32 + 左右余白4 × 2
        assert_eq!(clear_group_width(), 40.0);
        assert_eq!(ROW_HEIGHT, 36.0);
    }

    #[test]
    fn u_turn_arrow_is_smooth_and_mirrored() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(15.0, 15.0));
        let points = u_turn_points(rect, false);
        assert!(points.len() >= 18, "弧の分割が足りない");

        let cy = rect.center().y;
        let first = points[0];
        let last = points[points.len() - 1];
        assert!(
            (first.y - (2.0 * cy - last.y)).abs() < 0.01,
            "上下が対称でない"
        );

        // 連続する線分の向きの変化が小さい（折れ線が滑らか）
        for window in points[1..].windows(3) {
            let a = (window[1] - window[0]).angle();
            let b = (window[2] - window[1]).angle();
            let mut delta = (b - a).abs();
            if delta > std::f32::consts::PI {
                delta = 2.0 * std::f32::consts::PI - delta;
            }
            assert!(delta < 0.35, "弧の分割が粗い: {delta}");
        }

        let mirrored = u_turn_points(rect, true);
        assert_eq!(points.len(), mirrored.len());
        let two_cx = 2.0 * rect.center().x;
        for (a, b) in points.iter().zip(&mirrored) {
            assert!((a.x + b.x - two_cx).abs() < 0.01, "左右反転になっていない");
            assert!((a.y - b.y).abs() < 0.01);
        }
    }

    #[test]
    fn state_layer_blending_interpolates_rgb() {
        let base = Color32::WHITE;
        let state = Color32::BLACK;
        let blended = blend_state(base, state, 0.1);
        let [r, g, b, a] = blended.to_srgba_unmultiplied();
        assert_eq!(a, 255);
        assert!((228..=231).contains(&r), "r={r}");
        assert_eq!(r, g);
        assert_eq!(g, b);
    }
}
