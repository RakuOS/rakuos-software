// ui-qt/backend.rs — QObject backend exposed to QML

#![allow(non_snake_case)]

use qmetaobject::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

// ── Cache readiness flag (set by warmCache thread) ───────────────────────────
static CACHE_READY: AtomicBool = AtomicBool::new(false);

// ── Reviews async state (separate from main op so they don't conflict) ────────
static REVIEWS_RUNNING: AtomicBool = AtomicBool::new(false);
static REVIEW_SUBMIT_RUNNING: AtomicBool = AtomicBool::new(false);

fn reviews_log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("rakuos-reviews.json")
}

fn review_submit_log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("rakuos-review-submit.json")
}

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



fn show_flag_path() -> std::path::PathBuf {
    std::env::temp_dir().join("rakuos-software-show")
}

fn daemon_cache_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    std::path::PathBuf::from(home).join(".cache/rakuos/daemon-update-cache.json")
}

fn append_log(text: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(log_path()) {
        let _ = f.write_all(text.as_bytes());
        let _ = f.write_all(b"\n");
    }
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

    // Background update badge count (populated by daemon cache or startup check)
    pendingUpdateCount: qt_property!(i32; NOTIFY pendingUpdateCountChanged),
    pendingUpdateCountChanged: qt_signal!(),

    // Shared state between Rust threads and Qt
    shared: Option<Arc<SharedState>>,

    // ── Navigation ────────────────────────────────────────────────────────────

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
                let (picks, popular, updated, new) = rakuos_home::load_all().await;
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

    // ── Screenshot download / cache ───────────────────────────────────────────


    // ── Add-ons lookup (sync — appstream cache is already in memory) ──────────

    getAddons: qt_method!(fn getAddons(&mut self, app_id: QString, source_type: QString) -> QString {
        let app_id = app_id.to_string();
        let source_type = source_type.to_string();
        match rakuos_packages::get_addons_for_app(&app_id, &source_type) {
            Ok(addons) => QString::from(serde_json::to_string(&addons).unwrap_or_else(|_| "[]".into())),
            Err(_) => QString::from("[]"),
        }
    }),

    // ── Installed Flatpak runtimes/add-ons (sync) ────────────────────────────

    loadFlatpakRuntimes: qt_method!(fn loadFlatpakRuntimes(&mut self) -> QString {
        let runtimes = rakuos_packages::get_installed_flatpak_runtimes();
        QString::from(serde_json::to_string(&runtimes).unwrap_or_else(|_| "[]".to_string()))
    }),

    // ── Web App catalog ───────────────────────────────────────────────────────

    loadWebAppCatalog: qt_method!(fn loadWebAppCatalog(&mut self) {
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let apps = rakuos_webapps::get_catalog();
            let json = serde_json::to_string(&apps).unwrap_or_default();
            let _ = std::fs::write(log_path(), &json);
            shared.result.store(1, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    /// Install a user-defined custom web app.
    /// icon_source: local file path or HTTP(S) URL, or empty for no icon.
    installCustomWebApp: qt_method!(fn installCustomWebApp(
        &mut self,
        name:        QString,
        url:         QString,
        description: QString,
        category:    QString,
        icon_source: QString,
    ) {
        self.start_op();
        let shared = self.get_shared();
        let name        = name.to_string();
        let url         = url.to_string();
        let description = description.to_string();
        let category    = category.to_string();
        let icon_source = icon_source.to_string();
        std::thread::spawn(move || {
            let (ok, msg) = rakuos_webapps::install_custom(
                &name, &url, &description, &category, &icon_source,
            );
            append_log(&msg);
            shared.result.store(if ok { 1 } else { 0 }, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    // ── AppImage settings ─────────────────────────────────────────────────────

    // ── Local file install ────────────────────────────────────────────────────

    /// Store the startup file path (set from argv in main.rs before QML loads).
    startupFilePath: qt_property!(QString; NOTIFY startupFilePathChanged),
    startupFilePathChanged: qt_signal!(),

    /// Read the startup file path from the temp flag written by main.rs.
    /// QML calls this once at Component.onCompleted; clears the flag after reading.
    readStartupFilePath: qt_method!(fn readStartupFilePath(&mut self) -> QString {
        let flag = std::env::temp_dir().join("rakuos-software-open-file");
        if let Ok(path) = std::fs::read_to_string(&flag) {
            let _ = std::fs::remove_file(&flag);
            let path = path.trim().to_string();
            if !path.is_empty() {
                self.startupFilePath = path.clone().into();
                self.startupFilePathChanged();
                return path.into();
            }
        }
        "".into()
    }),

    /// Determine file kind from extension.
    fileKind: qt_method!(fn fileKind(&mut self, path: QString) -> QString {
        let p = path.to_string().to_lowercase();
        if p.ends_with(".rpm")        { return "rpm".into(); }
        if p.ends_with(".flatpak")    { return "flatpak".into(); }
        if p.ends_with(".flatpakref") { return "flatpakref".into(); }
        if p.ends_with(".appimage")   { return "appimage".into(); }
        "unknown".into()
    }),

    /// Get metadata for a local file. Returns JSON.
    getLocalFileInfo: qt_method!(fn getLocalFileInfo(&mut self, path: QString, kind: QString) -> QString {
        let path = path.to_string();
        let kind = kind.to_string();
        let info = match kind.as_str() {
            "rpm"        => rakuos_packages::get_local_rpm_info(&path),
            "flatpak"    => rakuos_flatpak::get_local_flatpak_info(&path),
            "flatpakref" => rakuos_flatpak::get_flatpakref_info(&path),
            "appimage"   => rakuos_appimages::get_appimage_info_for_display(&path),
            _            => serde_json::json!({"error": "Unknown file type"}),
        };
        info.to_string().into()
    }),

    /// Install a local file. Uses start_op pattern; poll opRunning/opResult.
    installLocalFile: qt_method!(fn installLocalFile(&mut self, path: QString, kind: QString, action: QString, pkg_name: QString) {
        let path     = path.to_string();
        let kind     = kind.to_string();
        let action   = action.to_string();
        let pkg_name = pkg_name.to_string();
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let _ = std::fs::write(log_path(), format!("Installing {}...\n", path));
            let ok = match kind.as_str() {
                "rpm" => {
                    let mut exit_code = 1i32;
                    let iter: Box<dyn Iterator<Item = String> + Send> = if action == "reinstall" {
                        Box::new(rakuos_packages::reinstall_local_rpm_stream(&pkg_name, &path))
                    } else {
                        Box::new(rakuos_packages::install_local_rpm_stream(&path))
                    };
                    for line in iter {
                        if let Some(code) = line.strip_prefix("__done__") {
                            exit_code = code.trim().parse().unwrap_or(1);
                        } else if !line.is_empty() {
                            append_log(&line);
                        }
                    }
                    exit_code == 0
                }
                "flatpak" => {
                    let mut exit_code = 1i32;
                    for line in rakuos_flatpak::install_local_bundle_stream(&path) {
                        if let Some(code) = line.strip_prefix("__done__") {
                            exit_code = code.trim().parse().unwrap_or(1);
                        } else if !line.is_empty() {
                            append_log(&line);
                        }
                    }
                    exit_code == 0
                }
                "flatpakref" => {
                    let mut exit_code = 1i32;
                    for line in rakuos_flatpak::install_flatpakref_stream(&path) {
                        if let Some(code) = line.strip_prefix("__done__") {
                            exit_code = code.trim().parse().unwrap_or(1);
                        } else if !line.is_empty() {
                            append_log(&line);
                        }
                    }
                    exit_code == 0
                }
                "appimage" => {
                    let (ok, msg, _) = rakuos_appimages::install_appimage(&path);
                    append_log(&msg);
                    ok
                }
                _ => { append_log("Unknown file type"); false }
            };
            shared.result.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    // ── AppImage settings ─────────────────────────────────────────────────────

    saveAppImageSettings: qt_method!(fn saveAppImageSettings(&mut self, id: QString, update_source: QString, update_url: QString, update_pattern: QString) -> QString {
        let (ok, msg) = rakuos_appimages::save_settings(
            &id.to_string(),
            &update_source.to_string(),
            &update_url.to_string(),
            &update_pattern.to_string(),
        );
        serde_json::json!({ "ok": ok, "msg": msg }).to_string().into()
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
            // Search AppImages
            let q_lower = query.to_lowercase();
            for a in rakuos_appimages::get_installed() {
                let name = a.name.to_lowercase();
                let summary = a.description.to_lowercase();
                if name.contains(&q_lower) || summary.contains(&q_lower) {
                    results.push(serde_json::to_value(a).unwrap_or_default());
                }
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

            // Check packages via rakuos-update script
            let pkg_out = Command::new("/usr/libexec/rakuos/rakuos-update")
                .arg("check")
                .output()
                .ok();

            let pkg_updates: Vec<serde_json::Value> = {
                let raw: Vec<serde_json::Value> = pkg_out
                    .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
                    .and_then(|v| v["updates"].as_array().cloned())
                    .unwrap_or_default();
                // Enrich with icon paths using the AppStream cache (same source as installed page)
                let appstream = rakuos_appstream::get_appstream();
                // Build pkg_name → (icon_path, icon_url) map.  Prefer entries with a
                // non-empty icon_path so that a flatpak entry with no local icon doesn't
                // shadow a native entry that has one (HashMap iteration order is undefined).
                let mut pkg_icons: std::collections::HashMap<String, (String, String)> =
                    std::collections::HashMap::new();
                for a in appstream.values().filter(|a| !a.package_name.is_empty()) {
                    let entry = pkg_icons.entry(a.package_name.clone())
                        .or_insert_with(|| (a.icon_path.clone(), a.icon_url.clone()));
                    if entry.0.is_empty() && entry.1.is_empty() {
                        *entry = (a.icon_path.clone(), a.icon_url.clone());
                    } else if entry.0.is_empty() && !a.icon_path.is_empty() {
                        entry.0 = a.icon_path.clone();
                    }
                }
                raw.into_iter().map(|mut v| {
                    if let Some(name) = v["name"].as_str().map(|s| s.to_string()) {
                        if let Some((ip, iu)) = pkg_icons.get(&name) {
                            if !ip.is_empty() { v["icon_path"] = ip.clone().into(); }
                            if !iu.is_empty() { v["icon_url"]  = iu.clone().into(); }
                        }
                    }
                    v
                }).collect()
            };

            // Use get_all_updates() so flatpak entries are icon-enriched via the
            // AppStream cache (same path as the installed page).  Direct script
            // parsing skips that enrichment and leaves icon_path empty.
            let fp_updates: Vec<serde_json::Value> = rakuos_flatpak::get_all_updates()
                .into_iter()
                .filter_map(|f| serde_json::to_value(f).ok())
                .collect();

            // Check image update — also detects staged images waiting for reboot.
            let (image_available, image_info) = rakuos_updates::check_image_script();
            let reboot_required = image_info["reboot_required"].as_bool().unwrap_or(false);

            let total = pkg_updates.len() + fp_updates.len()
                + if image_available { 1 } else { 0 };

            let result = serde_json::json!({
                "packages":        pkg_updates,
                "flatpak":         fp_updates,
                "appimages":       [],
                "image_available": image_available,
                "reboot_required": reboot_required,
                "image_info":      image_info,
                "total": total,
            });

            let result_json = serde_json::to_string_pretty(&result).unwrap_or_default();
            let _ = std::fs::write(log_path(), &result_json);

            // Write to daemon cache so the tray and update page share the same data
            let cache_path = daemon_cache_path();
            if let Some(parent) = cache_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&cache_path, &result_json);

            shared.result.store(1, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    // ── Image update ─────────────────────────────────────────────────────────

    upgradeImage: qt_method!(fn upgradeImage(&mut self, update_type: QString, repo_url: QString, new_tag: QString) {
        let update_type = update_type.to_string();
        let repo_url = repo_url.to_string();
        let new_tag = new_tag.to_string();
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let _ = std::fs::write(log_path(), "Starting image upgrade...\n");
            let mut exit_code = 1i32;
            for line in rakuos_updates::upgrade_image_stream(&update_type, &repo_url, &new_tag) {
                if let Some(code) = line.strip_prefix("__done__") {
                    exit_code = code.trim().parse().unwrap_or(1);
                } else {
                    if let Some(pct) = parse_layers_progress(&line) {
                        shared.progress.store(pct, Ordering::Relaxed);
                    }
                    if !line.is_empty() { append_log(&line); }
                }
            }
            shared.result.store(if exit_code == 0 { 1 } else { 2 }, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    rollbackSystem: qt_method!(fn rollbackSystem(&mut self) {
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let _ = std::fs::write(log_path(), "Rolling back...\n");
            let mut exit_code = 1i32;
            for line in rakuos_updates::rollback_stream() {
                if let Some(code) = line.strip_prefix("__done__") {
                    exit_code = code.trim().parse().unwrap_or(1);
                } else {
                    if !line.is_empty() { append_log(&line); }
                }
            }
            shared.result.store(if exit_code == 0 { 1 } else { 2 }, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    rebootSystem: qt_method!(fn rebootSystem(&mut self) {
        std::thread::spawn(|| {
            rakuos_updates::schedule_reboot();
        });
    }),

    getOverlayStatus: qt_method!(fn getOverlayStatus(&mut self) -> QString {
        let status = rakuos_updates::get_overlay_status();
        serde_json::to_string(&status).unwrap_or_default().into()
    }),

    upgradePackages: qt_method!(fn upgradePackages(&mut self) {
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let _ = std::fs::write(log_path(), "Upgrading overlay packages...\n");
            let mut exit_code = 1i32;
            for line in rakuos_updates::upgrade_packages_stream() {
                if let Some(code) = line.strip_prefix("__done__") {
                    exit_code = code.trim().parse().unwrap_or(1);
                } else {
                    // Parse [N/M] progress from dnf5 transaction output
                    if let Some(pct) = parse_layers_progress(&line) {
                        shared.progress.store(pct, Ordering::Relaxed);
                    }
                    if !line.is_empty() { append_log(&line); }
                }
            }
            shared.result.store(if exit_code == 0 { 1 } else { 2 }, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    upgradePackage: qt_method!(fn upgradePackage(&mut self, name: QString) {
        let name = name.to_string();
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let _ = std::fs::write(log_path(), format!("Upgrading package {}...\n", name));
            let mut exit_code = 1i32;
            for line in rakuos_updates::upgrade_single_package_stream(&name) {
                if let Some(code) = line.strip_prefix("__done__") {
                    exit_code = code.trim().parse().unwrap_or(1);
                } else {
                    if let Some(pct) = parse_layers_progress(&line) {
                        shared.progress.store(pct, Ordering::Relaxed);
                    }
                    if !line.is_empty() { append_log(&line); }
                }
            }
            shared.result.store(if exit_code == 0 { 1 } else { 2 }, Ordering::Relaxed);
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
            use std::io::{BufRead, BufReader};
            use std::process::{Command, Stdio};
            let _ = std::fs::write(log_path(), format!("Installing {}...\n", id));

            let (mut child, ok) = match source.as_str() {
                "flatpak" => {
                    // "__upgrade_all__" upgrades everything (including required new runtimes).
                    // Individual IDs may include a branch spec (e.g. "org.gnome.Platform//50")
                    // for new runtime installs — flatpak install handles both cases.
                    let mut cmd = Command::new("flatpak");
                    if id == "__upgrade_all__" {
                        cmd.args(["update", "--noninteractive", "-y"]);
                    } else {
                        cmd.args(["install", "--noninteractive", "-y", &id]);
                    }
                    let c = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
                    match c {
                        Ok(child) => (Some(child), true),
                        Err(e)    => { append_log(&e.to_string()); (None, false) }
                    }
                }
                "webapp" => {
                    let (ok, msg) = rakuos_webapps::install(&id);
                    append_log(&msg);
                    (None, ok)
                }
                _ => {
                    // Resolve AppStream ID → RPM package name
                    let appstream = rakuos_appstream::get_appstream();
                    let pkg = appstream.get(&id)
                        .or_else(|| appstream.get(&format!("native:{}", id)))
                        .map(|a| a.package_name.clone())
                        .filter(|p| !p.is_empty())
                        .unwrap_or_else(|| id.clone());
                    drop(appstream);
                    let c = Command::new("sudo")
                        .args(["/usr/libexec/rakuos/rakuos-install", &pkg])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn();
                    match c {
                        Ok(child) => (Some(child), true),
                        Err(e)    => { append_log(&e.to_string()); (None, false) }
                    }
                }
            };

            if let Some(ref mut child) = child {
                if let Some(stdout) = child.stdout.take() {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        if !line.is_empty() { append_log(&line); }
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

    resetOverlay: qt_method!(fn resetOverlay(&mut self, mode: QString) {
        let mode = mode.to_string();
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            use std::process::Command;
            let flag = if mode == "soft" { "--soft" } else { "--confirm" };
            let _ = std::fs::write(log_path(), format!("Running overlay reset ({mode})...\n"));
            let out = Command::new("pkexec")
                .args(["/usr/libexec/rakuos/rakuos-reset-overlay", flag])
                .output();
            match out {
                Ok(o) => {
                    let msg = String::from_utf8_lossy(&o.stdout).to_string()
                        + &String::from_utf8_lossy(&o.stderr);
                    let _ = std::fs::write(log_path(), msg);
                    shared.result.store(if o.status.success() { 1 } else { 2 }, Ordering::Relaxed);
                }
                Err(e) => {
                    let _ = std::fs::write(log_path(), e.to_string());
                    shared.result.store(2, Ordering::Relaxed);
                }
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
            use std::process::Command;
            let _ = std::fs::write(log_path(), format!("Removing {}...\n", id));

            let ok = match source.as_str() {
                "flatpak" => {
                    let out = Command::new("flatpak")
                        .args(["uninstall", "--noninteractive", "-y", &id])
                        .output()
                        .ok();
                    if let Some(o) = &out {
                        append_log(&String::from_utf8_lossy(&o.stdout));
                    }
                    out.map(|o| o.status.success()).unwrap_or(false)
                }
                "flatpak-runtime" => {
                    // Use --force-remove so runtimes with dependent apps can still be removed
                    let out = Command::new("flatpak")
                        .args(["uninstall", "--noninteractive", "-y", "--force-remove", &id])
                        .output()
                        .ok();
                    if let Some(o) = &out {
                        append_log(&String::from_utf8_lossy(&o.stdout));
                    }
                    out.map(|o| o.status.success()).unwrap_or(false)
                }
                "webapp" => {
                    let (ok, msg) = rakuos_webapps::uninstall(&id);
                    append_log(&msg);
                    ok
                }
                "appimage" => {
                    let (ok, msg) = rakuos_appimages::uninstall(&id);
                    append_log(&msg);
                    ok
                }
                _ => {
                    // Resolve AppStream ID → RPM package name
                    let appstream = rakuos_appstream::get_appstream();
                    let pkg = appstream.get(&id)
                        .or_else(|| appstream.get(&format!("native:{}", id)))
                        .map(|a| a.package_name.clone())
                        .filter(|p| !p.is_empty())
                        .unwrap_or_else(|| id.clone());
                    drop(appstream);
                    let out = Command::new("sudo")
                        .args(["/usr/libexec/rakuos/rakuos-remove", &pkg])
                        .output()
                        .ok();
                    if let Some(o) = &out {
                        append_log(&String::from_utf8_lossy(&o.stdout));
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
            let overlay = rakuos_updates::get_overlay_status();
            let result = serde_json::json!({
                "image":     status.image,
                "version":   status.version,
                "digest":    status.digest,
                "timestamp": status.timestamp,
                "error":     status.error,
                "overlay_packages": overlay.packages,
                "overlay_count": overlay.package_count,
                "overlay_dirty": overlay.is_dirty,
            });
            let json = serde_json::to_string(&result).unwrap_or_default();
            let _ = std::fs::write(log_path(), &json);
            shared.result.store(1, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    upgradeSystem: qt_method!(fn upgradeSystem(&mut self) {
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            use std::process::{Command, Stdio};
            let _ = std::fs::write(log_path(), "Starting system upgrade...\n");

            let mut child = match Command::new("pkexec")
                .args(["bootc", "upgrade"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    append_log(&e.to_string());
                    shared.result.store(2, Ordering::Relaxed);
                    shared.running.store(false, Ordering::Relaxed);
                    return;
                }
            };

            if let Some(stdout) = child.stdout.take() {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if let Some(pct) = parse_layers_progress(&line) {
                        shared.progress.store(pct, Ordering::Relaxed);
                    }
                    if !line.is_empty() { append_log(&line); }
                }
            }

            let ok = child.wait().map(|s| s.success()).unwrap_or(false);
            shared.result.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    // ── Flatpak remote management ─────────────────────────────────────────────

    getFlatpakRemotes: qt_method!(fn getFlatpakRemotes(&mut self) -> QString {
        let remotes = rakuos_flatpak::get_remotes();
        let has_flathub = rakuos_flatpak::has_flathub();
        serde_json::json!({ "remotes": remotes, "has_flathub": has_flathub })
            .to_string().into()
    }),

    addFlatpakRemote: qt_method!(fn addFlatpakRemote(&mut self, name: QString, url: QString, system: bool) -> QString {
        let (ok, msg) = rakuos_flatpak::add_remote(&name.to_string(), &url.to_string(), system);
        serde_json::json!({ "ok": ok, "msg": msg }).to_string().into()
    }),

    addFlathub: qt_method!(fn addFlathub(&mut self, system: bool) -> QString {
        let (ok, msg) = rakuos_flatpak::add_flathub(system);
        serde_json::json!({ "ok": ok, "msg": msg }).to_string().into()
    }),

    removeFlatpakRemote: qt_method!(fn removeFlatpakRemote(&mut self, name: QString, system: bool) -> QString {
        let (ok, msg) = rakuos_flatpak::remove_remote(&name.to_string(), system);
        serde_json::json!({ "ok": ok, "msg": msg }).to_string().into()
    }),

    setFlatpakRemoteEnabled: qt_method!(fn setFlatpakRemoteEnabled(&mut self, name: QString, enabled: bool, system: bool) -> QString {
        let (ok, msg) = rakuos_flatpak::set_remote_enabled(&name.to_string(), enabled, system);
        serde_json::json!({ "ok": ok, "msg": msg }).to_string().into()
    }),

    // ── Firmware management ───────────────────────────────────────────────────

    getFirmwareData: qt_method!(fn getFirmwareData(&mut self) -> QString {
        let available = rakuos_firmware::fwupd_available();
        let remotes     = if available { rakuos_firmware::get_remotes()     } else { vec![] };
        let vendor_dirs = if available { rakuos_firmware::get_vendor_dirs() } else { vec![] };
        let updates     = if available { rakuos_firmware::get_updates()     } else { vec![] };
        serde_json::json!({
            "available":   available,
            "remotes":     remotes,
            "vendor_dirs": vendor_dirs,
            "updates":     updates,
        }).to_string().into()
    }),

    setFirmwareRemoteEnabled: qt_method!(fn setFirmwareRemoteEnabled(&mut self, id: QString, enabled: bool) -> QString {
        let (ok, msg) = rakuos_firmware::set_remote_enabled(&id.to_string(), enabled);
        serde_json::json!({ "ok": ok, "msg": msg }).to_string().into()
    }),

    refreshFirmware: qt_method!(fn refreshFirmware(&mut self) {
        self.start_op();
        let shared = self.get_shared();
        std::thread::spawn(move || {
            let _ = std::fs::write(log_path(), "Refreshing firmware metadata...\n");
            let mut exit_code = 1i32;
            for line in rakuos_firmware::refresh_stream() {
                if let Some(code) = line.strip_prefix("__done__") {
                    exit_code = code.trim().parse().unwrap_or(1);
                } else {
                    if !line.is_empty() { append_log(&line); }
                }
            }
            shared.result.store(if exit_code == 0 { 1 } else { 2 }, Ordering::Relaxed);
            shared.running.store(false, Ordering::Relaxed);
        });
    }),

    checkFirmwareUpdates: qt_method!(fn checkFirmwareUpdates(&mut self) -> QString {
        let updates = rakuos_firmware::get_updates();
        serde_json::to_string(&updates).unwrap_or_default().into()
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

    // ── Startup helpers ───────────────────────────────────────────────────────

    // Pre-warm the appstream cache in a background thread so first searches
    // and category loads are instant.
    warmCache: qt_method!(fn warmCache(&mut self) {
        std::thread::spawn(|| {
            let _ = rakuos_appstream::get_appstream();
            CACHE_READY.store(true, Ordering::Release);
            log::info!("AppStream cache warmed.");
        });
    }),

    isCacheReady: qt_method!(fn isCacheReady(&mut self) -> bool {
        CACHE_READY.load(Ordering::Acquire)
    }),

    // Check if main.rs wrote a "start hidden" flag (launched with --tray).
    // Returns true once (deletes the flag) so QML knows to stay hidden at startup.
    readStartHidden: qt_method!(fn readStartHidden(&mut self) -> bool {
        let flag = std::env::temp_dir().join("rakuos-software-start-hidden");
        if flag.exists() {
            let _ = std::fs::remove_file(&flag);
            return true;
        }
        false
    }),

    // Check if the tray daemon wrote a "show window" flag.
    // Returns true once (deletes the flag) so QML can show + activate the window.
    checkShowRequest: qt_method!(fn checkShowRequest(&mut self) -> bool {
        let flag = show_flag_path();
        if flag.exists() {
            let _ = std::fs::remove_file(&flag);
            return true;
        }
        false
    }),

    // Check if the tray daemon wrote a "quit" flag.
    // Returns true once (deletes the flag) so QML can call Qt.quit().
    checkQuitRequest: qt_method!(fn checkQuitRequest(&mut self) -> bool {
        let flag = std::env::temp_dir().join("rakuos-software-quit");
        if flag.exists() {
            let _ = std::fs::remove_file(&flag);
            return true;
        }
        false
    }),

    // Read cached update count written by the daemon's last background check.
    // Populates pendingUpdateCount so the UI can show a badge without
    // blocking on a fresh check.
    loadDaemonUpdateCache: qt_method!(fn loadDaemonUpdateCache(&mut self) {
        let path = daemon_cache_path();
        if let Ok(json) = std::fs::read_to_string(&path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) {
                let count = val["total"].as_i64().unwrap_or(0) as i32;
                if self.pendingUpdateCount != count {
                    self.pendingUpdateCount = count;
                    self.pendingUpdateCountChanged();
                }
            }
        }
    }),

    // Return the raw JSON from the daemon's update cache, or "" if not yet
    // written (daemon still running its first check).
    loadUpdatesCache: qt_method!(fn loadUpdatesCache(&mut self) -> QString {
        std::fs::read_to_string(daemon_cache_path())
            .unwrap_or_default()
            .into()
    }),

    // Returns true if the daemon wrote a check-trigger flag (and consumes it).
    // The UI calls this on a timer and runs checkUpdates() when true.
    checkDaemonTrigger: qt_method!(fn checkDaemonTrigger(&mut self) -> bool {
        let path = std::env::temp_dir().join("rakuos-software-check-requested");
        if path.exists() {
            let _ = std::fs::remove_file(&path);
            true
        } else {
            false
        }
    }),

    // ── ODRS Reviews ──────────────────────────────────────────────────────────

    /// Start fetching ODRS reviews for app_id in a background thread.
    loadReviews: qt_method!(fn loadReviews(&mut self, app_id: QString) {
        let app_id = app_id.to_string();
        REVIEWS_RUNNING.store(true, Ordering::Relaxed);
        let _ = std::fs::write(reviews_log_path(), "[]");
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let reviews = rt.block_on(rakuos_reviews::get_reviews(&app_id));
            let json = serde_json::to_string(&reviews).unwrap_or_else(|_| "[]".into());
            let _ = std::fs::write(reviews_log_path(), json);
            REVIEWS_RUNNING.store(false, Ordering::Relaxed);
        });
    }),

    /// Returns true while reviews are being fetched.
    reviewsRunning: qt_method!(fn reviewsRunning(&mut self) -> bool {
        REVIEWS_RUNNING.load(Ordering::Relaxed)
    }),

    /// Read the cached reviews JSON (array of Review objects).
    readReviews: qt_method!(fn readReviews(&mut self) -> QString {
        std::fs::read_to_string(reviews_log_path())
            .unwrap_or_else(|_| "[]".into())
            .into()
    }),

    /// Submit a review asynchronously.  Poll reviewSubmitRunning(); when false
    /// call readReviewSubmitResult() for {"ok":bool,"msg":string}.
    submitReview: qt_method!(fn submitReview(
        &mut self,
        app_id: QString, summary: QString,
        description: QString, rating: i32, version: QString,
        display_name: QString
    ) {
        let app_id       = app_id.to_string();
        let summary      = summary.to_string();
        let description  = description.to_string();
        let version      = version.to_string();
        let display_name = display_name.to_string();
        let user_display: Option<String> = if display_name.is_empty() { None } else { Some(display_name) };
        REVIEW_SUBMIT_RUNNING.store(true, Ordering::Relaxed);
        let _ = std::fs::write(review_submit_log_path(), "");
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(rakuos_reviews::submit_review(
                &app_id, &summary, &description, rating, &version, user_display.as_deref(),
            ));
            let json = match result {
                Ok(())   => serde_json::json!({"ok": true,  "msg": "Review submitted — thank you!"}).to_string(),
                Err(msg) => serde_json::json!({"ok": false, "msg": msg}).to_string(),
            };
            let _ = std::fs::write(review_submit_log_path(), json);
            REVIEW_SUBMIT_RUNNING.store(false, Ordering::Relaxed);
        });
    }),

    reviewSubmitRunning: qt_method!(fn reviewSubmitRunning(&mut self) -> bool {
        REVIEW_SUBMIT_RUNNING.load(Ordering::Relaxed)
    }),

    readReviewSubmitResult: qt_method!(fn readReviewSubmitResult(&mut self) -> QString {
        std::fs::read_to_string(review_submit_log_path())
            .unwrap_or_default()
            .into()
    }),

    /// Vote on a review — fire and forget.
    voteReview: qt_method!(fn voteReview(&mut self, review_id: i32, upvote: bool) {
        rakuos_reviews::vote_review(review_id as i64, upvote);
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
        s.progress.store(2, Ordering::Relaxed);  // start at 2 so bar is immediately visible
        self.opRunning  = true;
        self.opResult   = 0;
        self.opProgress = 2;
        self.opStateChanged();
        let _ = std::fs::write(log_path(), "");
        self.logRevision = 0;
        self.logRevisionChanged();
    }
}

fn parse_layers_progress(line: &str) -> Option<i32> {
    // Match various formats bootc/ostree/skopeo may emit:
    //   "layers[3/50]"   "layers [3/50]"   "[3/50]"   "3/50 layers"
    //   "Fetching 3 of 50"   "Pulling layer 3/50"
    let patterns: &[&str] = &[
        r"layers?\s*\[(\d+)/(\d+)\]",
        r"\[(\d+)/(\d+)\]",
        r"(\d+)\s*/\s*(\d+)\s+layers?",
        r"(\d+)\s+of\s+(\d+)",
        r"layer[s]?\s+(\d+)/(\d+)",
    ];
    for pat in patterns {
        if let Some(caps) = regex::Regex::new(pat).ok()?.captures(line) {
            let n: i32 = caps[1].parse().ok()?;
            let total: i32 = caps[2].parse().ok()?;
            if total > 0 { return Some((n * 100 / total).min(100)); }
        }
    }
    None
}
