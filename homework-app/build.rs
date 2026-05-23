use std::process::Command;

fn qmake_query(var: &str) -> String {
    let qmake = std::env::var("QMAKE").unwrap_or_else(|_| "qmake".to_string());
    let output = Command::new(&qmake)
        .args(["-query", var])
        .output()
        .unwrap_or_else(|e| panic!("failed to run {qmake}: {e}"));
    if !output.status.success() {
        panic!(
            "qmake query failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_string()
}

fn qt_paths() -> (String, String) {
    if let (Ok(inc), Ok(flags)) = (
        std::env::var("DEP_QT_INCLUDE_PATH"),
        std::env::var("DEP_QT_COMPILE_FLAGS"),
    ) {
        return (inc, flags);
    }
    let include = std::env::var("QT_INCLUDE_PATH").unwrap_or_else(|_| qmake_query("QT_INSTALL_HEADERS"));
    let lib_path = std::env::var("QT_LIBRARY_PATH").unwrap_or_else(|_| qmake_query("QT_INSTALL_LIBS"));
    let mut flags = vec![];
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        flags.push("/permissive-".to_string());
        flags.push("/utf-8".to_string());
        flags.push("/Zc:__cplusplus".to_string());
        flags.push("/std:c++17".to_string());
    } else {
        flags.push("-std=c++17".to_string());
    }
    println!("cargo:rustc-link-search=native={lib_path}");
    println!("cargo:rustc-link-lib=Qt6Widgets");
    println!("cargo:rustc-link-lib=Qt6Gui");
    println!("cargo:rustc-link-lib=Qt6Core");
    (include, flags.join(";"))
}

#[cfg(windows)]
fn embed_windows_icon() {
    let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("icon.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    if !icon.exists() {
        return;
    }
    if let Err(err) = winres::WindowsResource::new()
        .set_icon(icon.to_str().expect("icon path utf-8"))
        .compile()
    {
        println!("cargo:warning=failed to embed icon.ico: {err}");
    }
}

fn main() {
    #[cfg(windows)]
    embed_windows_icon();

    let (include, compile_flags) = qt_paths();
    let mut config = cpp_build::Config::new();
    for flag in compile_flags.split_terminator(';') {
        if !flag.is_empty() {
            config.flag(flag);
        }
    }
    config.include(&include);
    for module in ["QtCore", "QtGui", "QtWidgets"] {
        config.include(format!("{include}/{module}"));
    }
    config.file("cpp/ui_bridge.cpp");
    config.build("src/ffi.rs");
    println!("cargo:rerun-if-changed=cpp/ui_bridge.cpp");
    println!("cargo:rerun-if-changed=cpp/ui_bridge.h");
}
