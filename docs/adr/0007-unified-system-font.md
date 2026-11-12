---
status: Accepted
date: 2026-11-12
---

# Context

ツールバーの日本語ラベルと英数字が混在すると、文字の縦位置（ベースライン）やボタン内の
上下中央が揃わない、という指摘があった（「上下中央からずれてる」「日本語も英語もフォントを
統一して」「日本語と英語の中央揃えが変なので直して」）。

原因は eframe 0.37 の既定フォント構成にある。`FontFamily::Proportional` の先頭は同梱の
`Ubuntu-Light`（CJK 字形なし）で、英数字は Ubuntu、日本語は `system_font_fallback`
（macOS では CoreText 経由のヒラギノ）で描かれる。行の高さ・ベースラインは先頭フォントの
メトリクスで計算されるため、フォールバックされた和文と英数字で縦位置が食い違う。

ADR 0004 は「フォントは同梱せず、eframe のシステムフォントフォールバックに任せる」と
していたが、この方針のままではメトリクスが揃わない。

# Decision

起動時に macOS のシステムフォントから**和文・欧文を同一ファイルで持つフォント**を読み込み、
`FontFamily::Proportional` の最優先（`FontPriority::Highest`）に登録する。

- 候補は次の順で、最初に読めたものを使う:
  1. `/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc`
  2. `/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc`
  3. `/System/Library/Fonts/Hiragino Sans GB.ttc`
  4. `$HOME/Library/Fonts/NotoSansCJKjp-Regular.otf`
- 読み込みは `std::fs::read` + `egui::FontData::from_owned`、登録は
  `Context::add_font(FontInsert::new(..., FontPriority::Highest))`。
- 候補が1つも読めない場合は何もしない。従来どおり `system_font_fallback` が働き、
  字形が無い環境では `MissingGlyphPolicy::Tofu` で豆腐を描いて継続する。
- フォントファイルは引き続きバイナリに同梱しない（ADR 0004 の「同梱しない」方針は維持）。
- ボタン内のラベルは `Painter::text(..., Align2::LEFT_CENTER)` ではなく、レイアウト済み
  `Galley` の実寸ボックスをボタン矩形の中心に置く（`center_y - galley.size().y * 0.5`）。
  フォントの内部パディングに依存せず上下中央を揃えるため。

不採用: Noto Sans JP などの同梱（バイナリが数MB増え、ライセンス表記の管理も要る）、
欧文用と和文用で別フォントを指定する方式（メトリクスが揃わない）。

# Consequences

## Pros

- 英数字と日本語が同じフォント・同じメトリクスで描かれ、ベースラインと上下中央が揃う。
- IME の変換候補とアプリ内の字形が揃う。
- バイナリサイズは増えない（システムフォントを実行時に読むだけ）。

## Cons

- 環境依存。候補フォントが無い環境では従来のフォールバックに戻り、見た目が変わる。
- 起動時に数 MB〜20MB 超のフォントファイルを読むため、起動がわずかに遅くなる。
- 出力 PNG の字形・字幅はマシンごとに変わる（画素単位の再現性はない）。

## 関連

- [ADR 0004](0004-japanese-via-system-fonts.md) を Superseded にした。
