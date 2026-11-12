//! テキスト入力の統合テスト。
//!
//! `egui_kittest` で実際にクリック・キー入力を流し、複数行テキストが
//! 1つの `Item::Text` として確定すること、入力中に単キーショートカットが
//! ツールを切り替えないことを確認する（GPU が必要）。

use std::path::PathBuf;

use dood::{app::DoodApp, scene::Item, tool::Tool};
use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, Vec2, pos2};

type AppState = Option<DoodApp>;

fn build_harness() -> egui_kittest::Harness<'static, AppState> {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(Vec2::new(600.0, 400.0))
        // キャンバス上の入力欄は accessible name を持たない（egui の TextEdit 既定）。
        .with_accessibility_check(false)
        .build_ui_state(
            |ui, state: &mut AppState| {
                let app = state
                    .get_or_insert_with(|| DoodApp::with_ctx(ui.ctx(), PathBuf::from("dood.png")));
                app.render_ui(ui);
            },
            None,
        );
    harness.run();
    harness
}

fn app<'a>(harness: &'a egui_kittest::Harness<'static, AppState>) -> &'a DoodApp {
    harness
        .state()
        .as_ref()
        .expect("アプリが初期化されていない")
}

fn click(harness: &mut egui_kittest::Harness<'static, AppState>, pos: Pos2) {
    harness.hover_at(pos);
    harness.run();
    harness.event(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    harness.event(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();
}

fn type_text(harness: &mut egui_kittest::Harness<'static, AppState>, text: &str) {
    harness.event(Event::Text(text.to_owned()));
    harness.run();
}

#[test]
fn shift_enter_writes_a_multiline_text_item() {
    let mut harness = build_harness();

    // テキストツールに切り替え、キャンバスをクリックして入力欄を開く。
    harness.key_press(Key::T);
    harness.run();
    assert_eq!(app(&harness).tool(), Tool::Text);

    click(&mut harness, pos2(100.0, 300.0));

    // 入力中の母音キーはツール切り替えに奪われない。
    harness.key_press(Key::A);
    harness.run();
    harness.key_press(Key::O);
    harness.run();
    assert_eq!(
        app(&harness).tool(),
        Tool::Text,
        "テキスト入力中の a / o がショートカットに奪われた"
    );

    type_text(&mut harness, "a");
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
    harness.run();
    type_text(&mut harness, "b");

    // Shift なしの Enter で確定。
    harness.key_press(Key::Enter);
    harness.run();

    let texts: Vec<(String, f32)> = app(&harness)
        .scene()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Text { text, size, .. } => Some((text.clone(), *size)),
            _ => None,
        })
        .collect();

    assert_eq!(texts.len(), 1, "テキストが1つに確定していない: {texts:?}");
    assert_eq!(texts[0].0, "a\nb", "Shift+Enter の改行が保持されていない");
    assert!(texts[0].1 >= 18.0);
}

#[test]
fn escape_discards_the_text_draft() {
    let mut harness = build_harness();

    harness.key_press(Key::T);
    harness.run();
    click(&mut harness, pos2(100.0, 300.0));

    type_text(&mut harness, "x");
    harness.key_press(Key::Escape);
    harness.run();

    assert!(
        app(&harness).scene().items.is_empty(),
        "Esc で破棄されずにテキストが確定した"
    );
}
