fn main() {
    tauri_build::build();

    // tauri_build は resource.lib（Common Controls v6 マニフェスト含む）を
    // rustc-link-arg-bins でアプリバイナリにのみリンクする。
    // lib test バイナリにはマニフェストが付かず、comctl32.dll v5 がロードされ
    // TaskDialogIndirect が見つからない (STATUS_ENTRYPOINT_NOT_FOUND) 問題が起きる。
    //
    // cargo:rustc-link-arg（全リンクターゲット対象）で resource.lib を追加してテストバイナリでも
    // Common Controls v6 マニフェストが有効になるよう回避する。
    // cdylib/staticlib ではリソースは無視されるため副作用なし。
    // cargo:rustc-link-arg-tests は crate-type が複数ある場合には使用不可のため利用しない。
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let resource_lib = out_dir.join("resource.lib");
    if resource_lib.exists() {
        println!("cargo:rustc-link-arg={}", resource_lib.display());
    }
}
