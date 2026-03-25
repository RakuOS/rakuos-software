// rakuos-software-qt — Qt6/QML software center frontend

mod backend;
use backend::SoftwareBackend;
use qmetaobject::QmlEngine;

extern "C" {
    fn set_qt_app_properties();
}

fn main() {
    env_logger::init();
    std::env::set_var("QML_XHR_ALLOW_FILE_READ", "1");

    qmetaobject::qml_register_type::<SoftwareBackend>(
        c"org.rakuos.software",
        1, 0,
        c"SoftwareBackend",
    );

    let mut engine = QmlEngine::new();
    unsafe { set_qt_app_properties(); }

    let qml_dir = std::env::var("RAKUOS_SOFTWARE_QML_DIR")
        .unwrap_or_else(|_| "/usr/share/rakuos-software-qt".to_string());
    let qml_dir = std::fs::canonicalize(&qml_dir)
        .unwrap_or_else(|_| std::path::PathBuf::from(&qml_dir));
    engine.load_file(format!("file://{}/main.qml", qml_dir.display()).into());
    engine.exec();
}
