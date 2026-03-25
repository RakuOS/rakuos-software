// rakuos-packages — Native package management via `rakuos install/remove`
// Mirrors src/backend/packages.py

use anyhow::Result;
use rakuos_appstream::{AppInfo, load_appstream};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeApp {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub version: String,
    pub package_name: String,
    pub icon: String,
    pub icon_url: String,
    pub icon_path: String,  // resolved local filesystem path
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub screenshots: Vec<String>,
    pub developer: String,
    pub url_homepage: String,
    pub license: String,
    pub source: String, // "native", "terra", "flatpak", etc.
    pub installed: bool,
    pub is_addon: bool,
    pub pkg_name_guessed: bool,
}

impl From<&AppInfo> for NativeApp {
    fn from(a: &AppInfo) -> Self {
        NativeApp {
            id: a.id.clone(),
            name: a.name.clone(),
            summary: a.summary.clone(),
            description: a.description.clone(),
            version: a.version.clone(),
            package_name: a.package_name.clone(),
            icon: a.icon.clone(),
            icon_url: a.icon_url.clone(),
            icon_path: a.icon_path.clone(),
            categories: a.categories.clone(),
            keywords: a.keywords.clone(),
            screenshots: a.screenshots.clone(),
            developer: a.developer.clone(),
            url_homepage: a.url_homepage.clone(),
            license: a.license.clone(),
            source: a.source.clone(),
            installed: false,
            is_addon: a.is_addon,
            pkg_name_guessed: a.pkg_name_guessed,
        }
    }
}

// ── Browsability filter ───────────────────────────────────────────────────────

/// Mirror of Python's _is_browseable(): excludes addons (unless standalone),
/// and apps without a package_name.
fn is_browseable(a: &NativeApp) -> bool {
    if a.package_name.is_empty() {
        return false;
    }
    // Guessed pkg_names may not exist in repos — exclude from browse/search
    if a.pkg_name_guessed {
        return false;
    }
    if a.is_addon {
        let standalone = [
            "game", "network", "audiovideo", "audio", "video", "graphics",
            "office", "development", "education", "science", "accessibility",
        ];
        let cats: Vec<String> = a.categories.iter().map(|c| c.to_lowercase()).collect();
        return cats.iter().any(|c| standalone.contains(&c.as_str()));
    }
    true
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Return all available apps from AppStream (native + flatpak), with installed status.
pub fn get_available() -> Result<Vec<NativeApp>> {
    let appstream = load_appstream()?;
    let installed_rpm = get_installed_packages()?;
    let installed_fp = get_installed_flatpaks();

    let apps: Vec<NativeApp> = appstream
        .values()
        .filter(|a| !a.package_name.is_empty())
        .map(|a| {
            let mut app = NativeApp::from(a);
            if a.source == "flatpak" {
                app.installed = installed_fp.contains(&a.id);
            } else {
                app.installed = installed_rpm.contains(&a.package_name);
            }
            app
        })
        .collect();

    Ok(apps)
}

/// Return user-installed overlay packages (from packages.list), enriched with AppStream metadata.
/// Mirrors Python's get_installed_with_metadata().
/// Only returns packages the user explicitly added — NOT the base OS image packages.
pub fn get_installed() -> Result<Vec<NativeApp>> {
    let overlay_pkgs = read_packages_list();
    if overlay_pkgs.is_empty() {
        return Ok(Vec::new());
    }

    let appstream = load_appstream()?;

    // Build pkg_name → AppInfo lookup
    let mut pkg_map: std::collections::HashMap<String, &rakuos_appstream::AppInfo> =
        std::collections::HashMap::new();
    for a in appstream.values() {
        if a.is_addon || a.package_name.is_empty() {
            continue;
        }
        pkg_map.entry(a.package_name.clone()).or_insert(a);
    }

    // Build flatpak-by-name/id-segment for fallback matching
    let mut flatpak_by_name: std::collections::HashMap<String, &rakuos_appstream::AppInfo> =
        std::collections::HashMap::new();
    for a in appstream.values() {
        if a.source == "flatpak" {
            flatpak_by_name.entry(a.name.to_lowercase()).or_insert(a);
            let last_seg = a.id.split('.').last().unwrap_or("").to_lowercase();
            if !last_seg.is_empty() {
                flatpak_by_name.entry(last_seg).or_insert(a);
            }
        }
    }

    let mut results = Vec::new();
    for pkg in &overlay_pkgs {
        // Verify it is actually installed
        if !rpm_is_installed(pkg) {
            continue;
        }
        if let Some(meta) = pkg_map.get(pkg) {
            let mut app = NativeApp::from(*meta);
            app.installed = true;
            app.source = "native".to_string();
            results.push(app);
        } else if let Some(meta) = flatpak_by_name.get(&pkg.to_lowercase()) {
            // Borrow Flatpak metadata for icon/description
            let mut app = NativeApp::from(*meta);
            app.id = pkg.clone();
            app.package_name = pkg.clone();
            app.source = "native".to_string();
            app.installed = true;
            results.push(app);
        } else {
            results.push(NativeApp {
                id: pkg.clone(),
                name: pkg.clone(),
                summary: "Installed package".to_string(),
                package_name: pkg.clone(),
                source: "native".to_string(),
                installed: true,
                ..Default::default()
            });
        }
    }

    results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(results)
}

/// Return installed Flatpak apps enriched with AppStream metadata.
pub fn get_installed_flatpaks_enriched() -> Result<Vec<NativeApp>> {
    let appstream = load_appstream()?;
    let installed_fp = get_installed_flatpaks_with_info();

    let mut results: Vec<NativeApp> = Vec::new();
    for (app_id, name, version, summary, origin) in &installed_fp {
        if let Some(meta) = appstream.get(app_id.as_str()) {
            let mut app = NativeApp::from(meta);
            app.installed = true;
            if !version.is_empty() {
                app.version = version.clone();
            }
            results.push(app);
        } else {
            // No AppStream data — use flatpak list output directly
            results.push(NativeApp {
                id: app_id.clone(),
                name: name.clone(),
                summary: summary.clone(),
                version: version.clone(),
                package_name: app_id.clone(),
                source: "flatpak".to_string(),
                installed: true,
                icon: format!("{}.png", app_id),
                ..Default::default()
            });
        }
    }
    Ok(results)
}

/// Search all apps (native + flatpak) by name, summary, id, or keywords.
/// Results ranked: name-starts-with > name-contains > id/pkg-contains > summary > keyword.
pub fn search(query: &str) -> Result<Vec<NativeApp>> {
    let appstream = load_appstream()?;
    let installed_rpm = get_installed_packages()?;
    let installed_fp = get_installed_flatpaks();
    let q = query.to_lowercase();

    let mut scored: Vec<(u8, NativeApp)> = Vec::new();

    for a in appstream.values() {
        let mut app = NativeApp::from(a);
        if a.source == "flatpak" {
            app.installed = installed_fp.contains(&a.id);
        } else {
            app.installed = installed_rpm.contains(&a.package_name);
        }
        if !is_browseable(&app) {
            continue;
        }

        let name_lc = app.name.to_lowercase();
        let id_lc = app.id.to_lowercase();
        let pkg_lc = app.package_name.to_lowercase();
        let summary_lc = app.summary.to_lowercase();

        let score = if name_lc.starts_with(&q) {
            6
        } else if name_lc.contains(&q) {
            5
        } else if id_lc.contains(&q) || pkg_lc.contains(&q) {
            4
        } else if summary_lc.contains(&q) {
            3
        } else if app.keywords.iter().any(|k| k.to_lowercase().contains(&q)) {
            2
        } else {
            continue;
        };

        scored.push((score, app));
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.name.cmp(&b.1.name)));
    Ok(scored.into_iter().map(|(_, a)| a).collect())
}

/// Return all apps in a given AppStream category (case-insensitive).
/// Includes both native and flatpak. Deduplicates by (name_lower, source).
pub fn get_by_category(category: &str) -> Result<Vec<NativeApp>> {
    let appstream = load_appstream()?;
    let installed_rpm = get_installed_packages()?;
    let installed_fp = get_installed_flatpaks();
    let cat = category.to_lowercase();

    let mut raw: Vec<NativeApp> = Vec::new();
    for a in appstream.values() {
        let mut app = NativeApp::from(a);
        if !is_browseable(&app) {
            continue;
        }
        if !app.categories.iter().any(|c| c.to_lowercase() == cat) {
            continue;
        }
        if a.source == "flatpak" {
            app.installed = installed_fp.contains(&a.id);
        } else {
            app.installed = installed_rpm.contains(&a.package_name);
        }
        raw.push(app);
    }

    // Deduplicate by (name_lower, source) — keep highest package_name
    let mut seen: HashMap<(String, String), NativeApp> = HashMap::new();
    for app in raw {
        let key = (app.name.to_lowercase(), app.source.clone());
        let entry = seen.entry(key).or_insert_with(|| app.clone());
        if app.package_name > entry.package_name {
            *entry = app;
        }
    }

    let mut results: Vec<NativeApp> = seen.into_values().collect();
    results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(results)
}

/// Look up a single app by its AppStream ID.
pub fn get_app_by_id(app_id: &str) -> Result<Option<NativeApp>> {
    let appstream = load_appstream()?;
    let installed_rpm = get_installed_packages()?;
    let installed_fp = get_installed_flatpaks();

    if let Some(a) = appstream.get(app_id) {
        let mut app = NativeApp::from(a);
        if a.source == "flatpak" {
            app.installed = installed_fp.contains(&a.id);
        } else {
            app.installed = installed_rpm.contains(&a.package_name);
        }
        return Ok(Some(app));
    }
    Ok(None)
}

/// Install a native package via `pkexec rakuos install <pkg>`.
/// Streams output lines; last line is "__done__<code>".
pub fn install_stream(package_name: &str) -> impl Iterator<Item = String> + '_ {
    run_rakuos_stream(&["install", package_name])
}

/// Remove a native package via `pkexec rakuos remove <pkg>`.
pub fn remove_stream(package_name: &str) -> impl Iterator<Item = String> + '_ {
    run_rakuos_stream(&["remove", package_name])
}

/// Check if a package name is installed via rpm -q.
pub fn is_installed(package_name: &str) -> bool {
    Command::new("rpm")
        .args(["-q", package_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ── Internals ─────────────────────────────────────────────────────────────────

const PACKAGES_LIST: &str = "/var/lib/rakuos/packages.list";

/// Read /var/lib/rakuos/packages.list — the user's overlay package list.
/// Returns package names, skipping blank lines and comments.
fn read_packages_list() -> Vec<String> {
    match std::fs::read_to_string(PACKAGES_LIST) {
        Ok(content) => content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(String::from)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check if a single package is installed via rpm -q.
fn rpm_is_installed(pkg: &str) -> bool {
    Command::new("rpm")
        .args(["-q", "--quiet", pkg])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Return set of ALL installed RPM package names (for installed-status checks in browse/search).
/// Uses rpm -qa — only called for available/search/category, not for the installed list UI.
fn get_installed_packages() -> Result<HashSet<String>> {
    // For available() / search() / get_by_category() we use packages.list if present,
    // so the installed badge is accurate without querying the entire rpm database.
    // Fall back to rpm -qa only when packages.list doesn't exist (non-RakuOS systems).
    let list = read_packages_list();
    if !list.is_empty() {
        return Ok(list.into_iter().collect());
    }
    // Non-RakuOS fallback
    let out = Command::new("rpm")
        .args(["-qa", "--queryformat", "%{NAME}\\n"])
        .output()?;
    let packages = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(String::from)
        .collect();
    Ok(packages)
}

/// Return set of installed Flatpak app IDs.
fn get_installed_flatpaks() -> HashSet<String> {
    let Ok(out) = Command::new("flatpak")
        .args(["list", "--app", "--columns=application"])
        .output()
    else {
        return HashSet::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

/// Return installed Flatpak apps with basic metadata from `flatpak list`.
fn get_installed_flatpaks_with_info() -> Vec<(String, String, String, String, String)> {
    let Ok(out) = Command::new("flatpak")
        .args([
            "list",
            "--app",
            "--columns=application,name,version,description,origin",
        ])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.is_empty() || parts[0].is_empty() {
                return None;
            }
            Some((
                parts[0].to_string(),
                parts.get(1).unwrap_or(&"").to_string(),
                parts.get(2).unwrap_or(&"").to_string(),
                parts.get(3).unwrap_or(&"").to_string(),
                parts.get(4).unwrap_or(&"flathub").to_string(),
            ))
        })
        .collect()
}

fn run_rakuos_stream(args: &[&str]) -> impl Iterator<Item = String> {
    let mut cmd_args = vec!["rakuos"];
    cmd_args.extend_from_slice(args);

    let mut child = Command::new("pkexec")
        .args(&cmd_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn pkexec");

    let stdout = child.stdout.take().unwrap();
    let lines: Vec<String> = BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
        .collect();
    let code = child.wait().map(|s| s.code().unwrap_or(1)).unwrap_or(1);

    lines
        .into_iter()
        .chain(std::iter::once(format!("__done__{}", code)))
}

// Needed for Default impl in get_installed_flatpaks_enriched
impl Default for NativeApp {
    fn default() -> Self {
        NativeApp {
            id: String::new(),
            name: String::new(),
            summary: String::new(),
            description: String::new(),
            version: String::new(),
            package_name: String::new(),
            icon: String::new(),
            icon_url: String::new(),
            icon_path: String::new(),
            categories: Vec::new(),
            keywords: Vec::new(),
            screenshots: Vec::new(),
            developer: String::new(),
            url_homepage: String::new(),
            license: String::new(),
            source: String::new(),
            installed: false,
            is_addon: false,
            pkg_name_guessed: false,
        }
    }
}
