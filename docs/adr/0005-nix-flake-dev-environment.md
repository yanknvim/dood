---
status: Accepted
date: 2026-11-12
---

# Context

dood は macOS（Apple Silicon）専用の GUI アプリで、Rust のツールチェーンと
ネイティブ依存（Metal / AppKit）を揃える必要がある。手元には rustup 管理の
rustc 1.98.1 があり、`Cargo.toml` の MSRV は 1.95。

環境構築とビルドを再現可能にしたいが、次の制約がある。

- eframe 0.37 / wgpu 30 を含むため初回ビルドが重く、ツールチェーンの版がぶれると
  再現できない。
- テスト（`egui_kittest`）は Metal の GPU アダプタを要求し、Nix のビルドサンドボックス
  内では実行できない。
- ホストは Nix（Determinate Nix）で、flakes が使える。

# Decision

ルートに `flake.nix` を置き、flake-parts で出力を構成する。

- **flake-parts**: `perSystem` で devShell・パッケージ・formatter を宣言する。
- **rust-overlay**: `rust-bin.stable.latest.default` に `rust-src` / `rustfmt` /
  `clippy` / `rust-analyzer` を足したツールチェーンを devShell とビルドの両方で使う。
- **nix-systems/default**: 対応システム一覧（aarch64/x86_64 × darwin/linux）を
  input として固定し、`systems = import inputs.systems` で使う。
- **パッケージ**: `rustPlatform.buildRustPackage` に `Cargo.lock` を渡し、
  `packages.default` で `result/bin/dood` を生成する。
- **テスト**: サンドボックスで GPU が使えないため `doCheck = false` とし、
  テストは `nix develop` 内の `cargo test` で回す。
- **direnv**: `.envrc` は `use flake` のみ（nix-direnv 前提）。`.direnv` と
  `result` は `.gitignore` に入れる。

不採用: naersk / crane / cargo2nix（依存の少ない単一クレートには過剰）、
rust-toolchain.toml によるツールチェーン固定（版の管理元が flake.lock と二重になる）。

# Consequences

## Pros

- `nix develop`（または `direnv allow`）だけで rustc / cargo / clippy / rustfmt /
  rust-analyzer が揃う。
- `nix build` が `Cargo.lock` に基づいて再現可能なバイナリを作る。
- flake.lock が nixpkgs・rust-overlay・systems の版を固定する。更新は
  `nix flake update` に集約できる。

## Cons

- `nix build` はテストを実行しない。テストは devShell 内で明示的に回す必要がある。
- ツールチェーンは「stable の最新」を追うため、`nix flake update` で
  rustc の版が上がる。MSRV 1.95 を下回らないことは保証されない。
- Linux 向けの devShell も評価できるが、GUI の実行には X11 / Wayland などの
  実行時依存が別途必要（ADR 0001 のとおり主対象は macOS）。
