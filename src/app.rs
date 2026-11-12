//! アプリ本体：状態・入力・ツールバー・書き出しの調停。

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use eframe::egui::{
    self, Color32, ColorImage, Id, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit,
    Vec2,
};

use crate::{
    export, render,
    scene::{Background, History, Item, Scene},
    tool::{COLOR_KEYS, PALETTE, TEXT_SIZES, THICKER_KEY, THINNER_KEY, Tool, WIDTHS},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportKind {
    Copy,
    Save,
}

#[derive(Clone, Default)]
pub struct Status {
    pub text: String,
    pub error: bool,
}

fn set_shared(status: &Arc<Mutex<Status>>, text: impl Into<String>, error: bool) {
    if let Ok(mut guard) = status.lock() {
        guard.text = text.into();
        guard.error = error;
    }
}

/// ドラッグ中の下書き。確定前なので `Scene` には入れない。
enum Draft {
    Freehand { points: Vec<Pos2> },
    Shape { start: Pos2, current: Pos2 },
}

struct TextDraft {
    pos: Pos2,
    buffer: String,
    focused: bool,
    color: Color32,
    size: f32,
}

enum Action {
    Tool(Tool),
    Color(usize),
    Width(usize),
    Undo,
    Redo,
    Clear,
}

#[derive(Default)]
struct Pressed {
    copy: bool,
    save: bool,
    undo: bool,
    redo: bool,
    clear: bool,
    toggle_top: bool,
    tool: Option<Tool>,
    color: Option<usize>,
    width_delta: i32,
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
    always_on_top: bool,
    canvas_rect: Rect,
}

impl DoodApp {
    pub fn new(cc: &eframe::CreationContext<'_>, out_path: PathBuf) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        // 字形が無いときに panic せず豆腐で描く（和文フォントが引けない環境での保険）。
        cc.egui_ctx
            .set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
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
            always_on_top: true,
            canvas_rect: Rect::NOTHING,
        }
    }

    pub fn out_path(&self) -> &std::path::Path {
        &self.out_path
    }

    fn width(&self) -> f32 {
        WIDTHS[self.width_index]
    }

    fn text_size(&self) -> f32 {
        TEXT_SIZES[self.width_index]
    }

    fn set_status(&self, text: impl Into<String>, error: bool) {
        set_shared(&self.status, text, error);
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
        }
    }

    // ---- ショートカット -------------------------------------------------

    fn read_keys(ctx: &egui::Context) -> Pressed {
        ctx.input_mut(|input| {
            let mut pressed = Pressed::default();

            pressed.copy = input.consume_key(Modifiers::COMMAND, Key::C);
            pressed.save = input.consume_key(Modifiers::COMMAND, Key::S);
            pressed.clear = input.consume_key(Modifiers::COMMAND, Key::Backspace);
            pressed.redo = input.consume_key(command_shift(), Key::Z);
            pressed.undo = input.consume_key(Modifiers::COMMAND, Key::Z);
            pressed.toggle_top = input.consume_key(command_shift(), Key::T);

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

        if pressed.copy {
            self.pending_export = Some(ExportKind::Copy);
        }
        if pressed.save {
            self.pending_export = Some(ExportKind::Save);
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
            let next = (self.width_index as i32 + pressed.width_delta).clamp(0, WIDTHS.len() as i32 - 1);
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
        }
    }

    // ---- 背景画像の貼り付け ---------------------------------------------

    fn handle_paste(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let pasted = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::PasteImage(image) => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = pasted {
            self.apply_background(ctx, image);
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
                note = "（画面に収まらないため縮小表示）".to_owned();
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target));
        self.set_status(
            format!("背景を貼り付けました {}×{}px{note}", size_px[0], size_px[1]),
            false,
        );
    }

    // ---- ツールバー -----------------------------------------------------

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let mut actions = Vec::new();

        ui.horizontal(|ui| {
            ui.add_space(2.0);

            for tool in Tool::ALL {
                let selected = self.tool == tool;
                let label = format!("{} {}", tool.label(), tool.key_label());
                if ui.selectable_label(selected, label).clicked() {
                    actions.push(Action::Tool(tool));
                }
            }

            ui.separator();

            for (index, (name, color)) in PALETTE.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                let painter = ui.painter();
                painter.rect_filled(rect, egui::CornerRadius::same(3), *color);
                painter.rect_stroke(
                    rect,
                    egui::CornerRadius::same(3),
                    Stroke::new(1.0, Color32::from_gray(140)),
                    StrokeKind::Inside,
                );
                if self.color == *color {
                    painter.rect_stroke(
                        rect.shrink(2.0),
                        egui::CornerRadius::same(2),
                        Stroke::new(2.0, Color32::BLACK),
                        StrokeKind::Inside,
                    );
                }
                let response = response.on_hover_text(format!("{name} ({})", index + 1));
                if response.clicked() {
                    actions.push(Action::Color(index));
                }
            }

            ui.separator();

            for (index, width) in WIDTHS.iter().enumerate() {
                if ui
                    .selectable_label(self.width_index == index, format!("{width:.0}pt"))
                    .clicked()
                {
                    actions.push(Action::Width(index));
                }
            }

            ui.separator();

            if ui
                .add_enabled(self.history.can_undo(), egui::Button::new("戻す"))
                .clicked()
            {
                actions.push(Action::Undo);
            }
            if ui
                .add_enabled(self.history.can_redo(), egui::Button::new("やり直す"))
                .clicked()
            {
                actions.push(Action::Redo);
            }
            if ui.button("全消去").clicked() {
                actions.push(Action::Clear);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let status = self.status.lock().map(|s| s.clone()).unwrap_or_default();
                if !status.text.is_empty() {
                    let text = egui::RichText::new(&status.text);
                    ui.label(if status.error {
                        text.color(Color32::from_rgb(0xb0, 0x1c, 0x1c))
                    } else {
                        text
                    });
                }
                let file_name = self
                    .out_path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| "dood.png".to_owned());
                ui.label(format!("保存先: {file_name}"))
                    .on_hover_text(self.out_path.display().to_string());
            });
        });

        ui.separator();

        for action in actions {
            self.apply(action);
        }
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
            Stroke::new(1.0, Color32::from_gray(205)),
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

        if response.drag_started()
            && let Some(at) = local
        {
            self.draft = match self.tool {
                Tool::Pen => Some(Draft::Freehand { points: vec![at] }),
                Tool::Text => None,
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
                Draft::Shape { current, .. } => *current = at,
            }
        }

        if response.drag_stopped()
            && let Some(draft) = self.draft.take()
            && let Some(item) = self.item_from_draft(draft)
        {
            self.history.record(&self.scene);
            self.scene.push(item);
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
        }
    }

    // ---- テキスト入力 ---------------------------------------------------

    fn open_text(&mut self, at: Pos2) {
        self.draft = None;
        self.text = Some(TextDraft {
            pos: at,
            buffer: String::new(),
            focused: false,
            color: self.color,
            size: self.text_size(),
        });
    }

    fn text_editor(&mut self, ui: &mut egui::Ui) {
        let Some(mut draft) = self.text.take() else {
            return;
        };
        let canvas = self.canvas_rect;
        if !canvas.is_positive() {
            return;
        }

        let width = 220.0;
        let mut rect = Rect::from_min_size(canvas.min + draft.pos.to_vec2(), Vec2::new(width, 26.0));
        let overflow = rect.max - canvas.max;
        if overflow.x > 0.0 {
            rect = rect.translate(Vec2::new(-overflow.x, 0.0));
        }
        if overflow.y > 0.0 {
            rect = rect.translate(Vec2::new(0.0, -overflow.y));
        }

        #[allow(deprecated)]
        let id = Id::new("dood_text_input");
        let response = ui.put(
            rect,
            TextEdit::singleline(&mut draft.buffer)
                .id(id)
                .desired_width(width),
        );

        // フォーカス要求は開いた瞬間の1回だけ（毎フレーム要求すると IME 組成が壊れる）。
        if !draft.focused {
            response.request_focus();
            draft.focused = true;
            self.text = Some(draft);
            return;
        }

        let (enter, escape) = ui.input(|input| {
            (
                input.key_pressed(Key::Enter),
                input.key_pressed(Key::Escape),
            )
        });

        if escape {
            return;
        }

        if enter || response.lost_focus() {
            let text = draft.buffer.trim().to_owned();
            if !text.is_empty() {
                self.history.record(&self.scene);
                self.scene.push(Item::Text {
                    pos: draft.pos,
                    text,
                    size: draft.size,
                    color: draft.color,
                });
            }
            return;
        }

        self.text = Some(draft);
    }

    // ---- 書き出し -------------------------------------------------------

    fn flush_export(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.pending_export.take() else {
            return;
        };
        if self.busy.load(Ordering::SeqCst) {
            self.pending_export = Some(kind);
            return;
        }
        if !self.canvas_rect.is_positive() {
            self.set_status("キャンバスがまだ描画されていません", true);
            return;
        }

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
                                false,
                            );
                        }
                        ExportKind::Save => match export::save_png(&cropped, &path) {
                            Ok(()) => {
                                ctx.copy_text(path.display().to_string());
                                set_shared(
                                    &status,
                                    format!(
                                        "保存しました（{width}×{height}px）: {}",
                                        path.display()
                                    ),
                                    false,
                                );
                            }
                            Err(error) => set_shared(&status, error, true),
                        },
                    }
                }
                Err(error) => set_shared(&status, error, true),
            }
            busy.store(false, Ordering::SeqCst);
            ctx.request_repaint();
        });
    }
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
        self.handle_shortcuts(ui.ctx());
        self.handle_paste(ui.ctx());

        self.toolbar(ui);
        let area = ui.available_rect_before_wrap();
        self.canvas(ui, area);
        self.text_editor(ui);
        self.flush_export(ui.ctx());
    }
}
