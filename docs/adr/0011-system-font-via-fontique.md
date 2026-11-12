---
status: Accepted
date: 2026-11-12
---

# Context

Linux に移植したところ、UI の文字が細すぎるという指摘があった。原因は2つある。

- ADR 0007 は macOS の固定パス（ヒラギノ角ゴシック W3 / W4）からフォントを読んでいた。
  Linux にはそのパスが無いため、`configure_fonts` は何も登録せず、egui 同梱の
  `Ubuntu-Light`（weight 300）で描かれていた。
- この環境の日本語フォントは Noto Sans CJK JP の可変フォント（`NotoSansCJK-VF.otf.ttc`）で、
  既定の `wght` 軸は 100（Thin）。fontique で同じフォントを引いても、軸を指定しないと細いまま。
- NixOS ではフォントの実体が `/nix/store/...` のビルドごとに異なるパスに置かれるため、
  固定パスの候補リストでは拾えない。

ADR 0004 / 0007 の「フォントをバイナリに同梱しない」方針は維持したい。

# Decision

起動時に fontique でシステムフォントを引き、和文・欧文を同じファイルで描けるものを
`FontFamily::Proportional` の最優先（`FontPriority::Highest`）に登録する。

- クエリは `GenericFamily::SansSerif`、locale `ja`、Han スクリプトのフォールバック。
- 候補のうち `A` と `あ` の両方を描けるフォントを優先する。無ければ最初の候補に
  フォールバックし、和文は eframe の `system_font_fallback` に任せる。
- ウェイトは Medium（500）を要求する。可変フォントで `wght` 軸を持つ場合は、
  `FontTweak::coords` で 500 に固定する。
- 読み込んだフォントはプロセス内で一度だけ解決して使い回す。
- ボタンラベルを実寸 galley の中心に置く方式（ADR 0007 の後半）は維持する。

不採用: 固定パスの候補リスト（NixOS でパスが安定しない）、フォントの同梱（バイナリサイズと
ライセンス）、描画時の太字合成（字形が崩れる）。

# Consequences

## Pros

- Linux / macOS のどちらでも、同じフォントファイルで欧文と和文を Medium で描ける。
- フォントの配置がビルドごとに変わる環境（NixOS）でも起動時に解決できる。
- フォントを同梱しない方針を維持できる。

## Cons

- 起動時にシステムフォントの列挙が走る（環境によっては数百 ms）。
- Linux では fontconfig、macOS では CoreText を実行時に使う。ADR 0010 の
  `LD_LIBRARY_PATH` が無い環境ではフォントを引けないことがある。
- 実際に選ばれる字形とウェイトは環境依存で、出力 PNG の画素単位の再現性はない。
- 和文と欧文を1ファイルで持たない環境では、片方がフォールバックになりメトリクスが揃わない。

## 関連

- [ADR 0007](0007-unified-system-font.md) を Superseded にした（ラベル中央揃えの方針は引き継ぐ）。
