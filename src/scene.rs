//! 図形データだけを持つ層。egui のウィジェットには依存しない。

use eframe::egui::{Color32, Pos2, TextureHandle, Vec2};

/// キャンバス上の1要素。座標はキャンバス左上を原点とする pt。
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Stroke {
        points: Vec<Pos2>,
        color: Color32,
        width: f32,
    },
    Line {
        a: Pos2,
        b: Pos2,
        color: Color32,
        width: f32,
    },
    Arrow {
        a: Pos2,
        b: Pos2,
        color: Color32,
        width: f32,
    },
    Rect {
        a: Pos2,
        b: Pos2,
        color: Color32,
        width: f32,
    },
    Ellipse {
        center: Pos2,
        radius: Vec2,
        color: Color32,
        width: f32,
    },
    Text {
        pos: Pos2,
        text: String,
        size: f32,
        color: Color32,
        /// 自動折り返しの最大幅（pt）。`None` なら折り返さない。
        wrap_width: Option<f32>,
    },
}

/// 下敷きにしている画像。
#[derive(Clone)]
pub struct Background {
    pub texture: TextureHandle,
    pub size_px: [usize; 2],
    pub size_pt: Vec2,
}

/// スクラッチパッド1枚分の内容。
#[derive(Clone, Default)]
pub struct Scene {
    pub items: Vec<Item>,
    pub bg: Option<Background>,
}

impl Scene {
    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }

    /// 背景も含めて空にする。
    pub fn clear(&mut self) {
        self.items.clear();
        self.bg = None;
    }
}

const HISTORY_LIMIT: usize = 100;

#[derive(Clone, Default)]
struct Snapshot {
    items: Vec<Item>,
    bg: Option<Background>,
}

impl Snapshot {
    fn of(scene: &Scene) -> Self {
        Self {
            items: scene.items.clone(),
            bg: scene.bg.clone(),
        }
    }

    fn restore(self, scene: &mut Scene) {
        scene.items = self.items;
        scene.bg = self.bg;
    }
}

/// スナップショット方式の undo/redo。要素数が小さいので clone で足りる。
#[derive(Default)]
pub struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl History {
    /// 変更を加える直前に呼ぶ。
    pub fn record(&mut self, scene: &Scene) {
        self.undo.push(Snapshot::of(scene));
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, scene: &mut Scene) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(Snapshot::of(scene));
        previous.restore(scene);
        true
    }

    pub fn redo(&mut self, scene: &mut Scene) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(Snapshot::of(scene));
        next.restore(scene);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    fn item(x: f32) -> Item {
        Item::Line {
            a: pos2(x, 0.0),
            b: pos2(x, 10.0),
            color: Color32::BLACK,
            width: 2.0,
        }
    }

    #[test]
    fn push_recorded_then_undo_restores() {
        let mut scene = Scene::default();
        let mut history = History::default();

        history.record(&scene);
        scene.push(item(1.0));
        assert_eq!(scene.items.len(), 1);
        assert!(history.can_undo());

        assert!(history.undo(&mut scene));
        assert!(scene.items.is_empty());
        assert!(history.can_redo());

        assert!(history.redo(&mut scene));
        assert_eq!(scene.items.len(), 1);
    }

    #[test]
    fn redo_is_dropped_by_new_action() {
        let mut scene = Scene::default();
        let mut history = History::default();

        history.record(&scene);
        scene.push(item(1.0));
        history.undo(&mut scene);
        assert!(history.can_redo());

        history.record(&scene);
        assert!(!history.can_redo());
    }

    #[test]
    fn clear_is_undoable() {
        let mut scene = Scene::default();
        let mut history = History::default();
        scene.push(item(1.0));
        scene.push(item(2.0));

        history.record(&scene);
        scene.clear();
        assert!(scene.items.is_empty());

        history.undo(&mut scene);
        assert_eq!(scene.items.len(), 2);
    }

    #[test]
    fn history_is_capped() {
        let mut scene = Scene::default();
        let mut history = History::default();
        for i in 0..(HISTORY_LIMIT + 20) {
            history.record(&scene);
            scene.push(item(i as f32));
        }
        let mut undone = 0;
        while history.undo(&mut scene) {
            undone += 1;
        }
        assert_eq!(undone, HISTORY_LIMIT);
    }

    #[test]
    fn undo_on_empty_history_is_noop() {
        let mut scene = Scene::default();
        let mut history = History::default();
        assert!(!history.undo(&mut scene));
        assert!(!history.redo(&mut scene));
    }
}
