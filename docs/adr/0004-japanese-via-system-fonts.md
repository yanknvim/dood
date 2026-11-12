---
status: Superseded
date: 2026-11-12
superseded_by: 0007-unified-system-font.md
---

> **Superseded by [ADR 0007](0007-unified-system-font.md)**: 和文だけをフォールバックに
> 任せると英数字とメトリクスが揃わないため、和文・欧文統合フォントを `Proportional` の
> 最優先に登録する方針へ変更した。「フォントは同梱しない」方針は 0007 でも維持している。

# Context

egui 同梱のフォントは Hack、Ubuntu-Light、NotoEmoji、egui-icons で、CJK の字形を持たない。
日本語のラベルを描くにはフォントを足す必要がある。

選択肢:
1. Noto Sans JP などをバイナリに同梱する（数MB増、OFL のライセンス表記が必要、全環境で同一の見た目）。
2. OS のシステムフォントを実行時に読み込む（バイナリは軽いが環境依存）。
3. eframe 0.37 の `system_fonts` フィーチャと `system_font_fallback`（既定 true）に任せる。
   macOS では CoreText 経由で和文フォントが自動的に選ばれる。

このマシンには `~/Library/Fonts` に Noto Sans CJK JP があり、`/System/Library/Fonts` に
ヒラギノがある。同梱しなくても表示は成立する。

# Decision

フォントは同梱しない。eframe の既定（`system_fonts` ＋ `system_font_fallback = true`）に任せる。

豆腐が出た場合の手順だけ残す: `~/Library/Fonts/NotoSansCJKjp-Regular.otf` を
`egui::FontData::from_owned` で読み、`Context::add_font` で Proportional の先頭に入れる。

# Consequences

## Pros

- バイナリに数MBのフォントを含めずに済み、ライセンス表記の管理も不要。
- OS のフォント設定（ヒラギノ等）がそのまま反映される。
- IME の変換候補ウィンドウとアプリ内の字形が揃う。

## Cons

- 環境依存になる。フォントを持たない環境では豆腐になる。
- 出力 PNG の字形・字幅は環境ごとに変わり、画素単位の再現性はない。
- ヘッドレス描画（`egui_kittest`）ではシステムフォントプロバイダを明示しないと和文が出ないため、
  自動テストでは和文を描かない（英数字のみで画素検証する）。
