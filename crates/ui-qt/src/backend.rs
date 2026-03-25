// ui-qt/backend.rs — QObject backend exposed to QML

#![allow(non_snake_case)]

use qmetaobject::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

// ── Shared async state ────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SharedState {
    pub running:  AtomicBool,
    /// 0=idle 1=success 2=failed
    pub result:   AtomicI32,
    pub progress: AtomicI32,  // 0-100
}

fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("rakuos-software-qt.log")
}

fn settings_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    std::path::PathBuf::from(home).join(".config/rakuos/software-settings.json")
}

// ── Backend QObject ───────────────────────────────────────────────────────────

#[derive(QObject, Default)]
pub struct SoftwareBackend {
    base: qt_base_class!(trait QObject),

    // Navigation
    currentPage: qt_property!(i32; NOTIFY currentPageChanged),
    currentPageChanged: qt_signal!(),

    // Operation state (for showing progress/log area)
    opRunning: qt_property!(bool; NOTIFY opStateChanged),
    opResult:  qt_property!(i32;  NOTIFY opStateChanged),   // 0=idle 1=ok 2=fail
    opProgress: qt_property!(i32; NOTIFY opStateChanged),   // 0-100
    logRevision: qt_property!(i32; NOTIFY logRevisionChanged),
    logRevisionChanged: qt_signal!(),
    opStateChanged: qt_signal!(),

    // Search
    searchQuery: qt_property!(QString; NOTIFY searchQueryChanged),
    searchQueryChanged: qt_signal!(),

    // Cached JSON strings — UI reads these as JS objects via JSON.parse
    homeDataJson:      qt_property!(QString; NOTIFY homeDataChanged),
    homeDataChanged:   qt_signal!(),
    installedJson:     qt_property!(QString; NOTIFY installedChanged),
    installedChanged:  qt_signal!(),
    updatesJson:       qt_property!(QString; NOTIFY updatesChanged),
    updatesChanged:    qt_signal!(),
    searchResultsJson: qt_property!(QString; NOTIFY searchResultsChanged),
    searchResultsChanged: qt_signal!(),
    systemStatusJson:  qt_property!(QString; NOTIFY systemStatusChanged),
    systemStatusChanged: qt_signal!(),
    settingsJson:      qt_property!(QString; NOTIFY settingsChanged),
    settingsChanged:   qt_signal!(),

    // Shared state between Rust threads and Qt
    shared: Option<Arc<SharedState>>,

    // ── Methods ───────────────────────────────────────────────────────────────

    navigate: qt_method!(fn navigate(&mut self, page: i32) {
        self.currentPage = page;
        self.currentPageChanged();
    }),

    pollOp: qt_method!(fn pollOp(&mut self) {
        let s = self.get_shared();
        let running  = s.running.load(Ordering::Relaxed);
        let result   = s.result.load(Ordering::Relaxed);
        let progress = s.progress.load(Ordering::Relaxed);

        let changed = self.opRunning != running
            || self.opResult != result
            || self.opProgress != progress;

        if changed {
            self.opRunning  = running;
            self.opResult   = result;
            self.opProgress = progress;
            self.opStateChanged();
        }

        // Poll log file revision
        if let Ok(m) = std::fs::metadata(log_path()) {
            let rev = m.len() as i32;
            if self.logRevision != rev {
                self.logRevision = rev;
                self.logRevisionChanged();
            }
        }
    }),

    readLog: qt_method!(fn readLog(&mut self) -> QString {
        std::fs::read_to_string(log_path())
            .unwrap_or_default()
            .into()
    }),

    // ── Home page ─────────────────────────────────────────────────────────────

    loadHome: qt_method!(fn loadHome(&mut self) {
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                let picks   = rakuos_home::get_picks().await;
                let popular = rakuos_home::get_popular().await;
                let updated = rakuos_home::get_recently_updated().await;
                let new     = rakuos_home::get_new_apps().await;
                serde_json::json!({
                    "picks":   picks,
                    "popular": popular,
                    "updated": updated,
                    "new":     new,
                })
            });
            let _ = std::fs::write(
                log_path(),
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            );
            shared.result.store(1, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    homeDataLoaded: qt_method!(fn homeDataLoaded(&mut self) -> QString {
        std::fs::read_to_string(log_path()).unwrap_or_default().into()
    }),

    // ── Installed page ────────────────────────────────────────────────────────

    loadInstalled: qt_method!(fn loadInstalled(&mut self) {
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let mut all: Vec<serde_json::Value> = Vec::new();

            // Flatpak installed — enriched with AppStream
            if let Ok(apps) = rakuos_packages::get_installed_flatpaks_enriched() {
                for a in apps {
                    all.push(serde_json::to_value(a).unwrap_or_default());
                }
            }
            // Native/overlay installed
            if let Ok(apps) = rakuos_packages::get_installed() {
                for a in apps {
                    all.push(serde_json::to_value(a).unwrap_or_default());
                }
            }
            for a in rakuos_webapps::get_installed() {
                all.push(serde_json::to_value(a).unwrap_or_default());
            }
            for a in rakuos_appimages::get_installed() {
                all.push(serde_json::to_value(a).unwrap_or_default());
            }

            let _ = std::fs::write(
                log_path(),
                serde_json::to_string(&all).unwrap_or_default(),
            );
            shared.result.store(1, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    // ── App detail lookup ─────────────────────────────────────────────────────

    loadAppById: qt_method!(fn loadAppById(&mut self, app_id: QString) {
        let app_id = app_id.to_string();
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let result = match rakuos_packages::get_app_by_id(&app_id) {
                Ok(Some(app)) => serde_json::to_string(&app).unwrap_or_default(),
                _ => "null".to_string(),
            };
            let _ = std::fs::write(log_path(), &result);
            shared.result.store(1, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    // ── Search ────────────────────────────────────────────────────────────────

    search: qt_method!(fn search(&mut self, query: QString) {
        let query = query.to_string();
        self.searchQuery = query.clone().into();
        self.searchQueryChanged();
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let mut results: Vec<serde_json::Value> = Vec::new();

            if let Ok(apps) = rakuos_packages::search(&query) {
                for a in apps { results.push(serde_json::to_value(a).unwrap_or_default()); }
            }
            for a in rakuos_webapps::search(&query) {
                results.push(serde_json::to_value(a).unwrap_or_default());
            }

            let _ = std::fs::write(
                log_path(),
                serde_json::to_string(&results).unwrap_or_default(),
            );
            shared.result.store(1, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    loadCategory: qt_method!(fn loadCategory(&mut self, category: QString) {
        let category = category.to_string();
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let mut results: Vec<serde_json::Value> = Vec::new();
            if let Ok(apps) = rakuos_packages::get_by_category(&category) {
                for a in apps { results.push(serde_json::to_value(a).unwrap_or_default()); }
            }
            let _ = std::fs::write(
                log_path(),
                serde_json::to_string(&results).unwrap_or_default(),
            );
            shared.result.store(1, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    // ── Updates ───────────────────────────────────────────────────────────────

    checkUpdates: qt_method!(fn checkUpdates(&mut self) {
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            use std::process::Command;

            let _ = std::fs::write(log_path(), "Checking for updates...\n");

            // Check packages
            let pkg_out = Command::new("/usr/libexec/rakuos/rakuos-update")
                .arg("check")
                .output()
                .ok();

            // Check flatpak
            let fp_out = Command::new("/usr/libexec/rakuos/rakuos-update")
                .arg("check-flatpak")
                .output()
                .ok();

            let pkg_updates: Vec<serde_json::Value> = pkg_out
                .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
                .and_then(|v| v["updates"].as_array().cloned())
                .unwrap_or_default();

            let fp_updates: Vec<serde_json::Value> = fp_out
                .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
                .and_then(|v| v["updates"].as_array().cloned())
                .unwrap_or_default();

            let result = serde_json::json!({
                "packages": pkg_updates,
                "flatpak":  fp_updates,
                "total": pkg_updates.len() + fp_updates.len(),
            });

            let _ = std::fs::write(
                log_path(),
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            );

            shared.result.store(1, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    // ── Install / Remove ─────────────────────────────────────────────────────

    installApp: qt_method!(fn installApp(&mut self, id: QString, source: QString) {
        let id = id.to_string();
        let source = source.to_string();
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader, Write};
            use std::process::{Command, Stdio};
            let _ = std::fs::write(log_path(), format!("Installing {}...\n", id));

            let append = |text: &str| {
                if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(log_path()) {
                    let _ = f.write_all(text.as_bytes());
                    let _ = f.write_all(b"\n");
                }
            };

            let (mut child, ok) = match source.as_str() {
                "flatpak" => {
                    let c = Command::new("flatpak")
                        .args(["install", "--noninteractive", "-y", &id])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn();
                    match c {
                        Ok(child) => (Some(child), true),
                        Err(e)    => { append(&e.to_string()); (None, false) }
                    }
                }
                "webapp" => {
                    let (ok, msg) = rakuos_webapps::install(&id);
                    append(&msg);
                    (None, ok)
                }
                _ => {
                    let c = Command::new("pkexec")
                        .args(["rakuos", "install", &id])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn();
                    match c {
                        Ok(child) => (Some(child), true),
                        Err(e)    => { append(&e.to_string()); (None, false) }
                    }
                }
            };

            if let Some(ref mut child) = child {
                if let Some(stdout) = child.stdout.take() {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        if !line.is_empty() { append(&line); }
                    }
                }
                let success = child.wait().map(|s| s.success()).unwrap_or(false);
                shared.result.store(if success { 1 } else { 2 }, Ordering::Relaxed);
            } else {
                shared.result.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
            }
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    removeApp: qt_method!(fn removeApp(&mut self, id: QString, source: QString) {
        let id = id.to_string();
        let source = source.to_string();
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            use std::io::Write;
            use std::process::Command;
            let _ = std::fs::write(log_path(), format!("Removing {}...\n", id));

            let append = |text: &str| {
                if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(log_path()) {
                    let _ = f.write_all(text.as_bytes());
                    let _ = f.write_all(b"\n");
                }
            };

            let ok = match source.as_str() {
                "flatpak" => {
                    let out = Command::new("flatpak")
                        .args(["uninstall", "--noninteractive", "-y", &id])
                        .output()
                        .ok();
                    if let Some(o) = &out {
                        append(&String::from_utf8_lossy(&o.stdout));
                    }
                    out.map(|o| o.status.success()).unwrap_or(false)
                }
                "webapp" => {
                    let (ok, msg) = rakuos_webapps::uninstall(&id);
                    append(&msg);
                    ok
                }
                "appimage" => {
                    let (ok, msg) = rakuos_appimages::uninstall(&id);
                    append(&msg);
                    ok
                }
                _ => {
                    let out = Command::new("pkexec")
                        .args(["rakuos", "remove", &id])
                        .output()
                        .ok();
                    if let Some(o) = &out {
                        append(&String::from_utf8_lossy(&o.stdout));
                    }
                    out.map(|o| o.status.success()).unwrap_or(false)
                }
            };

            shared.result.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    // ── System status ─────────────────────────────────────────────────────────

    loadSystemStatus: qt_method!(fn loadSystemStatus(&mut self) {
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let status = rakuos_updates::get_system_status();
            let json = serde_json::to_string(&status).unwrap_or_default();
            let _ = std::fs::write(log_path(), &json);
            shared.result.store(1, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    upgradeSystem: qt_method!(fn upgradeSystem(&mut self) {
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader, Write};
            use std::process::{Command, Stdio};
            let _ = std::fs::write(log_path(), "Starting system upgrade...\n");

            let append = |text: &str| {
                if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(log_path()) {
                    let _ = f.write_all(text.as_bytes());
                    let _ = f.write_all(b"\n");
                }
            };

            let mut child = match Command::new("pkexec")
                .args(["bootc", "upgrade"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    append(&e.to_string());
                    shared.result.store(2, Ordering::Relaxed);
                    shared.running.store(false, Ordering::Relaxed);
                    return;
                }
            };

            if let Some(stdout) = child.stdout.take() {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    // Parse layers progress: layers[N/M]
                    if let Some(pct) = parse_layers_progress(&line) {
                        shared.progress.store(pct, Ordering::Relaxed);
                    }
                    if !line.is_empty() { append(&line); }
                }
            }

            let ok = child.wait().map(|s| s.success()).unwrap_or(false);
            shared.result.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    // ── Settings ─────────────────────────────────────────────────────────────

    loadSettings: qt_method!(fn loadSettings(&mut self) -> QString {
        std::fs::read_to_string(settings_path())
            .unwrap_or_else(|_| serde_json::json!({
                "update_interval": 1440,
                "auto_check_packages": true,
                "auto_check_flatpak": true,
                "auto_check_image": true,
                "auto_check_appimages": true,
                "auto_update": false,
            }).to_string())
            .into()
    }),

    saveSettings: qt_method!(fn saveSettings(&mut self, json: QString) {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, json.to_string());
    }),
}

impl SoftwareBackend {
    fn get_shared(&mut self) -> Arc<SharedState> {
        if self.shared.is_none() {
            self.shared = Some(Arc::new(SharedState::default()));
        }
        self.shared.as_ref().unwrap().clone()
    }

    fn start_op(&mut self) {
        let s = self.get_shared();
        s.running.store(true, Ordering::Relaxed);
        s.result.store(0, Ordering::Relaxed);
        s.progress.store(0, Ordering::Relaxed);
        self.opRunning  = true;
        self.opResult   = 0;
        self.opProgress = 0;
        self.opStateChanged();
        let _ = std::fs::write(log_path(), "");
        self.logRevision = 0;
        self.logRevisionChanged();
    }
}

fn parse_layers_progress(line: &str) -> Option<i32> {
    let re = regex::Regex::new(r"layers\[(\d+)/(\d+)\]").ok()?;
    let caps = re.captures(line)?;
    let n: i32 = caps[1].parse().ok()?;
    let total: i32 = caps[2].parse().ok()?;
    if total > 0 { Some((n * 100 / total).min(100)) } else { None }
}
