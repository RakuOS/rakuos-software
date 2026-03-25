// rakuos-appstream — AppStream metadata parsing and caching
//
// Reads compressed AppStream XML from /usr/share/swcatalog/xml/*.xml.gz
// and the overrides from /usr/share/rakuos/appstream/appstream-overrides.json.
// Mirrors the logic in src/backend/packages.py (_load_appstream).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

// ── Global in-memory cache ────────────────────────────────────────────────────
// Parsed once on first call, shared across all callers via Arc (no re-parsing).

static APPSTREAM_CACHE: OnceLock<Arc<HashMap<String, AppInfo>>> = OnceLock::new();

/// Returns a reference-counted handle to the shared appstream data.
/// First call parses all XML files; subsequent calls return in nanoseconds.
pub fn get_appstream() -> Arc<HashMap<String, AppInfo>> {
    APPSTREAM_CACHE
        .get_or_init(|| Arc::new(load_appstream_inner().unwrap_or_default()))
        .clone()
}

/// Compatibility wrapper — prefer get_appstream() to avoid cloning the map.
pub fn load_appstream() -> Result<HashMap<String, AppInfo>> {
    Ok((*get_appstream()).clone())
}

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub icon: String,
    pub icon_url: String,
    pub icon_path: String,        // resolved local filesystem path (empty if not found)
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub screenshots: Vec<String>,
    pub source: String,           // "native", "flatpak", "terra", etc.
    pub package_name: String,
    pub version: String,
    pub developer: String,
    pub url_homepage: String,
    pub license: String,
    pub content_rating: String,
    pub is_addon: bool,
    pub extends: String,          // parent app id for addons
    pub pkg_name_guessed: bool,   // true when package_name was guessed from id, not from <pkgname>
}

// ── Paths ─────────────────────────────────────────────────────────────────────

const SWCATALOG_DIR: &str = "/usr/share/swcatalog/xml";
const RAKUOS_APPSTREAM_DIR: &str = "/usr/share/rakuos/appstream/data";
const APP_INFO_XMLS: &str = "/usr/share/app-info/xmls";
const APP_INFO_XMLS_CACHE: &str = "/var/cache/app-info/xmls";
const OVERRIDES_PATH: &str = "/usr/share/rakuos/appstream/appstream-overrides.json";
const FLATPAK_TO_RPM_PATH: &str = "/usr/share/rakuos/appstream/flatpak-to-rpm.json";

// Icon search directories — matched to Python's ICON_DIRS list
const STATIC_ICON_DIRS: &[&str] = &[
    "/usr/share/rakuos/appstream/icons",
    "/usr/share/swcatalog/icons/fedora/64x64",
    "/usr/share/swcatalog/icons/fedora/128x128",
    "/usr/share/swcatalog/icons/rpmfusion-free-43/64x64",
    "/usr/share/swcatalog/icons/rpmfusion-nonfree-43/64x64",
    "/var/lib/flatpak/appstream/flathub/x86_64/active/icons/128x128",
    "/var/lib/flatpak/appstream/flathub/x86_64/active/icons/64x64",
    "/var/lib/flatpak/appstream/fedora-flatpaks/x86_64/active/icons/64x64",
    "/usr/share/app-info/icons",
    "/usr/share/pixmaps",
];

// Terra icon dirs are version-specific — we build them at runtime
fn terra_icon_dirs(fedora_ver: &str) -> Vec<String> {
    let suffixes = ["", "-mesa", "-nvidia", "-extras", "-multimedia"];
    let sizes = ["64x64", "128x128"];
    let mut dirs = Vec::new();
    for suffix in &suffixes {
        for size in &sizes {
            dirs.push(format!(
                "/usr/share/swcatalog/icons/terra{}{}/{}",
                fedora_ver, suffix, size
            ));
        }
    }
    dirs
}

fn get_fedora_version() -> String {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("VERSION_ID=") {
                return rest.trim_matches('"').to_string();
            }
        }
    }
    "43".to_string()
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Returns true if a path looks like an AppStream XML file (.xml or .xml.gz).
fn is_appstream_file(p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.ends_with(".xml.gz") || name.ends_with(".xml")
}

/// Recursively scan a directory tree for AppStream XML files (.xml and .xml.gz).
/// Stops recursing at `max_depth` to avoid traversing huge trees.
fn scan_appstream_files(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth == 0 || !dir.exists() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            scan_appstream_files(&p, out, depth - 1);
        } else if is_appstream_file(&p) {
            out.push(p);
        }
    }
}

/// Internal: parse all AppStream XML files from disk. Called at most once.
fn load_appstream_inner() -> Result<HashMap<String, AppInfo>> {
    let mut apps: HashMap<String, AppInfo> = HashMap::new();
    let mut xml_files: Vec<PathBuf> = Vec::new();

    // RakuOS custom entries (highest priority, plain .xml)
    scan_appstream_files(Path::new(RAKUOS_APPSTREAM_DIR), &mut xml_files, 3);

    // System swcatalog (native + terra; mix of .xml and .xml.gz)
    scan_appstream_files(Path::new(SWCATALOG_DIR), &mut xml_files, 2);

    // app-info dirs
    scan_appstream_files(Path::new(APP_INFO_XMLS), &mut xml_files, 2);
    scan_appstream_files(Path::new(APP_INFO_XMLS_CACHE), &mut xml_files, 2);

    // Flatpak AppStream: system-wide (flathub/x86_64/active/ is 3 levels deep)
    scan_appstream_files(Path::new("/var/lib/flatpak/appstream"), &mut xml_files, 5);

    // Flatpak AppStream: per-user
    if let Ok(home) = std::env::var("HOME") {
        let user_fp = PathBuf::from(home).join(".local/share/flatpak/appstream");
        scan_appstream_files(&user_fp, &mut xml_files, 5);
    }

    // Prefer .xml.gz over .xml when both exist for the same base name
    // (flatpak ships both; prefer the gz to avoid double-parsing)
    let xml_files = dedup_prefer_gz(xml_files);

    for path in &xml_files {
        if let Err(e) = parse_catalog_file(path, &mut apps) {
            log::warn!("Failed to parse {:?}: {}", path, e);
        }
    }

    // Apply overrides
    apply_overrides(&mut apps);

    // Inject native stubs from flatpak-to-rpm.json:
    // Many apps (Firefox, VLC, etc.) have no native RPM AppStream on Fedora but
    // have rich Flatpak AppStream — inject a native stub so users can install
    // the RPM with full metadata (name, icon, description, screenshots).
    inject_native_stubs(&mut apps);

    // Resolve icon paths
    let fedora_ver = get_fedora_version();
    let terra_dirs = terra_icon_dirs(&fedora_ver);
    let home_icon_cache = std::env::var("HOME")
        .map(|h| format!("{}/.cache/rakuos/icons", h))
        .unwrap_or_default();

    let app_ids: Vec<String> = apps.keys().cloned().collect();
    for id in app_ids {
        let app = apps.get_mut(&id).unwrap();
        if app.icon_path.is_empty() {
            app.icon_path = resolve_icon_path(
                app,
                &home_icon_cache,
                &terra_dirs,
            );
        }
    }

    log::info!(
        "AppStream: loaded {} apps from {} files",
        apps.len(),
        xml_files.len()
    );
    Ok(apps)
}

/// When a directory ships both `appstream.xml` and `appstream.xml.gz`,
/// keep only the `.gz` to avoid parsing the same data twice.
fn dedup_prefer_gz(files: Vec<PathBuf>) -> Vec<PathBuf> {
    use std::collections::HashSet;
    // Collect all .gz base paths (without the .gz suffix) for fast lookup
    let gz_bases: HashSet<PathBuf> = files
        .iter()
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("gz"))
        .filter_map(|p| {
            // "appstream.xml.gz" → strip ".gz" → "appstream.xml"
            let s = p.to_str()?;
            Some(PathBuf::from(s.strip_suffix(".gz")?))
        })
        .collect();

    files
        .into_iter()
        .filter(|p| {
            // If this is a plain .xml and a .gz version exists, skip it
            let ext = p.extension().and_then(|e| e.to_str());
            if ext == Some("xml") && gz_bases.contains(p.as_path()) {
                return false;
            }
            true
        })
        .collect()
}

/// Resolve the local icon path for an app, checking all icon dirs.
/// Returns empty string if not found.
pub fn resolve_icon_path(app: &AppInfo, home_icon_cache: &str, terra_dirs: &[String]) -> String {
    let app_id = &app.id;
    let source = &app.source;

    // For Flatpak apps, icon is always named {app_id}.png
    // Check in the Flatpak icon cache dirs first
    if source == "flatpak" {
        let candidates = [
            format!("/var/lib/flatpak/appstream/flathub/x86_64/active/icons/128x128/{}.png", app_id),
            format!("/var/lib/flatpak/appstream/flathub/x86_64/active/icons/64x64/{}.png", app_id),
            format!("/var/lib/flatpak/appstream/fedora-flatpaks/x86_64/active/icons/64x64/{}.png", app_id),
        ];
        for p in &candidates {
            if Path::new(p).exists() {
                return p.clone();
            }
        }
        // Check home icon cache (downloaded from Flathub API)
        if !home_icon_cache.is_empty() {
            let cached = format!("{}/{}.png", home_icon_cache, app_id);
            if Path::new(&cached).exists() {
                return cached;
            }
        }
    }

    // For all sources, try the icon field (could be filename like "gimp.png")
    if !app.icon.is_empty() {
        let icon = &app.icon;
        let stem = icon.trim_end_matches(".png")
            .trim_end_matches(".svg")
            .trim_end_matches(".xpm");

        // Build search dirs: static + terra
        let mut search_dirs: Vec<&str> = STATIC_ICON_DIRS.to_vec();
        let terra_refs: Vec<&str> = terra_dirs.iter().map(|s| s.as_str()).collect();
        search_dirs.extend_from_slice(&terra_refs);

        for dir in &search_dirs {
            for candidate in &[
                icon.clone(),
                format!("{}.png", stem),
                format!("{}.svg", stem),
            ] {
                let path = format!("{}/{}", dir, candidate);
                if Path::new(&path).exists() {
                    return path;
                }
            }
        }
    }

    // Home icon cache fallback
    if !home_icon_cache.is_empty() {
        let cached = format!("{}/{}.png", home_icon_cache, app_id);
        if Path::new(&cached).exists() {
            return cached;
        }
    }

    String::new()
}

/// Load the flatpak-to-rpm mapping (Flatpak app id → RPM package name).
pub fn load_flatpak_to_rpm() -> HashMap<String, String> {
    let path = Path::new(FLATPAK_TO_RPM_PATH);
    if !path.exists() {
        return HashMap::new();
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn parse_catalog_file(path: &Path, apps: &mut HashMap<String, AppInfo>) -> Result<()> {
    use flate2::read::GzDecoder;
    use quick_xml::Reader;
    use quick_xml::events::Event;

    // Determine source from the full path (not just filename — flatpak ships
    // a generic "appstream.xml.gz" that only reveals its origin via directory)
    let path_str = path.to_string_lossy().to_lowercase();
    let source = if path_str.contains("flathub") || path_str.contains("flatpak") {
        "flatpak"
    } else if path_str.contains("terra") {
        "terra"
    } else {
        "native"
    };

    let file = File::open(path)?;
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let mut reader = if file_name.ends_with(".xml.gz") {
        let gz = GzDecoder::new(BufReader::new(file));
        Reader::from_reader(BufReader::new(Box::new(gz) as Box<dyn std::io::Read>))
    } else {
        // Plain .xml
        Reader::from_reader(BufReader::new(Box::new(file) as Box<dyn std::io::Read>))
    };
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current: Option<AppInfo> = None;
    let mut in_id = false;
    let mut in_name = false;
    let mut in_summary = false;
    let mut in_description = false;
    let mut in_pkgname = false;
    let mut in_developer = false;
    let mut in_category = false;
    let mut in_keyword = false;
    let mut in_icon_cached = false;
    let mut in_icon_remote = false;
    let mut in_screenshot_image = false;
    let mut in_url_homepage = false;
    let mut in_extends = false;
    let mut lang_ok = true;
    let mut comp_type = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"component" => {
                        let kind = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| a.key.as_ref() == b"type")
                            .map(|a| {
                                String::from_utf8_lossy(&a.value).to_lowercase()
                            })
                            .unwrap_or_default();
                        comp_type = kind;
                        current = Some(AppInfo {
                            source: source.to_string(),
                            is_addon: comp_type == "addon",
                            ..Default::default()
                        });
                    }
                    b"id" if current.is_some() => in_id = true,
                    b"name" if current.is_some() => {
                        lang_ok = !e.attributes().any(|a| {
                            a.map(|a| a.key.as_ref() == b"xml:lang").unwrap_or(false)
                        });
                        in_name = lang_ok;
                    }
                    b"summary" if current.is_some() => {
                        lang_ok = !e.attributes().any(|a| {
                            a.map(|a| a.key.as_ref() == b"xml:lang").unwrap_or(false)
                        });
                        in_summary = lang_ok;
                    }
                    b"description" if current.is_some() => {
                        lang_ok = !e.attributes().any(|a| {
                            a.map(|a| a.key.as_ref() == b"xml:lang").unwrap_or(false)
                        });
                        in_description = lang_ok;
                    }
                    b"pkgname" if current.is_some() => in_pkgname = true,
                    b"developer" | b"developer_name" if current.is_some() => {
                        lang_ok = !e.attributes().any(|a| {
                            a.map(|a| a.key.as_ref() == b"xml:lang").unwrap_or(false)
                        });
                        in_developer = lang_ok;
                    }
                    b"icon" if current.is_some() => {
                        in_icon_cached = e.attributes().any(|a| {
                            a.map(|a| {
                                a.key.as_ref() == b"type" && a.value.as_ref() == b"cached"
                            })
                            .unwrap_or(false)
                        });
                        in_icon_remote = e.attributes().any(|a| {
                            a.map(|a| {
                                a.key.as_ref() == b"type" && a.value.as_ref() == b"remote"
                            })
                            .unwrap_or(false)
                        });
                    }
                    b"category" if current.is_some() => in_category = true,
                    b"keyword" if current.is_some() => {
                        lang_ok = !e.attributes().any(|a| {
                            a.map(|a| a.key.as_ref() == b"xml:lang").unwrap_or(false)
                        });
                        in_keyword = lang_ok;
                    }
                    b"image" if current.is_some() => {
                        // Only capture screenshot images, not icon images
                        in_screenshot_image = true;
                    }
                    b"url" if current.is_some() => {
                        in_url_homepage = e.attributes().any(|a| {
                            a.map(|a| {
                                a.key.as_ref() == b"type" && a.value.as_ref() == b"homepage"
                            })
                            .unwrap_or(false)
                        });
                    }
                    b"extends" if current.is_some() => in_extends = true,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if let Some(app) = current.as_mut() {
                    if in_id && app.id.is_empty() {
                        app.id = text;
                        in_id = false;
                    } else if in_name && app.name.is_empty() {
                        app.name = text;
                        in_name = false;
                    } else if in_summary && app.summary.is_empty() {
                        app.summary = text;
                        in_summary = false;
                    } else if in_description && app.description.is_empty() {
                        app.description = text;
                        in_description = false;
                    } else if in_pkgname && app.package_name.is_empty() {
                        app.package_name = text;
                        in_pkgname = false;
                    } else if in_developer && app.developer.is_empty() {
                        app.developer = text;
                        in_developer = false;
                    } else if in_icon_cached && app.icon.is_empty() {
                        app.icon = text;
                        in_icon_cached = false;
                    } else if in_icon_remote && app.icon_url.is_empty() {
                        app.icon_url = text;
                        in_icon_remote = false;
                    } else if in_category {
                        app.categories.push(text);
                        in_category = false;
                    } else if in_keyword {
                        app.keywords.push(text);
                        in_keyword = false;
                    } else if in_screenshot_image && !text.is_empty() {
                        // Only add http URLs (not local paths)
                        if text.starts_with("http") {
                            app.screenshots.push(text);
                        }
                        in_screenshot_image = false;
                    } else if in_url_homepage && app.url_homepage.is_empty() {
                        app.url_homepage = text;
                        in_url_homepage = false;
                    } else if in_extends && app.extends.is_empty() {
                        app.extends = text;
                        in_extends = false;
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"component" {
                    if let Some(mut app) = current.take() {
                        if !app.id.is_empty() {
                            // For Flatpak apps: icon is always {app_id}.png
                            if app.source == "flatpak" {
                                app.icon = format!("{}.png", app.id);
                                if app.package_name.is_empty() {
                                    app.package_name = app.id.clone();
                                }
                            } else if app.package_name.is_empty() {
                                // Native with no <pkgname> — guess from last segment of id
                                let clean = app.id.trim_end_matches(".desktop");
                                let guess = clean.split('.').last().unwrap_or(clean).to_lowercase();
                                app.package_name = guess;
                                app.pkg_name_guessed = true;
                            }
                            let existing = apps.get(&app.id);
                            match existing {
                                None => {
                                    apps.insert(app.id.clone(), app);
                                }
                                Some(ex) if ex.source == "native" && app.source == "flatpak" => {
                                    // Keep native metadata; enrich with flatpak screenshots/icon if missing
                                    let app_id = app.id.clone();
                                    let ex = apps.get_mut(&app_id).unwrap();
                                    if ex.screenshots.is_empty() && !app.screenshots.is_empty() {
                                        ex.screenshots = app.screenshots.clone();
                                    }
                                    if ex.icon.is_empty() && !app.icon.is_empty() {
                                        ex.icon = app.icon.clone();
                                    }
                                    // Also store flatpak version under its own key
                                    apps.insert(format!("flatpak:{}", app.id), app);
                                }
                                _ => {
                                    apps.insert(app.id.clone(), app);
                                }
                            }
                        }
                    }
                }
                in_id = false;
                in_name = false;
                in_summary = false;
                in_description = false;
                in_pkgname = false;
                in_developer = false;
                in_category = false;
                in_keyword = false;
                in_icon_cached = false;
                in_icon_remote = false;
                in_screenshot_image = false;
                in_url_homepage = false;
                in_extends = false;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                log::warn!("XML parse error in {:?}: {}", path, e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(())
}

/// Inject native install stubs for apps in flatpak-to-rpm.json.
/// On Fedora, many popular apps (Firefox, VLC, etc.) have rich Flatpak AppStream
/// but sparse or missing native RPM AppStream. This borrows the Flatpak metadata
/// and presents a native install option alongside the Flatpak one.
fn inject_native_stubs(apps: &mut HashMap<String, AppInfo>) {
    let fp_to_rpm = load_flatpak_to_rpm();
    if fp_to_rpm.is_empty() {
        return;
    }

    let mut stubs: Vec<(String, AppInfo)> = Vec::new();

    for (fp_id_lower, rpm_name) in &fp_to_rpm {
        // Find the flatpak entry by case-insensitive ID match
        let fp_entry = apps.values().find(|a| {
            a.source == "flatpak" && a.id.to_lowercase() == *fp_id_lower
        }).cloned();

        let Some(fp_entry) = fp_entry else { continue };

        // Skip if a native entry already exists with this rpm_name as package_name
        let already_exists = apps.values().any(|a| {
            a.source != "flatpak" && a.package_name == *rpm_name
        });
        if already_exists {
            continue;
        }

        // Build the native stub from Flatpak metadata
        let mut stub = fp_entry.clone();
        stub.source = "native".to_string();
        stub.package_name = rpm_name.clone();
        stub.id = rpm_name.clone();
        stub.pkg_name_guessed = false; // explicit mapping, not guessed
        stubs.push((format!("native:{}", rpm_name), stub));
    }

    for (key, stub) in stubs {
        apps.insert(key, stub);
    }
}

fn apply_overrides(apps: &mut HashMap<String, AppInfo>) {
    let path = Path::new(OVERRIDES_PATH);
    if !path.exists() {
        return;
    }
    let Ok(content) = std::fs::read_to_string(path) else { return };
    let Ok(overrides): Result<HashMap<String, serde_json::Value>, _> =
        serde_json::from_str(&content)
    else {
        return;
    };

    for (id, patch) in overrides {
        // Skip comment keys
        if id.starts_with('_') {
            continue;
        }
        // Match both plain id and flatpak: prefixed key
        for key in [id.clone(), format!("flatpak:{}", id)] {
            if let Some(app) = apps.get_mut(&key) {
                if let Some(name) = patch.get("name").and_then(|v| v.as_str()) {
                    app.name = name.to_string();
                }
                if let Some(summary) = patch.get("summary").and_then(|v| v.as_str()) {
                    app.summary = summary.to_string();
                }
                if let Some(icon) = patch.get("icon").and_then(|v| v.as_str()) {
                    app.icon = icon.to_string();
                }
                if let Some(icon_url) = patch.get("icon_url").and_then(|v| v.as_str()) {
                    app.icon_url = icon_url.to_string();
                }
                if let Some(src) = patch.get("source").and_then(|v| v.as_str()) {
                    app.source = src.to_string();
                }
                if let Some(pkg) = patch.get("package_name").and_then(|v| v.as_str()) {
                    app.package_name = pkg.to_string();
                }
                if let Some(cats) = patch.get("categories").and_then(|v| v.as_array()) {
                    app.categories = cats
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                }
            } else if key == id {
                // Create a new entry from override if not already present
                let mut app = AppInfo {
                    id: id.clone(),
                    ..Default::default()
                };
                if let Some(name) = patch.get("name").and_then(|v| v.as_str()) {
                    app.name = name.to_string();
                }
                if let Some(summary) = patch.get("summary").and_then(|v| v.as_str()) {
                    app.summary = summary.to_string();
                }
                if let Some(src) = patch.get("source").and_then(|v| v.as_str()) {
                    app.source = src.to_string();
                }
                if let Some(pkg) = patch.get("package_name").and_then(|v| v.as_str()) {
                    app.package_name = pkg.to_string();
                }
                if let Some(cats) = patch.get("categories").and_then(|v| v.as_array()) {
                    app.categories = cats
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                }
                if !app.source.is_empty() || !app.name.is_empty() {
                    apps.insert(id.clone(), app);
                }
            }
        }
    }
}
