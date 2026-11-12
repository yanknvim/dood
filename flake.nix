{
  description = "dood — コーディングエージェントに渡すための最小スクラッチパッド";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    flake-parts.url = "github:hercules-ci/flake-parts";

    # 対応システムの一覧（aarch64/x86_64 × darwin/linux）を提供する。
    systems.url = "github:nix-systems/default";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      # ADR 0001 では macOS (Apple Silicon) 専用だが、ビルド定義は
      # nix-systems/default が返す 4 システムすべてで評価できるようにする。
      systems = import inputs.systems;

      perSystem =
        { system, ... }:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
          };

          # Cargo.toml の rust-version (1.95) を満たす stable を使う。
          # rust-src / clippy / rustfmt / rust-analyzer を devShell に含める。
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-src"
              "rustfmt"
              "clippy"
              "rust-analyzer"
            ];
          };

          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };
        in
        {
          packages.default = rustPlatform.buildRustPackage {
            pname = "dood";
            version = "0.1.0";
            src = ./.;

            cargoLock.lockFile = ./Cargo.lock;

            # テストは egui_kittest が Metal の GPU アダプタを要求するため、
            # サンドボックス内の `nix build` では実行できない。
            # テストは `nix develop` の中で `cargo test` で回す。
            doCheck = false;

            meta = {
              description = "手描きの図をコーディングエージェントに渡すための最小 GUI アプリ";
              mainProgram = "dood";
              platforms = [
                "aarch64-darwin"
                "x86_64-darwin"
                "aarch64-linux"
                "x86_64-linux"
              ];
            };
          };

          devShells.default = pkgs.mkShell {
            packages = with pkgs; [
              rustToolchain
              pkg-config
            ];

            env.RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

            shellHook = ''
              echo "dood devShell — $(rustc --version)"
              echo "  cargo run --release     # ウィンドウを開く"
              echo "  cargo test              # 単体テスト + ヘッドレス描画テスト"
              echo "  cargo build             # ビルド"
            '';
          };

          formatter = pkgs.nixfmt-rfc-style;
        };
    };
}
