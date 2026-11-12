---
status: Accepted
date: 2026-11-12
---

# Context

ADR 0005 の devShell は rustc / cargo / pkg-config だけを含み、Linux で GUI を
起動するための実行時依存を持っていなかった。そのため Linux 上では `cargo run` が
ビルドには成功しても、実行時に次のどちらかで落ちていた。

- winit が `libwayland-client` を dlopen できず `NoWaylandLib`
- wgpu が `libvulkan` / `libEGL` をロードできず、バックエンドが0個になり
  `FailedToCreateSurfaceForAnyBackend`

eframe / winit / wgpu は X11・Wayland・EGL・Vulkan のクライアントライブラリを
ビルド時にリンクせず dlopen する。したがって `LD_LIBRARY_PATH` に載っていないと
ビルドが通っていても起動できない。NixOS では Mesa のドライバは
`/run/opengl-driver` にあるが、クライアントライブラリは devShell に入れる必要がある。

macOS（ADR 0001 の主対象）では AppKit / Metal がシステムにあり、この問題は起きない。

# Decision

Linux のときだけ、devShell に GUI の実行時依存を追加し `LD_LIBRARY_PATH` を設定する。

- `devShells.default.packages` に Linux 限定で次を追加する:
  `wayland`、`libxkbcommon`、`fontconfig`、`libGL`、`vulkan-loader`、
  `libx11`、`libxcursor`、`libxi`、`libxrandr`
- `shellHook` の末尾で `pkgs.lib.makeLibraryPath` の結果を既存の
  `LD_LIBRARY_PATH` の前に足す。既存値は残す。
- 追加は `pkgs.stdenv.isLinux` で囲み、darwin の評価結果と devShell は変えない。
- ドライバ（Mesa の Vulkan ICD / EGL ベンダー）は NixOS の `/run/opengl-driver`
  から読む。devShell にはドライバを同梱しない。
- バイナリ自体のリンクは変えない（dlopen のまま）。`nix build` の成果物は
  `LD_LIBRARY_PATH` を設定した環境で動かす必要がある。

不採用: `mesa` を devShell に同梱する案（NixOS のドライバと二重になり、GPU を
選び間違える）、バイナリに rpath を埋める案（`/nix/store` のパスが固定され、
`nix build` の成果物が環境に依存する）、X11 のみに絞る案（主対象の Linux 環境が
Wayland）、`libGL` を省く案（Vulkan が使えない環境で GL フォールバックが死ぬ）。

# Consequences

## Pros

- Linux（Wayland / X11）で `cargo run` がそのままウィンドウを開く。
- macOS の devShell は従来どおりで、追加コストはない。
- テスト（`cargo test`）も同じ devShell で動く。

## Cons

- Linux の devShell クロージャが大きくなる。
- Mesa のドライバが `/run/opengl-driver` に無い環境（NixOS 以外）では、
  クライアントライブラリは揃っても GPU が使えないことがある。
- `nix build` した成果物は `LD_LIBRARY_PATH` を別途用意しないと起動しない。
- サンドボックス内では `/dev/dri` への書き込みが拒否されるため、エージェントの
  シェルからは GPU 描画まで確認できない（ユーザーの通常の端末では描画できる）。
