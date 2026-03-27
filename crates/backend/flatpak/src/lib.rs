// rakuos-flatpak — Flatpak package management
// Mirrors src/backend/flatpak.py

use anyhow::Result;
use rakuos_appstream::{AppInfo, get_appstream};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

const FLATHUB_URL: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatpakApp {
    pub id: String,
    pub name: String,
    pub version: String,
    pub summary: String,
    pub origin: String,
    pub installed: bool,
    pub icon: String,
    pub icon_url: String,
    pub categories: Vec<String>,
    pub screenshots: Vec<String>,
    pub description: String,
    pub developer: String,
    pub url_homepage: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlatpakRemote {
    pub name: String,
    pub title: String,
    pub url: String,
    pub enabled: bool,
    pub system: bool,
}

impl From<&AppInfo> for FlatpakApp {
    fn from(a: &AppInfo) -> Self {
        FlatpakApp {
            id: a.id.clone(),
            name: a.name.clone(),
            version: a.version.clone(),
            summary: a.summary.clone(),
            origin: "flathub".to_string(),
            installed: false,
            icon: a.icon.clone(),
            icon_url: a.icon_url.clone(),
            categories: a.categories.clone(),
            screenshots: a.screenshots.clone(),
            description: a.description.clone(),
            developer: a.developer.clone(),
            url_homepage: a.url_homepage.clone(),
            source: "flatpak".to_string(),
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Return list of installed Flatpaks enriched with AppStream metadata.
pub fn get_installed() -> Result<Vec<FlatpakApp>> {
    let appstream = get_appstream();
    let appstream_flatpak: HashMap<&str, &AppInfo> = appstream
        .values()
        .filter(|a| a.source == "flatpak")
        .map(|a| (a.id.as_str(), a))
        .collect();

    let out = Command::new("flatpak")
        .args([
            "list",
            "--app",
            "--columns=application,name,version,description,origin",
        ])
        .output()?;

    let mut apps = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let app_id = parts[0].trim();
        let name = parts.get(1).map(|s| s.trim()).unwrap_or(app_id);
        let version = parts.get(2).map(|s| s.trim()).unwrap_or("").to_string();
        let summary = parts.get(3).map(|s| s.trim()).unwrap_or("").to_string();
        let origin = parts.get(4).map(|s| s.trim()).unwrap_or("flathub").to_string();

        let meta = appstream_flatpak.get(app_id);
        apps.push(FlatpakApp {
            id: app_id.to_string(),
            name: meta.map(|m| m.name.clone()).unwrap_or_else(|| name.to_string()),
            version,
            summary: meta.map(|m| m.summary.clone()).unwrap_or(summary),
            origin,
            installed: true,
            icon: meta.map(|m| m.icon.clone()).unwrap_or_default(),
            icon_url: meta.map(|m| m.icon_url.clone()).unwrap_or_default(),
            categories: meta.map(|m| m.categories.clone()).unwrap_or_default(),
            screenshots: meta.map(|m| m.screenshots.clone()).unwrap_or_default(),
            description: meta.map(|m| m.description.clone()).unwrap_or_default(),
            developer: meta.map(|m| m.developer.clone()).unwrap_or_default(),
            url_homepage: meta.map(|m| m.url_homepage.clone()).unwrap_or_default(),
            source: "flatpak".to_string(),
        });
    }
    Ok(apps)
}

/// Search Flatpak remotes for apps matching query. Returns up to `limit` results.
pub fn search(query: &str, limit: usize) -> Vec<FlatpakApp> {
    let out = Command::new("flatpak")
        .args([
            "search",
            "--columns=application,name,version,description,origin",
            query,
        ])
        .output();

    let Ok(out) = out else { return Vec::new() };

    let installed_ids = get_installed_ids();

    let mut apps = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 || parts[0].trim().is_empty() {
            continue;
        }
        let app_id = parts[0].trim().to_string();
        apps.push(FlatpakApp {
            installed: installed_ids.contains(&app_id),
            name: parts.get(1).map(|s| s.trim().to_string()).unwrap_or_else(|| app_id.clone()),
            version: parts.get(2).map(|s| s.trim().to_string()).unwrap_or_default(),
            summary: parts.get(3).map(|s| s.trim().to_string()).unwrap_or_default(),
            origin: parts.get(4).map(|s| s.trim().to_string()).unwrap_or_else(|| "flathub".to_string()),
            id: app_id,
            icon: String::new(),
            icon_url: String::new(),
            categories: Vec::new(),
            screenshots: Vec::new(),
            description: String::new(),
            developer: String::new(),
            url_homepage: String::new(),
            source: "flatpak".to_string(),
        });
        if apps.len() >= limit {
            break;
        }
    }
    apps
}

/// Return list of Flatpaks with available updates.
pub fn get_updates() -> Vec<FlatpakApp> {
    let out = Command::new("flatpak")
        .args([
            "remote-ls",
            "--updates",
            "--app",
            "--columns=application,name,version",
        ])
        .output();

    let Ok(out) = out else { return Vec::new() };

    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 || parts[0].trim().is_empty() {
                return None;
            }
            Some(FlatpakApp {
                id: parts[0].trim().to_string(),
                name: parts.get(1).map(|s| s.trim().to_string()).unwrap_or_default(),
                version: parts.get(2).map(|s| s.trim().to_string()).unwrap_or_default(),
                source: "flatpak".to_string(),
                installed: true,
                origin: String::new(),
                summary: String::new(),
                description: String::new(),
                icon: String::new(),
                icon_url: String::new(),
                categories: Vec::new(),
                screenshots: Vec::new(),
                developer: String::new(),
                url_homepage: String::new(),
            })
        })
        .collect()
}

/// Install a Flatpak. Streams output lines, last line is "__done__<code>".
pub fn install_stream(app_id: &str) -> impl Iterator<Item = String> + '_ {
    run_flatpak_stream(&["install", "--noninteractive", "-y", app_id])
}

/// Uninstall a Flatpak. Streams output lines.
pub fn uninstall_stream(app_id: &str) -> impl Iterator<Item = String> + '_ {
    run_flatpak_stream(&["uninstall", "--noninteractive", "-y", app_id])
}

/// Uninstall a Flatpak with --force-remove (for runtimes/add-ons with dependents).
pub fn force_uninstall_stream(app_id: &str) -> impl Iterator<Item = String> + '_ {
    run_flatpak_stream(&["uninstall", "--noninteractive", "-y", "--force-remove", app_id])
}

/// Update all Flatpaks. Streams output lines.
pub fn update_stream() -> impl Iterator<Item = String> {
    run_flatpak_stream(&["update", "--noninteractive", "-y"])
}

/// Update a single Flatpak. Streams output lines.
pub fn update_single_stream(app_id: &str) -> impl Iterator<Item = String> + '_ {
    run_flatpak_stream(&["update", "--noninteractive", "-y", app_id])
}

/// Install from a local .flatpak bundle (system-wide via pkexec).
pub fn install_local_bundle_stream(path: &str) -> impl Iterator<Item = String> + '_ {
    run_pkexec_flatpak_stream(&["install", "--bundle", "--noninteractive", "-y", path])
}

/// Install from a .flatpakref file (system-wide via pkexec).
pub fn install_flatpakref_stream(path: &str) -> impl Iterator<Item = String> + '_ {
    run_pkexec_flatpak_stream(&["install", "--from", "--noninteractive", "-y", path])
}

/// Check if a Flatpak app_id is installed.
pub fn is_installed(app_id: &str) -> bool {
    Command::new("flatpak")
        .args(["info", app_id])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ── Remote / repo management ──────────────────────────────────────────────────

/// Return all configured Flatpak remotes (system + user).
pub fn get_remotes() -> Vec<FlatpakRemote> {
    let mut remotes: Vec<FlatpakRemote> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (scope, flag) in &[("system", "--system"), ("user", "--user")] {
        // Try with --columns first (Flatpak >= 1.2)
        let out = Command::new("flatpak")
            .args(["remotes", flag, "--columns=name,title,url,options"])
            .output();

        let text = match out {
            Ok(ref o) if o.status.success() && !o.stdout.is_empty() => {
                String::from_utf8_lossy(&o.stdout).to_string()
            }
            _ => {
                // Fall back to plain output
                Command::new("flatpak")
                    .args(["remotes", flag])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                    .unwrap_or_default()
            }
        };

        for line in text.lines() {
            if line.trim().is_empty() || line.starts_with("Name") {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            let name = parts[0].trim().to_string();
            if name.is_empty() || seen.contains(&name) {
                continue;
            }
            seen.insert(name.clone());
            let title   = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();
            let url     = parts.get(2).map(|s| s.trim()).unwrap_or("").to_string();
            let options = parts.get(3).map(|s| s.trim().to_lowercase()).unwrap_or_default();
            let enabled = !options.contains("disabled");
            remotes.push(FlatpakRemote {
                name,
                title: if title.is_empty() { parts[0].trim().to_string() } else { title },
                url,
                enabled,
                system: *scope == "system",
            });
        }
    }

    // Last-resort: no-scope query
    if remotes.is_empty() {
        if let Ok(out) = Command::new("flatpak")
            .args(["remotes", "--columns=name,title,url,options"])
            .output()
        {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if line.trim().is_empty() || line.starts_with("Name") { continue; }
                let parts: Vec<&str> = line.split('\t').collect();
                let name = parts[0].trim().to_string();
                if name.is_empty() || seen.contains(&name) { continue; }
                seen.insert(name.clone());
                let title   = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();
                let url     = parts.get(2).map(|s| s.trim()).unwrap_or("").to_string();
                let options = parts.get(3).map(|s| s.trim().to_lowercase()).unwrap_or_default();
                remotes.push(FlatpakRemote {
                    name,
                    title,
                    url,
                    enabled: !options.contains("disabled"),
                    system: true,
                });
            }
        }
    }

    remotes
}

/// Check if Flathub is configured.
pub fn has_flathub() -> bool {
    get_remotes().iter().any(|r| {
        r.name.to_lowercase().contains("flathub") || r.url.to_lowercase().contains("flathub")
    })
}

/// Add a Flatpak remote. Returns (success, message).
pub fn add_remote(name: &str, url: &str, system: bool) -> (bool, String) {
    let scope = if system { "--system" } else { "--user" };
    match Command::new("flatpak")
        .args(["remote-add", scope, "--if-not-exists", name, url])
        .output()
    {
        Ok(o) if o.status.success() => (true, format!("Remote '{}' added.", name)),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// Add Flathub remote.
pub fn add_flathub(system: bool) -> (bool, String) {
    add_remote("flathub", FLATHUB_URL, system)
}

/// Remove a Flatpak remote. Returns (success, message).
pub fn remove_remote(name: &str, system: bool) -> (bool, String) {
    let scope = if system { "--system" } else { "--user" };
    match Command::new("flatpak")
        .args(["remote-delete", scope, "--force", name])
        .output()
    {
        Ok(o) if o.status.success() => (true, format!("Remote '{}' removed.", name)),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// Enable or disable a Flatpak remote. Returns (success, message).
pub fn set_remote_enabled(name: &str, enabled: bool, system: bool) -> (bool, String) {
    let scope = if system { "--system" } else { "--user" };
    let action = if enabled { "remote-modify" } else { "remote-modify" };
    let flag = if enabled { "--enable" } else { "--disable" };
    match Command::new("flatpak")
        .args([action, scope, flag, name])
        .output()
    {
        Ok(o) if o.status.success() => (
            true,
            format!("Remote '{}' {}.", name, if enabled { "enabled" } else { "disabled" }),
        ),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

// ── Local file info ───────────────────────────────────────────────────────────

/// Extract metadata from a local .flatpak bundle file using `flatpak info --bundle`.
pub fn get_local_flatpak_info(path: &str) -> serde_json::Value {
    let out = Command::new("flatpak")
        .args(["info", "--bundle", path])
        .output();

    if let Ok(o) = out {
        let mut data: HashMap<String, String> = HashMap::new();
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if let Some((k, v)) = line.split_once(':') {
                data.insert(k.trim().to_lowercase(), v.trim().to_string());
            }
        }
        let app_id  = data.get("id").cloned().unwrap_or_else(|| basename_no_ext(path));
        let name    = data.get("name").cloned()
            .unwrap_or_else(|| app_id.split('.').last().unwrap_or(&app_id).to_string());
        let version = data.get("version").cloned().unwrap_or_default();
        let summary = data.get("subject").cloned().unwrap_or_default();
        let branch  = data.get("branch").cloned().unwrap_or_else(|| "stable".to_string());

        return serde_json::json!({
            "id": app_id, "name": name, "summary": summary,
            "description": "", "categories": [], "icon": "", "screenshots": [],
            "pkg_name": app_id, "url": "", "source": "flatpak",
            "installed": false, "is_addon": false,
            "local_flatpak": path, "version": version, "branch": branch,
        });
    }

    // Minimal fallback
    let app_id = basename_no_ext(path);
    serde_json::json!({
        "id": app_id, "name": app_id, "summary": "Local Flatpak bundle",
        "description": "", "categories": [], "icon": "", "screenshots": [],
        "pkg_name": app_id, "url": "", "source": "flatpak",
        "installed": false, "is_addon": false, "local_flatpak": path,
    })
}

/// Parse a .flatpakref INI file and return app info.
pub fn get_flatpakref_info(path: &str) -> serde_json::Value {
    let Ok(text) = std::fs::read_to_string(path) else {
        let app_id = basename_no_ext(path);
        return serde_json::json!({
            "id": app_id, "name": app_id, "summary": "",
            "source": "flatpak", "installed": false, "local_flatpakref": path,
        });
    };

    let mut app_id = basename_no_ext(path);
    let mut title = String::new();
    let mut comment = String::new();
    let mut url = String::new();
    let mut branch = "stable".to_string();
    let mut in_section = false;

    for line in text.lines() {
        let line = line.trim();
        if line.eq_ignore_ascii_case("[Flatpak Ref]") {
            in_section = true;
            continue;
        }
        if line.starts_with('[') { in_section = false; continue; }
        if !in_section { continue; }
        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "Name"    => app_id = v.trim().to_string(),
                "Title"   => title = v.trim().to_string(),
                "Comment" => comment = v.trim().to_string(),
                "Url"     => url = v.trim().to_string(),
                "Branch"  => branch = v.trim().to_string(),
                _ => {}
            }
        }
    }

    let name = if title.is_empty() {
        app_id.split('.').last().unwrap_or(&app_id).to_string()
    } else {
        title
    };

    serde_json::json!({
        "id": app_id, "name": name, "summary": comment,
        "description": "", "categories": [], "icon": "", "screenshots": [],
        "pkg_name": app_id, "url": url, "source": "flatpak",
        "installed": false, "is_addon": false,
        "local_flatpakref": path, "branch": branch,
    })
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn get_installed_ids() -> std::collections::HashSet<String> {
    Command::new("flatpak")
        .args(["list", "--app", "--columns=application"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.trim().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn run_flatpak_stream(args: &[&str]) -> impl Iterator<Item = String> {
    let mut child = Command::new("flatpak")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn flatpak");

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

fn run_pkexec_flatpak_stream(flatpak_args: &[&str]) -> impl Iterator<Item = String> {
    let mut cmd_args = vec!["flatpak"];
    cmd_args.extend_from_slice(flatpak_args);

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

fn basename_no_ext(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}
