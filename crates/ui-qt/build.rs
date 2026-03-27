fn main() {
    let qmake = std::env::var("QMAKE").unwrap_or_else(|_| "qmake6".to_string());
    let qt_headers = std::process::Command::new(&qmake)
        .args(["-query", "QT_INSTALL_HEADERS"])
        .output()
        .expect("Failed to run qmake6")
        .stdout;
    let qt_headers = String::from_utf8(qt_headers).unwrap().trim().to_string();

    // Explicitly link Qt6Network so QML Image can load http/https URLs.
    // qttypes links Qt6Quick (which NEEDS libQt6Network at the .so level) but does
    // not add it to the Rust link line, so the TLS backend plugin may not initialise.
    let qt_libs = std::process::Command::new(&qmake)
        .args(["-query", "QT_INSTALL_LIBS"])
        .output()
        .expect("Failed to run qmake6 for libs")
        .stdout;
    let qt_libs = String::from_utf8(qt_libs).unwrap().trim().to_string();
    println!("cargo:rustc-link-search=native={}", qt_libs);
    println!("cargo:rustc-link-lib=Qt6Network");

    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    cc::Build::new()
        .cpp(true)
        .flag("-std=c++17")
        .include(&qt_headers)
        .include(format!("{}/QtCore", qt_headers))
        .include(format!("{}/QtGui", qt_headers))
        .file(format!("{}/src/qt_helpers.cpp", manifest))
        .compile("qt_helpers");
}
