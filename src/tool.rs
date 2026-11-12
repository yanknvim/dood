//! ツール・パレット・キー割当の定義。

use eframe::egui::{Color32, Key};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Pen,
    Line,
    Arrow,
    Rect,
    Ellipse,
    Text,
}

impl Tool {
    pub const ALL: [Tool; 6] = [
        Tool::Pen,
        Tool::Line,
        Tool::Arrow,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Text,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Pen => "ペン",
            Tool::Line => "直線",
            Tool::Arrow => "矢印",
            Tool::Rect => "矩形",
            Tool::Ellipse => "楕円",
            Tool::Text => "テキスト",
        }
    }

    /// ツールバーに出す単キーのヒント。
    pub fn key_label(self) -> &'static str {
        match self {
            Tool::Pen => "P",
            Tool::Line => "L",
            Tool::Arrow => "A",
            Tool::Rect => "R",
            Tool::Ellipse => "O",
            Tool::Text => "T",
        }
    }

    pub fn key(self) -> Key {
        match self {
            Tool::Pen => Key::P,
            Tool::Line => Key::L,
            Tool::Arrow => Key::A,
            Tool::Rect => Key::R,
            Tool::Ellipse => Key::O,
            Tool::Text => Key::T,
        }
    }

    pub fn from_key(key: Key) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.key() == key)
    }
}

/// 色パレット。白は背景を塗って消す「擬似消しゴム」。
pub const PALETTE: [(&str, Color32); 6] = [
    ("黒", Color32::from_rgb(0x1a, 0x1a, 0x1a)),
    ("赤", Color32::from_rgb(0xd6, 0x27, 0x28)),
    ("青", Color32::from_rgb(0x1c, 0x5c, 0xd6)),
    ("緑", Color32::from_rgb(0x1f, 0x8a, 0x3d)),
    ("橙", Color32::from_rgb(0xe0, 0x7a, 0x18)),
    ("白", Color32::WHITE),
];

pub const WIDTHS: [f32; 3] = [2.0, 4.0, 8.0];
pub const TEXT_SIZES: [f32; 3] = [18.0, 24.0, 32.0];

/// 色番号キー（1〜6）。
pub const COLOR_KEYS: [Key; 6] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
];

pub const THINNER_KEY: Key = Key::OpenBracket;
pub const THICKER_KEY: Key = Key::CloseBracket;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_a_unique_key() {
        for (i, tool) in Tool::ALL.iter().enumerate() {
            for other in Tool::ALL.iter().skip(i + 1) {
                assert_ne!(tool.key(), other.key());
            }
            assert_eq!(Tool::from_key(tool.key()), Some(*tool));
        }
    }

    #[test]
    fn palette_and_widths_have_expected_lengths() {
        assert_eq!(PALETTE.len(), 6);
        assert_eq!(WIDTHS.len(), 3);
        assert_eq!(TEXT_SIZES.len(), 3);
        assert_eq!(COLOR_KEYS.len(), PALETTE.len());
    }
}
