//! アプリ本体：状態・入力・ツールバー・書き出しの調停。

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use eframe::egui::{
    self, Color32, ColorImage, Id, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit,
    Vec2,
};

use crate::{
    export, render,
    scene::{Background, History, Item, Scene},
    theme::{self, ActionIcon},
    tool::{COLOR_KEYS, PALETTE, TEXT_SIZES, THICKER_KEY, THINNER_KEY, Tool, WIDTHS},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportKind {
    Copy,
    Save,
}

/// 画面右上に一時表示する通知。一定時間で自動的に消える。
#[derive(Clone, Default)]
pub struct Status {
    pub text: String,
    /// 保存先など、本文に添える補足。本文より小さく折り返して出す。
    pub detail: Option<String>,
    pub error: bool,
    pub expires_at: Option<Instant>,
}

impl Status {
    /// 成功メッセージの表示時間。
    pub const SUCCESS_TTL: Duration = Duration::from_secs(3);
    /// エラーメッセージの表示時間（読む時間を長めに取る）。
    pub const ERROR_TTL: Duration = Duration::from_secs(6);

    /// メッセージを設定し、表示期限を `now` から数える。
    pub fn set(&mut self, text: impl Into<String>, error: bool, now: Instant) {
        self.set_with_detail(text, None, error, now);
    }

    /// 補足付きのメッセージを設定し、表示期限を `now` から数える。
    pub fn set_with_detail(
        &mut self,
        text: impl Into<String>,
        detail: Option<String>,
        error: bool,
        now: Instant,
    ) {
        self.text = text.into();
        self.detail = detail;
        self.error = error;
        let ttl = if error {
            Self::ERROR_TTL
        } else {
            Self::SUCCESS_TTL
        };
        self.expires_at = Some(now + ttl);
    }

    /// 表示すべきメッセージがあれば返す。期限切れなら消して `None`。
    pub fn visible(&mut self, now: Instant) -> Option<(&str, bool)> {
        if self.text.is_empty() {
            return None;
        }
        if self.expires_at.is_some_and(|deadline| now >= deadline) {
            self.clear();
            return None;
        }
        Some((&self.text, self.error))
    }

    /// 表示期限までの残り時間（自動消去の再描画予約に使う）。
    pub fn time_until_expiry(&self, now: Instant) -> Option<Duration> {
        self.expires_at
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.detail = None;
        self.error = false;
        self.expires_at = None;
    }
}

fn set_shared(
    status: &Arc<Mutex<Status>>,
    text: impl Into<String>,
    detail: Option<String>,
    error: bool,
) {
    if let Ok(mut guard) = status.lock() {
        guard.set_with_detail(text, detail, error, Instant::now());
    }
}

/// ドラッグ中の下書き。確定前なので `Scene` には入れない。
enum Draft {
    Freehand {
        points: Vec<Pos2>,
    },
    Shape {
        start: Pos2,
        current: Pos2,
    },
    /// テキストツールのドラッグで指定中のテキストボックス。
    TextBox {
        start: Pos2,
        current: Pos2,
    },
}

struct TextDraft {
    /// 入力欄の矩形（キャンバス左上を原点とする pt）。
    rect: Rect,
    buffer: String,
    /// TextEdit にフォーカスを要求済みか。
    focused: bool,
    color: Color32,
    size: f32,
    /// ドラッグで指定した固定サイズか。クリックで開いた場合は内容に合わせて縦に伸びる。
    fixed_box: bool,
}

enum Action {
    Tool(Tool),
    Color(usize),
    Width(usize),
    Undo,
    Redo,
    Clear,
    Export(ExportKind),
    PasteImage,
}

#[derive(Default)]
struct Pressed {
    copy: bool,
    save: bool,
    close: bool,
    paste_image: bool,
    undo: bool,
    redo: bool,
    clear: bool,
    toggle_top: bool,
    tool: Option<Tool>,
    color: Option<usize>,
    width_delta: i32,
}

// ---- ツールバーの折り返し ---------------------------------------------------

/// ツールバーの折り返し単位。中身が行をまたいで分割されないようグループ単位で詰める。
#[derive(Clone, Copy)]
enum ToolbarGroup {
    Tools,
    Palette,
    Widths,
    History,
    Clear,
}

const TOOLBAR_GROUPS: [ToolbarGroup; 5] = [
    ToolbarGroup::Tools,
    ToolbarGroup::Palette,
    ToolbarGroup::Widths,
    ToolbarGroup::History,
    ToolbarGroup::Clear,
];

const TOOLBAR_PAD_X: i8 = 8;
const TOOLBAR_PAD_Y: i8 = 4;
/// グループ（プレート）間の間隔。区切り線は引かない。
const TOOLBAR_GAP: f32 = 6.0;
const TOOLBAR_ROW_GAP: f32 = 4.0;
/// 書き出しグループと左側グループの最小間隔。
const EXPORT_GAP: f32 = 12.0;
/// 閉じるボタンと、その左隣の要素との最小間隔。
const CLOSE_GAP: f32 = 8.0;
/// 1段目の右端に予約する閉じるボタンぶんの幅（ボタン + 余白）。
const CLOSE_RESERVE: f32 = theme::ICON_BUTTON_SIZE + CLOSE_GAP;

// ---- 通知（トースト） -------------------------------------------------------

/// キャンバス右上からの余白。ツールバーの下に置き、バーには重ねない。
const TOAST_MARGIN: f32 = 12.0;
/// 通知の最大幅。長い保存先は折り返す。
const TOAST_MAX_WIDTH: f32 = 420.0;
/// 通知の最小幅。短いメッセージでもカードとして見えるようにする。
const TOAST_MIN_WIDTH: f32 = 180.0;
const TOAST_RADIUS: u8 = 10;
const TOAST_ICON_SIZE: f32 = 16.0;
const TOAST_ICON_GAP: f32 = 8.0;
const TOAST_PAD_X: f32 = 14.0;
const TOAST_PAD_Y: f32 = 10.0;
const TOAST_HEADLINE_SIZE: f32 = 12.5;
const TOAST_DETAIL_SIZE: f32 = 11.0;

const PALETTE_GAP: f32 = theme::PLATE_ITEM_GAP;

const TEXT_EDIT_WIDTH: f32 = 260.0;
const TEXT_EDIT_PADDING: f32 = 14.0;
const TEXT_EDIT_MIN_HEIGHT: f32 = 30.0;
/// ドラッグでテキストボックスを作る最小サイズ。これ未満はクリック扱い。
const TEXT_BOX_MIN_WIDTH: f32 = 40.0;
const TEXT_BOX_MIN_HEIGHT: f32 = TEXT_EDIT_MIN_HEIGHT;

const fn palette_group_width(count: usize) -> f32 {
    theme::plate_width(
        theme::SWATCH_SIZE * count as f32 + PALETTE_GAP * count.saturating_sub(1) as f32,
    )
}

/// 幅 `widths` のグループを `max_width` に収まるよう上から順に詰める。
///
/// 1行目だけは右端の閉じるボタンぶんを差し引いた `first_row_width` を使う。
/// 返り値は各行に入るグループの添字。
fn wrap_groups(widths: &[f32], max_width: f32, first_row_width: f32, gap: f32) -> Vec<Vec<usize>> {
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut used = 0.0;

    for (index, &width) in widths.iter().enumerate() {
        // 最後の行（まだ行が無ければ1行目）の上限。
        let limit = if rows.len() <= 1 {
            first_row_width
        } else {
            max_width
        };
        match rows.last_mut() {
            Some(row) if !row.is_empty() && used + gap + width <= limit => {
                row.push(index);
                used += gap + width;
            }
            _ => {
                rows.push(vec![index]);
                used = width;
            }
        }
    }

    rows
}

fn row_width(row: &[usize], widths: &[f32]) -> f32 {
    let content: f32 = row.iter().map(|&index| widths[index]).sum();
    let gaps = row.len().saturating_sub(1) as f32 * TOOLBAR_GAP;
    content + gaps
}

/// 折り返した行と、右寄せの書き出しグループを置く行を決める。
///
/// `close_width` は1段目の右端に固定する閉じるボタンの予約幅。折り返しの有無に
/// 関係なく閉じるボタンとほかのボタンが重ならないよう、1行目の内容幅から差し引く。
fn plan_toolbar(
    widths: &[f32],
    export_width: f32,
    close_width: f32,
    max_width: f32,
) -> (Vec<Vec<usize>>, usize) {
    let first_row_width = (max_width - close_width).max(0.0);
    let mut rows = wrap_groups(widths, max_width, first_row_width, TOOLBAR_GAP);
    if rows.is_empty() {
        rows.push(Vec::new());
    }

    let last = rows.len() - 1;
    let limit = if last == 0 {
        first_row_width
    } else {
        max_width
    };
    if limit - row_width(&rows[last], widths) >= export_width + EXPORT_GAP {
        (rows, last)
    } else {
        rows.push(Vec::new());
        let last = rows.len() - 1;
        (rows, last)
    }
}

// ---- 通知の配置 -------------------------------------------------------------

/// 通知の右上アンカー。キャンバス右上の内側に置き、ツールバーには重ねない。
fn toast_anchor(canvas: Rect) -> Pos2 {
    Pos2::new(canvas.max.x - TOAST_MARGIN, canvas.min.y + TOAST_MARGIN)
}

/// 通知が写り込まないよう、撮影を次のフレームへ送るべきか。
///
/// 通知が無ければその場で撮る。通知があれば、通知を描かない次のフレームまで待つ。
fn should_defer_capture(capture_next_frame: bool, notification_visible: bool) -> bool {
    !capture_next_frame && notification_visible
}

// ---- テキスト入力の Enter 判定 ---------------------------------------------

/// `Enter` で入力欄を確定するか。
///
/// `Shift+Enter` は改行（`TextEdit` が `\n` を挿入済み）なので確定しない。
fn commit_on_enter(enter: bool, shift: bool) -> bool {
    enter && !shift
}

pub struct DoodApp {
    scene: Scene,
    history: History,
    tool: Tool,
    color: Color32,
    width_index: usize,
    draft: Option<Draft>,
    text: Option<TextDraft>,
    out_path: PathBuf,
    status: Arc<Mutex<Status>>,
    busy: Arc<AtomicBool>,
    pending_export: Option<ExportKind>,
    /// ツールバーの「ペースト」または ⌘⇧V で、クリップボードを直接読む要求。
    paste_requested: bool,
    /// 通知が写り込まないよう、次のフレームで通知を描かずに撮影する。
    capture_next_frame: bool,
    always_on_top: bool,
    canvas_rect: Rect,
}

impl DoodApp {
    pub fn new(cc: &eframe::CreationContext<'_>, out_path: PathBuf) -> Self {
        Self::with_ctx(&cc.egui_ctx, out_path)
    }

    pub fn with_ctx(ctx: &egui::Context, out_path: PathBuf) -> Self {
        theme::configure_fonts(ctx);
        theme::apply_m3_visuals(ctx);
        // 字形が無いときに panic せず豆腐で描く（和文フォントが引けない環境での保険）。
        ctx.set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
        Self {
            scene: Scene::default(),
            history: History::default(),
            tool: Tool::Pen,
            color: PALETTE[0].1,
            width_index: 1,
            draft: None,
            text: None,
            out_path,
            status: Arc::new(Mutex::new(Status::default())),
            busy: Arc::new(AtomicBool::new(false)),
            pending_export: None,
            paste_requested: false,
            capture_next_frame: false,
            always_on_top: true,
            canvas_rect: Rect::NOTHING,
        }
    }

    pub fn out_path(&self) -> &std::path::Path {
        &self.out_path
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn canvas_rect(&self) -> Rect {
        self.canvas_rect
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub fn push_item(&mut self, item: Item) {
        self.history.record(&self.scene);
        self.scene.push(item);
    }

    fn width(&self) -> f32 {
        WIDTHS[self.width_index]
    }

    fn text_size(&self) -> f32 {
        TEXT_SIZES[self.width_index]
    }

    fn set_status(&self, text: impl Into<String>, error: bool) {
        set_shared(&self.status, text, None, error);
    }

    fn set_status_with_detail(&self, text: impl Into<String>, detail: String, error: bool) {
        set_shared(&self.status, text, Some(detail), error);
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Tool(tool) => self.tool = tool,
            Action::Color(index) => self.color = PALETTE[index].1,
            Action::Width(index) => self.width_index = index,
            Action::Undo => {
                self.history.undo(&mut self.scene);
            }
            Action::Redo => {
                self.history.redo(&mut self.scene);
            }
            Action::Clear => {
                self.history.record(&self.scene);
                self.scene.clear();
            }
            Action::Export(kind) => {
                self.pending_export = Some(kind);
            }
            Action::PasteImage => {
                self.paste_requested = true;
            }
        }
    }

    // ---- ショートカット -------------------------------------------------

    fn read_keys(ctx: &egui::Context) -> Pressed {
        ctx.input_mut(|input| {
            let mut pressed = Pressed {
                copy: input.consume_key(Modifiers::COMMAND, Key::C),
                save: input.consume_key(Modifiers::COMMAND, Key::S),
                close: input.consume_key(Modifiers::COMMAND, Key::W),
                // ⌘V は egui がクリップボードのテキストを優先して消費するため、
                // 画像を直接読みたいときは ⌘⇧V を使う。
                paste_image: input.consume_key(command_shift(), Key::V),
                clear: input.consume_key(Modifiers::COMMAND, Key::Backspace),
                redo: input.consume_key(command_shift(), Key::Z),
                undo: input.consume_key(Modifiers::COMMAND, Key::Z),
                toggle_top: input.consume_key(command_shift(), Key::T),
                ..Default::default()
            };

            for tool in Tool::ALL {
                if input.consume_key(Modifiers::NONE, tool.key()) {
                    pressed.tool = Some(tool);
                }
            }
            for (index, key) in COLOR_KEYS.iter().enumerate() {
                if input.consume_key(Modifiers::NONE, *key) {
                    pressed.color = Some(index);
                }
            }
            if input.consume_key(Modifiers::NONE, THINNER_KEY) {
                pressed.width_delta -= 1;
            }
            if input.consume_key(Modifiers::NONE, THICKER_KEY) {
                pressed.width_delta += 1;
            }

            pressed
        })
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // テキスト編集中はキーを入力欄に譲る（⌘C/⌘V も同様）。
        if ctx.egui_wants_keyboard_input() {
            return;
        }

        let pressed = Self::read_keys(ctx);

        if pressed.close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if pressed.copy {
            self.pending_export = Some(ExportKind::Copy);
        }
        if pressed.save {
            self.pending_export = Some(ExportKind::Save);
        }
        if pressed.paste_image {
            self.apply(Action::PasteImage);
        }
        if pressed.undo {
            self.apply(Action::Undo);
        }
        if pressed.redo {
            self.apply(Action::Redo);
        }
        if pressed.clear {
            self.apply(Action::Clear);
        }
        if let Some(tool) = pressed.tool {
            self.apply(Action::Tool(tool));
        }
        if let Some(color) = pressed.color {
            self.apply(Action::Color(color));
        }
        if pressed.width_delta != 0 {
            let next =
                (self.width_index as i32 + pressed.width_delta).clamp(0, WIDTHS.len() as i32 - 1);
            self.apply(Action::Width(next as usize));
        }
        if pressed.toggle_top {
            self.always_on_top = !self.always_on_top;
            let level = if self.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
            // ツールバーに状態表示が無いので、切り替えは通知で知らせる。
            self.set_status(
                if self.always_on_top {
                    "最前面表示: オン"
                } else {
                    "最前面表示: オフ"
                },
                false,
            );
        }
    }

    // ---- 背景画像の貼り付け ---------------------------------------------

    fn handle_paste(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }

        let pasted = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::PasteImage(image) => Some(PasteContent::Image(image.clone())),
                egui::Event::Paste(text) => Some(PasteContent::Text(text.clone())),
                _ => None,
            })
        });

        match pasted {
            Some(PasteContent::Image(image)) => self.apply_background(ctx, image),
            // Linux のファイルマネージャは画像データではなく `text/uri-list` を
            // テキストとして置くため、パスから読み直す。
            Some(PasteContent::Text(text)) => {
                if !self.paste_image_file(ctx, &text)
                    && should_try_clipboard_image(&text)
                    && let Ok(Some(image)) = read_clipboard_image()
                {
                    // ブラウザの「画像をコピー」は text/html と画像が同居するため、
                    // egui がテキストを優先してもクリップボードから画像を拾う。
                    self.apply_background(ctx, Arc::new(image));
                }
            }
            None => {}
        }
    }

    /// クリップボードのテキストが画像ファイルのパス（または URI）なら背景に貼る。
    /// 貼れたかどうかを返す。
    fn paste_image_file(&mut self, ctx: &egui::Context, text: &str) -> bool {
        let paths = pasted_file_paths(text);
        let mut last_error = None;
        for path in paths {
            match load_image_file(&path) {
                Ok(image) => {
                    self.apply_background(ctx, Arc::new(image));
                    return true;
                }
                Err(error) => last_error = Some(error),
            }
        }
        if let Some(error) = last_error {
            self.set_status(error, true);
        }
        false
    }

    /// ツールバーの「ペースト」ボタン・⌘⇧V の要求を処理する。
    fn flush_clipboard_paste(&mut self, ctx: &egui::Context) {
        if !std::mem::take(&mut self.paste_requested) {
            return;
        }
        match read_clipboard_image() {
            Ok(Some(image)) => self.apply_background(ctx, Arc::new(image)),
            Ok(None) => self.set_status("クリップボードに画像がありません", true),
            Err(error) => self.set_status(error, true),
        }
    }

    fn apply_background(&mut self, ctx: &egui::Context, image: Arc<ColorImage>) {
        let size_px = image.size;
        let ppp = ctx.pixels_per_point().max(0.1);
        let size_pt = Vec2::new(size_px[0] as f32 / ppp, size_px[1] as f32 / ppp);

        let texture = ctx.load_texture(
            "dood_background",
            (*image).clone(),
            egui::TextureOptions::LINEAR,
        );

        self.history.record(&self.scene);
        self.scene.bg = Some(Background {
            texture,
            size_px,
            size_pt,
        });

        // 画像の実寸にウィンドウを合わせる（等倍で注釈できる）。
        let mut target = size_pt;
        let mut note = String::new();
        if let Some(monitor) = ctx.input(|input| input.viewport().monitor_size) {
            let available = monitor - Vec2::new(60.0, 140.0);
            if target.x > available.x || target.y > available.y {
                let scale = (available.x / target.x).min(available.y / target.y);
                target *= scale;
                note = "画面に収まらないため縮小表示".to_owned();
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target));
        let text = format!("背景を貼り付けました {}×{}px", size_px[0], size_px[1]);
        if note.is_empty() {
            self.set_status(text, false);
        } else {
            self.set_status_with_detail(text, note, false);
        }
    }

    // ---- ツールバー (Material Design 3 Docked Toolbar) ------------------

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let mut actions = Vec::new();

        // 幅に応じてグループ単位で折り返す。狭いウィンドウでも全ボタンが画面内に残る。
        // 1段目の右端は閉じるボタン用に空けておく。
        let available = ui.available_width();
        let group_widths = Self::toolbar_group_widths();
        let export_width = self.export_width(ui);
        let (rows, export_row) =
            plan_toolbar(&group_widths, export_width, CLOSE_RESERVE, available);

        let row_count = rows.len();
        let bar_height = TOOLBAR_PAD_Y as f32 * 2.0
            + row_count as f32 * theme::ROW_HEIGHT
            + row_count.saturating_sub(1) as f32 * TOOLBAR_ROW_GAP;

        // ウィンドウに枠が無いので、ツールバーの余白ドラッグでウィンドウを動かす。
        // 背景をボタンより先に確保し、ボタンのクリックを奪わないようにする。
        let (_, bar_rect) = ui.allocate_space(egui::vec2(available, bar_height));
        // 背景ドラッグはフレームをまたいで同じ ID で扱う。
        let drag = ui.interact(bar_rect, Id::unique("dood_toolbar_drag"), Sense::drag());
        ui.painter()
            .rect_filled(bar_rect, egui::CornerRadius::ZERO, theme::SURFACE_CONTAINER);
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        let inner = bar_rect.shrink2(egui::vec2(TOOLBAR_PAD_X as f32, TOOLBAR_PAD_Y as f32));
        let mut content = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        content.spacing_mut().item_spacing = Vec2::ZERO;

        for (row_index, row) in rows.iter().enumerate() {
            if row_index > 0 {
                content.add_space(TOOLBAR_ROW_GAP);
            }
            content.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (cell_index, &group_index) in row.iter().enumerate() {
                    if cell_index > 0 {
                        ui.add_space(TOOLBAR_GAP);
                    }
                    self.draw_toolbar_group(ui, TOOLBAR_GROUPS[group_index], &mut actions);
                }

                // 閉じる×は折り返し位置に関係なく1段目の右上端に固定する。
                let with_close = row_index == 0;
                let with_export = export_row == row_index;
                if with_close || with_export {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        if with_close
                            && theme::close_button(ui)
                                .on_hover_text("閉じる (⌘W)")
                                .clicked()
                        {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if with_export {
                            if with_close {
                                ui.add_space(CLOSE_GAP);
                            }
                            self.export_buttons(ui, &mut actions);
                        }
                    });
                }
            });
        }

        ui.painter().line_segment(
            [bar_rect.left_bottom(), bar_rect.right_bottom()],
            Stroke::new(1.0, theme::OUTLINE_VARIANT),
        );

        for action in actions {
            self.apply(action);
        }
    }

    /// 各グループの横幅。折り返し計算と実際の描画で同じ値を使う。
    fn toolbar_group_widths() -> [f32; 5] {
        [
            theme::tools_group_width(),
            palette_group_width(PALETTE.len()),
            theme::widths_group_width(),
            theme::history_group_width(),
            theme::clear_group_width(),
        ]
    }

    fn export_width(&self, ui: &egui::Ui) -> f32 {
        theme::plate_width(
            theme::ICON_BUTTON_SIZE * 2.0
                + theme::PLATE_ITEM_GAP * 2.0
                + theme::pill_button_width(ui, "保存"),
        )
    }

    fn draw_toolbar_group(
        &mut self,
        ui: &mut egui::Ui,
        group: ToolbarGroup,
        actions: &mut Vec<Action>,
    ) {
        match group {
            ToolbarGroup::Tools => {
                // アイコンのみ。選択中は secondaryContainer の角丸塗りで示す。
                theme::group_plate(
                    ui,
                    theme::tools_group_width(),
                    theme::SURFACE_CONTAINER_HIGH,
                    |ui| {
                        for tool in Tool::ALL {
                            let selected = self.tool == tool;
                            if theme::tool_button(ui, tool, selected)
                                .on_hover_text(format!("{} ({})", tool.label(), tool.key_label()))
                                .clicked()
                            {
                                actions.push(Action::Tool(tool));
                            }
                        }
                    },
                );
            }
            ToolbarGroup::Palette => {
                // 円形スウォッチを Surface Container High の角丸プレートにまとめる。
                theme::group_plate(
                    ui,
                    palette_group_width(PALETTE.len()),
                    theme::SURFACE_CONTAINER_HIGH,
                    |ui| {
                        for (index, (name, color)) in PALETTE.iter().enumerate() {
                            let selected = self.color == *color;
                            let tip = if *color == Color32::WHITE {
                                format!("{name} / 消しゴム ({})", index + 1)
                            } else {
                                format!("{name} ({})", index + 1)
                            };
                            if theme::color_swatch(ui, *color, selected)
                                .on_hover_text(tip)
                                .clicked()
                            {
                                actions.push(Action::Color(index));
                            }
                        }
                    },
                );
            }
            ToolbarGroup::Widths => {
                // 数値ラベルをやめ、実寸の横棒だけで太さを示す。
                theme::group_plate(
                    ui,
                    theme::widths_group_width(),
                    theme::SURFACE_CONTAINER_HIGH,
                    |ui| {
                        for (index, width) in WIDTHS.iter().enumerate() {
                            let tip = format!("{width:.0}pt");
                            if theme::width_button(ui, *width, self.width_index == index)
                                .on_hover_text(tip)
                                .clicked()
                            {
                                actions.push(Action::Width(index));
                            }
                        }
                    },
                );
            }
            ToolbarGroup::History => {
                theme::group_plate(
                    ui,
                    theme::history_group_width(),
                    theme::SURFACE_CONTAINER_HIGH,
                    |ui| {
                        if theme::flat_icon_button(ui, ActionIcon::Undo, self.history.can_undo())
                            .on_hover_text("元に戻す (⌘Z)")
                            .clicked()
                        {
                            actions.push(Action::Undo);
                        }
                        if theme::flat_icon_button(ui, ActionIcon::Redo, self.history.can_redo())
                            .on_hover_text("やり直す (⇧⌘Z)")
                            .clicked()
                        {
                            actions.push(Action::Redo);
                        }
                    },
                );
            }
            ToolbarGroup::Clear => {
                let can_clear = !self.scene.items.is_empty() || self.scene.bg.is_some();
                theme::group_plate(
                    ui,
                    theme::clear_group_width(),
                    theme::SURFACE_CONTAINER_HIGH,
                    |ui| {
                        if theme::clear_button(ui, can_clear)
                            .on_hover_text("すべて消去 (⌘⌫)")
                            .clicked()
                        {
                            actions.push(Action::Clear);
                        }
                    },
                );
            }
        }
    }

    /// 右寄せの書き出しグループ。プレート内は左から ペースト・コピー・保存。
    fn export_buttons(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let width = self.export_width(ui);
        let save_tip = format!(
            "PNG を保存してパスをコピー (⌘S)\n保存先: {}",
            self.out_path.display()
        );

        theme::group_plate(ui, width, theme::SURFACE_CONTAINER_HIGH, |ui| {
            if theme::flat_icon_button(ui, ActionIcon::Paste, true)
                .on_hover_text("クリップボードから画像を貼り付ける (⌘⇧V)")
                .clicked()
            {
                actions.push(Action::PasteImage);
            }

            if theme::flat_icon_button(ui, ActionIcon::Copy, true)
                .on_hover_text("キャンバスを PNG でクリップボードへコピー (⌘C)")
                .clicked()
            {
                actions.push(Action::Export(ExportKind::Copy));
            }

            if theme::save_button(ui, "保存")
                .on_hover_text(save_tip)
                .clicked()
            {
                actions.push(Action::Export(ExportKind::Save));
            }
        });
    }

    /// 画面右上（ツールバーの下）に通知を一時表示する。
    fn notification(&mut self, ctx: &egui::Context, canvas: Rect) {
        // 書き出しのスクリーンショットに通知を写り込ませない。
        if self.capture_next_frame {
            return;
        }

        let now = Instant::now();
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some((text, error)) = status
            .visible(now)
            .map(|(text, error)| (text.to_owned(), error))
        else {
            return;
        };
        let detail = status.detail.clone();
        let remaining = status.time_until_expiry(now);
        drop(status);

        // マウスを動かさなくても表示期限で消えるように再描画を予約する。
        if let Some(remaining) = remaining {
            ctx.request_repaint_after(remaining);
        }

        let max_width = TOAST_MAX_WIDTH.min(canvas.width() - TOAST_MARGIN * 2.0);
        if max_width < TOAST_MIN_WIDTH {
            return;
        }

        let (fill, fg, detail_fg, accent) = if error {
            (
                theme::ERROR,
                theme::ON_ERROR,
                theme::ON_ERROR.gamma_multiply(0.85),
                theme::ON_ERROR,
            )
        } else {
            (
                theme::INVERSE_SURFACE,
                theme::INVERSE_ON_SURFACE,
                theme::INVERSE_ON_SURFACE.gamma_multiply(0.8),
                theme::INVERSE_PRIMARY,
            )
        };

        egui::Area::new(Id::unique("dood_notification"))
            .order(egui::Order::Foreground)
            .fixed_pos(toast_anchor(canvas))
            .pivot(egui::Align2::RIGHT_TOP)
            // キャンバスへのクリックは通知を貫通させる。
            .interactable(false)
            .default_width(max_width)
            .show(ctx, |ui| {
                // 幅は文字の実寸から決める。Area の前回サイズに左右されず、
                // 長い保存先だけが折り返される。
                let headline = ui.painter().layout_no_wrap(
                    text.clone(),
                    egui::FontId::proportional(TOAST_HEADLINE_SIZE),
                    fg,
                );
                let detail_galley = detail.as_ref().map(|detail| {
                    ui.painter().layout_no_wrap(
                        detail.clone(),
                        egui::FontId::proportional(TOAST_DETAIL_SIZE),
                        detail_fg,
                    )
                });
                let text_max =
                    (max_width - TOAST_PAD_X * 2.0 - TOAST_ICON_SIZE - TOAST_ICON_GAP).max(40.0);
                let text_width = headline
                    .size()
                    .x
                    .max(detail_galley.as_ref().map_or(0.0, |galley| galley.size().x))
                    .min(text_max);
                let content_width = (text_width + TOAST_ICON_SIZE + TOAST_ICON_GAP)
                    .max(TOAST_MIN_WIDTH - TOAST_PAD_X * 2.0);

                egui::Frame::new()
                    .fill(fill)
                    .corner_radius(egui::CornerRadius::same(TOAST_RADIUS))
                    .inner_margin(egui::Margin::symmetric(
                        TOAST_PAD_X as i8,
                        TOAST_PAD_Y as i8,
                    ))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 3],
                        blur: 10,
                        spread: 0,
                        color: Color32::from_black_alpha(36),
                    })
                    .show(ui, |ui| {
                        ui.set_width(content_width);
                        ui.horizontal_top(|ui| {
                            // 幅の計算どおりにアイコンと本文を並べる（既定の item_spacing を足さない）。
                            ui.spacing_mut().item_spacing.x = 0.0;
                            let (icon_rect, _) = ui
                                .allocate_exact_size(Vec2::splat(TOAST_ICON_SIZE), Sense::hover());
                            theme::draw_notification_icon(ui.painter(), icon_rect, error, accent);
                            ui.add_space(TOAST_ICON_GAP);
                            ui.vertical(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(text.as_str())
                                            .size(TOAST_HEADLINE_SIZE)
                                            .color(fg),
                                    )
                                    .wrap(),
                                );
                                if let Some(detail) = &detail {
                                    ui.add_space(2.0);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(detail.as_str())
                                                .size(TOAST_DETAIL_SIZE)
                                                .color(detail_fg),
                                        )
                                        .wrap(),
                                    );
                                }
                            });
                        });
                    });
            });
    }

    /// 通知が表示中か。期限切れならここで消す。
    fn notification_visible(&mut self) -> bool {
        let now = Instant::now();
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.visible(now).is_some()
    }

    // ---- キャンバス -----------------------------------------------------

    fn canvas(&mut self, ui: &mut egui::Ui, area: Rect) {
        if area.width() < 40.0 || area.height() < 40.0 {
            return;
        }

        let (response, painter) = ui.allocate_painter(area.size(), Sense::click_and_drag());
        let rect = response.rect;
        self.canvas_rect = rect;

        render::render_scene(&painter, rect, &self.scene);
        if let Some(preview) = self.preview() {
            render::draw_item_on(&painter, rect, &preview);
        }
        painter.rect_stroke(
            rect,
            egui::CornerRadius::ZERO,
            Stroke::new(1.0, theme::OUTLINE_VARIANT),
            StrokeKind::Inside,
        );

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }

        self.pointer(&response, rect);
    }

    fn pointer(&mut self, response: &egui::Response, rect: Rect) {
        // テキスト編集中はキャンバスへの描き込みを止める（クリックは確定操作）。
        if self.text.is_some() {
            return;
        }

        let local = response
            .interact_pointer_pos()
            .map(|pos| Pos2::new(pos.x - rect.min.x, pos.y - rect.min.y));

        // ドラッグ開始はしきい値を超えた位置で通知されるため、始点は押下位置から取る。
        let origin = response.ctx.input(|input| input.pointer.press_origin());
        let start = origin
            .map(|pos| Pos2::new(pos.x - rect.min.x, pos.y - rect.min.y))
            .or(local);

        if response.drag_started()
            && let Some(at) = start
        {
            self.draft = match self.tool {
                Tool::Pen => Some(Draft::Freehand { points: vec![at] }),
                Tool::Text => Some(Draft::TextBox {
                    start: at,
                    current: at,
                }),
                _ => Some(Draft::Shape {
                    start: at,
                    current: at,
                }),
            };
        }

        if response.dragged()
            && let (Some(at), Some(draft)) = (local, self.draft.as_mut())
        {
            match draft {
                Draft::Freehand { points } => {
                    if points.last().is_none_or(|last| last.distance(at) >= 1.0) {
                        points.push(at);
                    }
                }
                Draft::Shape { current, .. } | Draft::TextBox { current, .. } => *current = at,
            }
        }

        if response.drag_stopped()
            && let Some(draft) = self.draft.take()
        {
            match draft {
                Draft::TextBox { start, current } => {
                    if let Some(box_rect) = text_box_rect(start, current) {
                        self.open_text_box(box_rect);
                    } else {
                        // 小さすぎるドラッグはクリックと同じ扱い。
                        self.open_text(start);
                    }
                }
                draft => {
                    if let Some(item) = self.item_from_draft(draft) {
                        self.history.record(&self.scene);
                        self.scene.push(item);
                    }
                }
            }
        }

        if response.clicked()
            && let Some(at) = local
        {
            match self.tool {
                Tool::Text => self.open_text(at),
                Tool::Pen => {
                    self.history.record(&self.scene);
                    self.scene.push(Item::Stroke {
                        points: vec![at],
                        color: self.color,
                        width: self.width(),
                    });
                }
                _ => {}
            }
        }
    }

    fn item_from_draft(&self, draft: Draft) -> Option<Item> {
        match draft {
            Draft::Freehand { points } => Some(Item::Stroke {
                points,
                color: self.color,
                width: self.width(),
            }),
            Draft::Shape { start, current } => self.shape_item(start, current),
            Draft::TextBox { .. } => None,
        }
    }

    fn shape_item(&self, start: Pos2, current: Pos2) -> Option<Item> {
        let color = self.color;
        let width = self.width();
        match self.tool {
            Tool::Line => Some(Item::Line {
                a: start,
                b: current,
                color,
                width,
            }),
            Tool::Arrow => Some(Item::Arrow {
                a: start,
                b: current,
                color,
                width,
            }),
            Tool::Rect => {
                let rect = Rect::from_two_pos(start, current);
                (rect.width() >= 2.0 && rect.height() >= 2.0).then_some(Item::Rect {
                    a: start,
                    b: current,
                    color,
                    width,
                })
            }
            Tool::Ellipse => {
                let rect = Rect::from_two_pos(start, current);
                (rect.width() >= 2.0 && rect.height() >= 2.0).then_some(Item::Ellipse {
                    center: rect.center(),
                    radius: rect.size() / 2.0,
                    color,
                    width,
                })
            }
            Tool::Pen | Tool::Text => None,
        }
    }

    fn preview(&self) -> Option<Item> {
        match self.draft.as_ref()? {
            Draft::Freehand { points } => Some(Item::Stroke {
                points: points.clone(),
                color: self.color,
                width: self.width(),
            }),
            Draft::Shape { start, current } => self.shape_item(*start, *current),
            Draft::TextBox { start, current } => {
                let rect = Rect::from_two_pos(*start, *current);
                (rect.width() >= 2.0 && rect.height() >= 2.0).then_some(Item::Rect {
                    a: *start,
                    b: *current,
                    color: self.color.gamma_multiply(0.55),
                    width: 1.0,
                })
            }
        }
    }

    // ---- テキスト入力 ---------------------------------------------------

    /// クリック位置に既定幅の入力欄を開く。
    fn open_text(&mut self, at: Pos2) {
        self.draft = None;
        self.text = Some(TextDraft {
            rect: Rect::from_min_size(at, Vec2::new(TEXT_EDIT_WIDTH, TEXT_EDIT_MIN_HEIGHT)),
            buffer: String::new(),
            focused: false,
            color: self.color,
            size: self.text_size(),
            fixed_box: false,
        });
    }

    /// ドラッグで指定した矩形いっぱいの入力欄を開く。
    fn open_text_box(&mut self, rect: Rect) {
        self.draft = None;
        // `rect` はキャンバス左上原点、`canvas_rect` は画面座標。同じ座標系で切り取る。
        let canvas = self.canvas_rect;
        let rect = if canvas.is_positive() {
            rect.intersect(Rect::from_min_size(Pos2::ZERO, canvas.size()))
        } else {
            rect
        };
        if !rect.is_positive() {
            return;
        }
        self.text = Some(TextDraft {
            rect,
            buffer: String::new(),
            focused: false,
            color: self.color,
            size: self.text_size(),
            fixed_box: true,
        });
    }

    fn text_editor(&mut self, ui: &mut egui::Ui) {
        let Some(mut draft) = self.text.take() else {
            return;
        };
        let canvas = self.canvas_rect;
        if !canvas.is_positive() {
            self.text = Some(draft);
            return;
        }

        #[allow(deprecated)]
        let id = Id::new("dood_text_input");

        let width = draft
            .rect
            .width()
            .min((canvas.width() - 4.0).max(24.0))
            .max(24.0);
        let height = if draft.fixed_box {
            draft.rect.height().max(TEXT_EDIT_MIN_HEIGHT)
        } else {
            text_editor_height(ui, &draft, width)
        };
        let rect = place_text_editor(canvas, draft.rect.min, width, height);
        // 選択した矩形いっぱいに見えるよう、行数から高さを合わせる。
        let rows = if draft.fixed_box {
            ((height / (draft.size * 1.25)).floor() as usize).max(1)
        } else {
            1
        };

        let response = ui.put(
            rect,
            TextEdit::multiline(&mut draft.buffer)
                .id(id)
                .desired_width(width)
                .desired_rows(rows)
                .return_key(Some(egui::KeyboardShortcut::new(
                    Modifiers::SHIFT,
                    Key::Enter,
                )))
                .font(egui::FontId::proportional(draft.size))
                .text_color(draft.color),
        );

        // フォーカス要求は開いた瞬間の1回だけ（毎フレーム要求すると入力状態が壊れる）。
        if !draft.focused {
            response.request_focus();
            draft.focused = true;
            self.text = Some(draft);
            return;
        }

        // Shift は Enter イベント自身から読む（同じフレームで修飾キーが離されても取りこぼさない）。
        let (enter, enter_shift, escape) = ui.input(|input| {
            let mut enter = false;
            let mut enter_shift = false;

            for event in &input.events {
                if let egui::Event::Key {
                    key: Key::Enter,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    enter = true;
                    enter_shift |= modifiers.shift;
                }
            }

            (enter, enter_shift, input.key_pressed(Key::Escape))
        });

        if escape {
            return;
        }

        if commit_on_enter(enter, enter_shift) || response.lost_focus() {
            self.commit_text(draft);
            return;
        }

        self.text = Some(draft);
    }

    fn commit_text(&mut self, draft: TextDraft) {
        let text = draft.buffer.trim_end().to_owned();
        if text.trim().is_empty() {
            return;
        }
        self.history.record(&self.scene);
        self.scene.push(Item::Text {
            pos: draft.rect.min,
            text,
            size: draft.size,
            color: draft.color,
            wrap_width: Some(text_wrap_width(draft.rect.width())),
        });
    }

    // ---- 書き出し -------------------------------------------------------

    fn flush_export(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.pending_export.take() else {
            self.capture_next_frame = false;
            return;
        };
        if self.busy.load(Ordering::SeqCst) {
            self.capture_next_frame = false;
            self.pending_export = Some(kind);
            return;
        }
        if !self.canvas_rect.is_positive() {
            self.capture_next_frame = false;
            self.set_status("キャンバスがまだ描画されていません", true);
            return;
        }

        // 表示中の通知が出力 PNG に写り込まないよう、通知を描かない次のフレームで撮る。
        if should_defer_capture(self.capture_next_frame, self.notification_visible()) {
            self.capture_next_frame = true;
            self.pending_export = Some(kind);
            ctx.request_repaint();
            return;
        }
        self.capture_next_frame = false;

        // キャンバスの枠線（1pt 内側に描いている）を写さないよう 1pt 内側を切り出す。
        let rect = self.canvas_rect.shrink(1.0);
        let ppp = ctx.pixels_per_point().max(0.1);
        let path = self.out_path.clone();
        let status = self.status.clone();
        let busy = self.busy.clone();
        let screenshot_ctx = ctx.clone();
        let ctx = ctx.clone();

        busy.store(true, Ordering::SeqCst);
        screenshot_ctx.request_screenshot(move |shot: Arc<ColorImage>| {
            match export::crop(&shot, rect, ppp) {
                Ok(cropped) => {
                    let (width, height) = (cropped.size[0], cropped.size[1]);
                    match kind {
                        ExportKind::Copy => {
                            ctx.copy_image(cropped);
                            set_shared(
                                &status,
                                format!("コピーしました（{width}×{height}px）"),
                                None,
                                false,
                            );
                        }
                        ExportKind::Save => match export::save_png(&cropped, &path) {
                            Ok(()) => {
                                ctx.copy_text(path.display().to_string());
                                set_shared(
                                    &status,
                                    format!("保存しました（{width}×{height}px）"),
                                    Some(path.display().to_string()),
                                    false,
                                );
                            }
                            Err(error) => set_shared(
                                &status,
                                error,
                                Some(format!("保存先: {}", path.display())),
                                true,
                            ),
                        },
                    }
                }
                Err(error) => set_shared(&status, error, None, true),
            }
            busy.store(false, Ordering::SeqCst);
            ctx.request_repaint();
        });
    }

    pub fn render_ui(&mut self, ui: &mut egui::Ui) {
        self.handle_shortcuts(ui.ctx());
        self.handle_paste(ui.ctx());

        ui.spacing_mut().item_spacing.y = 0.0;
        self.toolbar(ui);
        self.flush_clipboard_paste(ui.ctx());
        let area = ui.available_rect_before_wrap();
        self.canvas(ui, area);
        self.text_editor(ui);
        self.notification(ui.ctx(), area);
        self.flush_export(ui.ctx());
    }
}

/// 入力中の複数行テキストに合わせた入力欄の高さ。
fn text_editor_height(ui: &egui::Ui, draft: &TextDraft, width: f32) -> f32 {
    let galley = ui.fonts_mut(|fonts| {
        fonts.layout_job(egui::text::LayoutJob::simple(
            draft.buffer.clone(),
            egui::FontId::proportional(draft.size),
            draft.color,
            (width - TEXT_EDIT_PADDING).max(24.0),
        ))
    });
    (galley.size().y + TEXT_EDIT_PADDING).max(TEXT_EDIT_MIN_HEIGHT)
}

/// 入力欄をキャンバス内のクリック位置に置く。はみ出す分は内側へ寄せる。
fn place_text_editor(canvas: Rect, pos: Pos2, width: f32, height: f32) -> Rect {
    let width = width.min(canvas.width()).max(24.0);
    let height = height.min(canvas.height()).max(24.0);
    let max_x = (canvas.max.x - width).max(canvas.min.x);
    let max_y = (canvas.max.y - height).max(canvas.min.y);
    let x = (canvas.min.x + pos.x).clamp(canvas.min.x, max_x);
    let y = (canvas.min.y + pos.y).clamp(canvas.min.y, max_y);
    Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height))
}

/// ドラッグの矩形がテキストボックスとして使える大きさなら返す。
fn text_box_rect(start: Pos2, current: Pos2) -> Option<Rect> {
    let rect = Rect::from_two_pos(start, current);
    (rect.width() >= TEXT_BOX_MIN_WIDTH && rect.height() >= TEXT_BOX_MIN_HEIGHT).then_some(rect)
}

/// 確定後のテキストを折り返す幅。入力欄の内側余白ぶんを差し引く。
fn text_wrap_width(box_width: f32) -> f32 {
    (box_width - TEXT_EDIT_PADDING).max(24.0)
}

enum PasteContent {
    Image(Arc<ColorImage>),
    Text(String),
}

/// 貼り付けられたテキストから、実在するファイルのパスを取り出す。
///
/// Linux のファイルマネージャはコピーしたファイルを `text/uri-list`
/// （`file:///path/to/image.png` を改行区切り）として置く。素のパスを返す
/// アプリもあるため、どちらも受け付ける。
fn pasted_file_paths(text: &str) -> Vec<PathBuf> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(parse_pasted_path)
        .collect()
}

fn parse_pasted_path(line: &str) -> Option<PathBuf> {
    let path = match line.strip_prefix("file://") {
        Some(rest) => {
            let rest = rest.strip_prefix("localhost").unwrap_or(rest);
            // Windows の file:///C:/... は先頭の / を落とす。
            let rest = match rest.as_bytes() {
                [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => &rest[1..],
                _ => rest,
            };
            PathBuf::from(percent_decode(rest))
        }
        None => PathBuf::from(line),
    };
    path.is_file().then_some(path)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(index + 1).and_then(|byte| hex_digit(*byte)),
                bytes.get(index + 2).and_then(|byte| hex_digit(*byte)),
            )
        {
            out.push(high * 16 + low);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// テキスト貼り付けの裏に画像がありそうか。
///
/// ブラウザの「画像をコピー」は HTML と画像を同時に置く。素のテキストや
/// ファイルパスでは、クリップボードを二重に読まない。
fn should_try_clipboard_image(text: &str) -> bool {
    text.trim_start().starts_with('<')
}

/// クリップボードを直接読み、画像（または画像ファイル）を取り出す。
///
/// egui の `PasteImage` はクリップボードにテキストがあると発火しないため、
/// ツールバーの「ペースト」と ⌘⇧V ではこの経路を使う。
fn read_clipboard_image() -> Result<Option<ColorImage>, String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|error| format!("クリップボードを開けません: {error}"))?;

    match clipboard.get().image() {
        Ok(image) => return Ok(Some(color_image_from_arboard(image))),
        Err(error) => eprintln!("dood: clipboard image: {error}"),
    }

    let mut last_error = None;
    match clipboard.get().file_list() {
        Ok(paths) => {
            eprintln!("dood: clipboard file list: {} paths", paths.len());
            for path in paths {
                match load_image_file(&path) {
                    Ok(image) => return Ok(Some(image)),
                    Err(error) => last_error = Some(error),
                }
            }
        }
        Err(error) => eprintln!("dood: clipboard file list: {error}"),
    }
    match clipboard.get().text() {
        Ok(text) => {
            let paths = pasted_file_paths(&text);
            eprintln!("dood: clipboard text: {} file paths", paths.len());
            for path in paths {
                match load_image_file(&path) {
                    Ok(image) => return Ok(Some(image)),
                    Err(error) => last_error = Some(error),
                }
            }
        }
        Err(error) => eprintln!("dood: clipboard text: {error}"),
    }

    if let Some(error) = last_error {
        return Err(error);
    }
    Ok(None)
}

fn color_image_from_arboard(image: arboard::ImageData<'_>) -> ColorImage {
    ColorImage::from_rgba_unmultiplied([image.width, image.height], image.bytes.as_ref())
}

/// 画像ファイルを読み込んで egui の画像にする。
fn load_image_file(path: &Path) -> Result<ColorImage, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("画像ファイルを読めません: {} ({error})", path.display()))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|error| format!("画像ファイルを読めません: {} ({error})", path.display()))?;
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

fn command_shift() -> Modifiers {
    Modifiers {
        command: true,
        shift: true,
        ..Modifiers::NONE
    }
}

impl eframe::App for DoodApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [1.0, 1.0, 1.0, 1.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render_ui(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// フォント読み込みを避けたテスト用のアプリ。
    fn test_app() -> DoodApp {
        DoodApp {
            scene: Scene::default(),
            history: History::default(),
            tool: Tool::Pen,
            color: PALETTE[0].1,
            width_index: 1,
            draft: None,
            text: None,
            out_path: PathBuf::from("dood.png"),
            status: Arc::new(Mutex::new(Status::default())),
            busy: Arc::new(AtomicBool::new(false)),
            pending_export: None,
            paste_requested: false,
            capture_next_frame: false,
            always_on_top: true,
            canvas_rect: Rect::NOTHING,
        }
    }

    fn key_event(key: Key) -> egui::Event {
        key_event_with_modifiers(key, Modifiers::NONE)
    }

    fn key_event_with_modifiers(key: Key, modifiers: Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn run_with_events(
        app: &mut DoodApp,
        events: Vec<egui::Event>,
        mut f: impl FnMut(&mut DoodApp, &egui::Context),
    ) {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        let mut output = ctx.run_pass(input, |ctx| f(app, ctx));
        // テクスチャ差分を受け取るレンダラが居ないテストでは明示的に捨てる。
        output.textures_delta.clear();
    }

    // ---- Status -----------------------------------------------------

    #[test]
    fn status_message_expires_after_success_ttl() {
        let mut status = Status::default();
        let start = Instant::now();
        status.set("保存しました", false, start);

        assert!(status.visible(start).is_some());
        assert!(
            status
                .visible(start + Status::SUCCESS_TTL - Duration::from_millis(1))
                .is_some()
        );
        assert!(status.visible(start + Status::SUCCESS_TTL).is_none());
        assert!(status.text.is_empty());
        assert_eq!(status.time_until_expiry(start), None);
    }

    #[test]
    fn error_status_is_kept_longer_than_success() {
        let mut status = Status::default();
        let start = Instant::now();
        status.set("保存に失敗しました", true, start);

        assert!(status.visible(start + Status::SUCCESS_TTL).is_some());
        assert!(status.visible(start + Status::ERROR_TTL).is_none());
    }

    #[test]
    fn status_time_until_expiry_counts_down() {
        let mut status = Status::default();
        let start = Instant::now();
        status.set("コピーしました", false, start);

        assert_eq!(status.time_until_expiry(start), Some(Status::SUCCESS_TTL),);
        assert_eq!(
            status.time_until_expiry(start + Duration::from_secs(1)),
            Some(Status::SUCCESS_TTL - Duration::from_secs(1)),
        );
    }

    #[test]
    fn status_detail_is_shown_and_cleared_with_the_message() {
        let mut status = Status::default();
        let start = Instant::now();
        status.set_with_detail(
            "保存しました",
            Some("/tmp/dood.png".to_owned()),
            false,
            start,
        );

        assert_eq!(status.detail.as_deref(), Some("/tmp/dood.png"));
        assert!(status.visible(start).is_some());
        assert!(status.visible(start + Status::SUCCESS_TTL).is_none());
        assert_eq!(status.detail, None);
    }

    // ---- 通知の配置 ---------------------------------------------------

    #[test]
    fn toast_sits_at_the_top_right_below_the_toolbar() {
        let canvas = Rect::from_min_size(Pos2::new(0.0, 60.0), Vec2::new(480.0, 300.0));
        let anchor = toast_anchor(canvas);

        assert_eq!(anchor, Pos2::new(480.0 - TOAST_MARGIN, 60.0 + TOAST_MARGIN));
        assert!(anchor.x > canvas.center().x);
        assert!(anchor.y >= canvas.min.y, "ツールバーに重なる");
    }

    #[test]
    fn capture_waits_for_a_frame_without_the_notification() {
        assert!(should_defer_capture(false, true));
        assert!(!should_defer_capture(false, false));
        // すでに通知を隠したフレームなら、そのまま撮る。
        assert!(!should_defer_capture(true, true));
    }

    // ---- ツールバーの折り返し ----------------------------------------

    /// 1100pt ウィンドウ（左右余白 8pt を引いた幅）。
    const WIDE_AVAILABLE: f32 = 1084.0;
    /// 480pt ウィンドウ（左右余白 8pt を引いた幅）。
    const NARROW_AVAILABLE: f32 = 464.0;
    /// 保存ボタンの実幅はフォント依存なので、テストでは控えめな固定値を使う。
    const SAMPLE_EXPORT_WIDTH: f32 = 152.0;

    fn sample_widths() -> [f32; 5] {
        [
            theme::tools_group_width(),
            palette_group_width(PALETTE.len()),
            theme::widths_group_width(),
            theme::history_group_width(),
            theme::clear_group_width(),
        ]
    }

    #[test]
    fn wide_toolbar_fits_on_one_row() {
        let widths = sample_widths();
        let (rows, export_row) =
            plan_toolbar(&widths, SAMPLE_EXPORT_WIDTH, CLOSE_RESERVE, WIDE_AVAILABLE);

        assert_eq!(rows, vec![vec![0, 1, 2, 3, 4]]);
        assert_eq!(export_row, 0);
        // 閉じる×・書き出し・全グループが重ならずに1段へ収まる。
        assert!(
            row_width(&rows[0], &widths) + EXPORT_GAP + SAMPLE_EXPORT_WIDTH + CLOSE_RESERVE
                <= WIDE_AVAILABLE + 0.01,
            "1段に収まらない: {rows:?}"
        );
    }

    #[test]
    fn narrow_toolbar_wraps_to_exactly_two_rows() {
        let widths = sample_widths();
        let (rows, export_row) = plan_toolbar(
            &widths,
            SAMPLE_EXPORT_WIDTH,
            CLOSE_RESERVE,
            NARROW_AVAILABLE,
        );

        assert_eq!(rows, vec![vec![0, 1], vec![2, 3, 4]]);
        assert_eq!(export_row, 1);
        for (index, row) in rows.iter().enumerate() {
            let limit = if index == 0 {
                NARROW_AVAILABLE - CLOSE_RESERVE
            } else {
                NARROW_AVAILABLE
            };
            assert!(
                row_width(row, &widths) <= limit + 0.01,
                "行がはみ出している: {row:?}"
            );
        }
        assert!(
            row_width(&rows[export_row], &widths) + EXPORT_GAP + SAMPLE_EXPORT_WIDTH
                <= NARROW_AVAILABLE + 0.01
        );
    }

    #[test]
    fn first_row_reserves_room_for_the_close_button() {
        // 460pt なら閉じる×を予約しなければ1段に収まる幅。
        let widths = [390.0, 30.0];
        let (rows, export_row) = plan_toolbar(&widths, 10.0, CLOSE_RESERVE, 460.0);

        assert_eq!(rows, vec![vec![0], vec![1]], "閉じる×の幅を予約していない");
        assert_eq!(export_row, 1);
        assert!(row_width(&rows[0], &widths) + CLOSE_RESERVE <= 460.0 + 0.01);
        assert!(row_width(&rows[1], &widths) + EXPORT_GAP + 10.0 <= 460.0 + 0.01);
    }

    #[test]
    fn export_gets_its_own_row_when_it_does_not_fit() {
        let widths = [400.0];
        let (rows, export_row) = plan_toolbar(&widths, 150.0, CLOSE_RESERVE, 460.0);
        assert_eq!(rows, vec![vec![0], vec![]]);
        assert_eq!(export_row, 1);
    }

    #[test]
    fn palette_width_matches_swatch_layout() {
        // 左右余白 4*2 + スウォッチ 24*6 + 間隔 2*5
        assert_eq!(palette_group_width(6), 8.0 + 144.0 + 10.0);
    }

    #[test]
    fn toolbar_heights_match_the_design_targets() {
        let one_row = TOOLBAR_PAD_Y as f32 * 2.0 + theme::ROW_HEIGHT;
        let two_rows = one_row + TOOLBAR_ROW_GAP + theme::ROW_HEIGHT;

        assert!(
            (44.0..=48.0).contains(&one_row),
            "1段の高さが目標外: {one_row}"
        );
        assert!(
            (80.0..=84.0).contains(&two_rows),
            "2段の高さが目標外: {two_rows}"
        );
    }

    // ---- テキスト入力 ------------------------------------------------

    #[test]
    fn enter_commits_unless_shift_is_held() {
        assert!(commit_on_enter(true, false));
        // Shift+Enter は改行
        assert!(!commit_on_enter(true, true));
        assert!(!commit_on_enter(false, false));
    }

    #[test]
    fn text_editor_stays_inside_canvas() {
        let canvas = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(300.0, 200.0));
        let rect = place_text_editor(canvas, Pos2::new(290.0, 190.0), 260.0, 30.0);

        assert!(
            canvas.contains_rect(rect),
            "入力欄がはみ出している: {rect:?}"
        );
        assert!((rect.min.x - 50.0).abs() < 0.01, "右端で内側に寄っていない");
        assert!(
            (rect.min.y - 190.0).abs() < 0.01,
            "下端で内側に寄っていない"
        );
    }

    // ---- ショートカットとテキスト編集 ---------------------------------

    #[test]
    fn single_key_shortcut_switches_tool_normally() {
        let mut app = test_app();
        run_with_events(&mut app, vec![key_event(Key::A)], |app, ctx| {
            app.handle_shortcuts(ctx);
        });

        assert_eq!(app.tool, Tool::Arrow);
    }

    #[test]
    fn command_w_requests_closing_the_window() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let mut output = ctx.run_pass(
            egui::RawInput {
                events: vec![key_event_with_modifiers(Key::W, Modifiers::COMMAND)],
                ..Default::default()
            },
            |ctx| app.handle_shortcuts(ctx),
        );
        // テクスチャ差分を受け取るレンダラが居ないテストでは明示的に捨てる。
        output.textures_delta.clear();

        let viewport = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .expect("root viewport が無い");
        assert!(
            viewport.commands.contains(&egui::ViewportCommand::Close),
            "⌘W がウィンドウを閉じない: {:?}",
            viewport.commands
        );
    }

    fn ui_frame(
        app: &mut DoodApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 800.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.render_ui(ui));
        // テクスチャ差分を受け取るレンダラが居ないテストでは明示的に捨てる。
        output.textures_delta.clear();
        output
    }

    fn viewport_commands(output: &egui::FullOutput) -> Vec<egui::ViewportCommand> {
        output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .expect("root viewport が無い")
            .commands
            .clone()
    }

    /// `pos` を1回クリックし、その間に出た viewport コマンドを返す。
    fn click_at(app: &mut DoodApp, ctx: &egui::Context, pos: Pos2) -> Vec<egui::ViewportCommand> {
        let _ = ui_frame(app, ctx, vec![egui::Event::PointerMoved(pos)]);
        let press = ui_frame(
            app,
            ctx,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
        );
        let release = ui_frame(
            app,
            ctx,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        [press, release]
            .iter()
            .flat_map(viewport_commands)
            .collect()
    }

    #[test]
    fn toolbar_clicks_select_a_tool_and_clear() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let _ = ui_frame(&mut app, &ctx, Vec::new());
        let _ = ui_frame(&mut app, &ctx, Vec::new());

        // ツールプレート内の3番目（矢印）をクリックする。
        let _ = click_at(&mut app, &ctx, Pos2::new(110.0, 22.0));
        assert_eq!(app.tool, Tool::Arrow, "ツールアイコンが選択できない");

        // 空のシーンでは消去が無効（履歴に空の状態を積まない）。
        let _ = click_at(&mut app, &ctx, Pos2::new(642.0, 22.0));
        assert!(
            !app.history.can_undo(),
            "空のシーンで消去が有効になっている"
        );

        app.push_item(Item::Line {
            a: Pos2::ZERO,
            b: Pos2::new(10.0, 10.0),
            color: Color32::BLACK,
            width: 2.0,
        });
        let _ = click_at(&mut app, &ctx, Pos2::new(642.0, 22.0));
        assert!(app.scene.items.is_empty(), "消去ボタンが効かない");
    }

    #[test]
    fn clicking_the_close_button_requests_closing_the_window() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let _ = ui_frame(&mut app, &ctx, Vec::new());
        let _ = ui_frame(&mut app, &ctx, Vec::new());

        // 1段目の右上端（右余白8pt + ボタン32ptの中心）。
        let commands = click_at(&mut app, &ctx, Pos2::new(1076.0, 22.0));
        assert!(
            commands.contains(&egui::ViewportCommand::Close),
            "× ボタンがウィンドウを閉じない: {commands:?}"
        );
    }

    #[test]
    fn dragging_the_toolbar_background_starts_a_window_drag() {
        let mut app = test_app();
        let ctx = egui::Context::default();

        // ツールバーの矩形と、その上のドラッグ領域が登録されるまで2フレーム回す。
        let _ = ui_frame(&mut app, &ctx, Vec::new());
        let _ = ui_frame(&mut app, &ctx, Vec::new());

        // グループ間の余白（ボタンの無いところ）を押して動かす。
        let start = Pos2::new(800.0, 22.0);
        let press_output = ui_frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        let move_output = ui_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start + Vec2::new(30.0, 6.0))],
        );

        let commands: Vec<_> = [press_output, move_output]
            .iter()
            .flat_map(viewport_commands)
            .collect();
        assert!(
            commands.contains(&egui::ViewportCommand::StartDrag),
            "ツールバーのドラッグがウィンドウ移動にならない: {commands:?}"
        );
    }

    #[test]
    fn dragging_a_toolbar_button_does_not_move_the_window() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let _ = ui_frame(&mut app, &ctx, Vec::new());
        let _ = ui_frame(&mut app, &ctx, Vec::new());

        // 3番目のツール（矢印）の上から動かす。ボタンがドラッグを受け取る。
        let start = Pos2::new(110.0, 24.0);
        let outputs = [
            ui_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start)]),
            ui_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                }],
            ),
            ui_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start + Vec2::new(30.0, 6.0))],
            ),
        ];

        let commands: Vec<_> = outputs.iter().flat_map(viewport_commands).collect();
        assert!(
            !commands.contains(&egui::ViewportCommand::StartDrag),
            "ボタンからのドラッグでウィンドウが動く: {commands:?}"
        );
    }

    #[test]
    fn command_shift_v_requests_a_direct_clipboard_paste() {
        let mut app = test_app();
        run_with_events(
            &mut app,
            vec![key_event_with_modifiers(Key::V, command_shift())],
            |app, ctx| app.handle_shortcuts(ctx),
        );

        assert!(app.paste_requested, "⌘⇧V がペースト要求になっていない");
    }

    #[test]
    fn arboard_image_converts_to_egui_image() {
        let image = color_image_from_arboard(arboard::ImageData {
            width: 1,
            height: 1,
            bytes: std::borrow::Cow::Owned(vec![255, 0, 0, 255]),
        });
        assert_eq!(image.size, [1, 1]);
        assert_eq!(
            image.pixels[0],
            Color32::from_rgba_unmultiplied(255, 0, 0, 255)
        );
    }

    #[test]
    fn paste_image_works_when_not_editing() {
        let mut app = test_app();
        let image = ColorImage::new([2, 2], vec![Color32::RED; 4]);

        run_with_events(
            &mut app,
            vec![egui::Event::PasteImage(Arc::new(image))],
            |app, ctx| app.handle_paste(ctx),
        );

        assert!(app.scene.bg.is_some(), "背景画像が貼り付かない");
    }

    #[test]
    fn drag_must_be_large_enough_to_make_a_text_box() {
        assert!(text_box_rect(Pos2::new(10.0, 20.0), Pos2::new(50.0, 50.0)).is_some());
        assert!(text_box_rect(Pos2::new(10.0, 20.0), Pos2::new(49.0, 50.0)).is_none());
        assert!(text_box_rect(Pos2::new(10.0, 20.0), Pos2::new(50.0, 49.0)).is_none());
    }

    #[test]
    fn wrap_width_leaves_room_for_the_editor_padding() {
        assert_eq!(text_wrap_width(260.0), 260.0 - TEXT_EDIT_PADDING);
        assert_eq!(text_wrap_width(10.0), 24.0);
    }

    // ---- ファイル貼り付け ---------------------------------------------

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dood-paste-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn write_test_png(path: &Path) {
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        image.save(path).expect("write png");
    }

    #[test]
    fn pasted_uri_list_yields_existing_files() {
        let dir = temp_dir("uri-list");
        let image = dir.join("a b.png");
        write_test_png(&image);

        let uri = format!("file://{}", image.display()).replace(' ', "%20");
        let text = format!("# copied files\r\n{uri}\r\nfile:///missing.png\r\n");
        let paths = pasted_file_paths(&text);

        assert_eq!(paths, vec![image]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn percent_decode_handles_spaces_and_multibyte_characters() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("%E6%97%A5%E6%9C%AC"), "日本");
        // 不正な % はそのまま残す。
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn paste_image_file_sets_the_background() {
        let dir = temp_dir("paste-file");
        let image = dir.join("shot.png");
        write_test_png(&image);

        let mut app = test_app();
        let text = format!("file://{}", image.display());
        run_with_events(&mut app, vec![egui::Event::Paste(text)], |app, ctx| {
            app.handle_paste(ctx);
        });

        assert!(app.scene.bg.is_some(), "画像ファイルが背景に貼り付かない");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn html_paste_may_hide_an_image_but_plain_text_does_not() {
        assert!(should_try_clipboard_image(
            "<meta charset=\"utf-8\"><img src=...>"
        ));
        assert!(!should_try_clipboard_image("こんにちは"));
        assert!(!should_try_clipboard_image("file:///tmp/a.png"));
    }

    #[test]
    fn paste_plain_text_is_not_treated_as_an_image() {
        let mut app = test_app();
        run_with_events(
            &mut app,
            vec![egui::Event::Paste("こんにちは".to_owned())],
            |app, ctx| app.handle_paste(ctx),
        );

        assert!(app.scene.bg.is_none());
    }
}
