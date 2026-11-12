//! dood — コーディングエージェントに渡すための最小スクラッチパッド。

use std::path::PathBuf;

use dood::app::DoodApp;

fn main() -> eframe::Result {
    let mut out: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                if let Some(path) = args.next() {
                    out = Some(PathBuf::from(path));
                }
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other if other.starts_with("--out=") => {
                out = Some(PathBuf::from(&other["--out=".len()..]));
            }
            _ => {}
        }
    }

    let out_path = out
        .or_else(|| std::env::var_os("DOOD_OUT").map(PathBuf::from))
        .unwrap_or_else(default_out_path);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("dood")
            .with_inner_size([1100.0, 800.0])
            .with_min_inner_size([480.0, 360.0])
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "dood",
        options,
        Box::new(move |cc| Ok(Box::new(DoodApp::new(cc, out_path)))),
    )
}

fn default_out_path() -> PathBuf {
    match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home).join("Desktop").join("dood.png"),
        None => PathBuf::from("dood.png"),
    }
}

fn print_help() {
    println!(
        "dood — 最小スクラッチパッド\n\n\
         使い方: dood [--out PATH]\n\n\
         オプション:\n\
         \x20 --out PATH   書き出し先（既定: $HOME/Desktop/dood.png）\n\
         \x20 -h, --help   このヘルプ\n\n\
         環境変数:\n\
         \x20 DOOD_OUT     --out が無いときに使う書き出し先\n\n\
         操作:\n\
         \x20 P/L/A/R/O/T  ペン/直線/矢印/矩形/楕円/テキスト\n\
         \x20 1-6          色、 [ ] 太さ\n\
         \x20 ⌘Z / ⇧⌘Z    元に戻す / やり直す、 ⌘⌫ 全消去\n\
         \x20 ⌘C           キャンバスを PNG でクリップボードへ\n\
         \x20 ⌘S           PNG を保存してパスをクリップボードへ\n\
         \x20 ⌘V           背景画像を取り込む\n\
         \x20 ⌘⇧T          最前面の切り替え"
    );
}
