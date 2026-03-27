// rakuos-webapps — Web App management
// Mirrors src/backend/webapps.py

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

// ── Paths ─────────────────────────────────────────────────────────────────────

fn catalog_dir() -> PathBuf {
    PathBuf::from("/usr/share/rakuos/webapps")
}

fn install_dir() -> PathBuf {
    dirs_base().join(".local/share/rakuos/webapps")
}

fn icon_dir() -> PathBuf {
    dirs_base().join(".local/share/rakuos/webapps/icons")
}

fn desktop_dir() -> PathBuf {
    dirs_base().join(".local/share/applications")
}

fn dirs_base() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()))
}

const DESKTOP_PREFIX: &str = "rakuos-webapp-";

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebApp {
    pub id: String,
    pub name: String,
    pub url: String,
    pub website: String,
    pub summary: String,
    pub description: String,
    pub icon_path: String,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub screenshots: Vec<String>,
    pub custom_css: String,
    pub session_group: String,
    pub mimetypes: Vec<String>,
    pub source: String,
    pub installed: bool,
}

impl Default for WebApp {
    fn default() -> Self {
        WebApp {
            id: String::new(),
            name: String::new(),
            url: String::new(),
            website: String::new(),
            summary: String::new(),
            description: String::new(),
            icon_path: String::new(),
            categories: Vec::new(),
            keywords: Vec::new(),
            screenshots: Vec::new(),
            custom_css: String::new(),
            session_group: String::new(),
            mimetypes: Vec::new(),
            source: "webapp".to_string(),
            installed: false,
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Return all web apps from the system catalog.
pub fn get_catalog() -> Vec<WebApp> {
    let mut apps = Vec::new();
    let catalog = catalog_dir();
    if !catalog.exists() {
        return apps;
    }
    let Ok(entries) = std::fs::read_dir(&catalog) else { return apps };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else { continue };
        let Ok(data): Result<serde_json::Value, _> = serde_json::from_str(&content) else {
            continue
        };
        apps.extend(expand_catalog_data(&data));
    }
    apps
}

/// Return all installed web apps.
pub fn get_installed() -> Vec<WebApp> {
    let mut apps = Vec::new();
    let dir = install_dir();
    if !dir.exists() {
        return apps;
    }
    let Ok(entries) = std::fs::read_dir(&dir) else { return apps };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else { continue };
        let Ok(mut app): Result<WebApp, _> = serde_json::from_str(&content) else { continue };
        app.installed = true;
        app.source = "webapp".to_string();
        apps.push(app);
    }
    apps
}

/// Check if a web app is installed.
pub fn is_installed(app_id: &str) -> bool {
    install_dir().join(format!("{}.json", app_id)).exists()
}

/// Search catalog by name, summary, description, or keywords.
pub fn search(query: &str) -> Vec<WebApp> {
    let q = query.to_lowercase();
    get_catalog()
        .into_iter()
        .filter(|app| {
            app.name.to_lowercase().contains(&q)
                || app.summary.to_lowercase().contains(&q)
                || app.description.to_lowercase().contains(&q)
                || app.keywords.iter().any(|k| k.to_lowercase().contains(&q))
        })
        .collect()
}

/// Install a web app from the catalog. Returns (success, message).
pub fn install(app_id: &str) -> (bool, String) {
    let catalog = get_catalog();
    let Some(app) = catalog.iter().find(|a| a.id == app_id) else {
        return (false, format!("Web app '{}' not found in catalog.", app_id));
    };

    if let Err(e) = ensure_dirs() {
        return (false, format!("Failed to create directories: {}", e));
    }

    // Download icon if needed
    let icon_path = resolve_icon(app);

    // Write installed sidecar JSON
    let sidecar = serde_json::json!({
        "id": app.id,
        "name": app.name,
        "url": app.url,
        "website": app.website,
        "description": app.description,
        "summary": app.summary,
        "icon_path": icon_path,
        "categories": app.categories,
        "keywords": app.keywords,
        "screenshots": app.screenshots,
        "custom_css": app.custom_css,
        "session_group": app.session_group,
        "mimetypes": app.mimetypes,
        "source": "webapp",
        "installed": true,
    });

    let sidecar_path = install_dir().join(format!("{}.json", app_id));
    if let Err(e) = std::fs::write(&sidecar_path, serde_json::to_string_pretty(&sidecar).unwrap()) {
        return (false, format!("Failed to write sidecar: {}", e));
    }

    // Write .desktop file
    if let Err(e) = write_desktop(app, &icon_path) {
        return (false, format!("Failed to write .desktop: {}", e));
    }

    let _ = Command::new("update-desktop-database")
        .arg(desktop_dir())
        .output();

    (true, format!("{} installed successfully.", app.name))
}

/// Uninstall a web app. Returns (success, message).
pub fn uninstall(app_id: &str) -> (bool, String) {
    let sidecar = install_dir().join(format!("{}.json", app_id));
    let name = if sidecar.exists() {
        std::fs::read_to_string(&sidecar)
            .ok()
            .and_then(|s| serde_json::from_str::<WebApp>(&s).ok())
            .map(|a| a.name)
            .unwrap_or_else(|| app_id.to_string())
    } else {
        app_id.to_string()
    };

    let _ = std::fs::remove_file(&sidecar);
    let desktop = desktop_dir().join(format!("{}{}.desktop", DESKTOP_PREFIX, app_id));
    let _ = std::fs::remove_file(&desktop);

    let _ = Command::new("update-desktop-database")
        .arg(desktop_dir())
        .output();

    (true, format!("{} uninstalled.", name))
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn expand_catalog_data(data: &serde_json::Value) -> Vec<WebApp> {
    let sub_apps = data["apps"].as_array();
    if sub_apps.is_none() {
        return vec![finalise(parse_webapp(data, None))];
    }

    let mut entries = Vec::new();

    // Parent hub entry
    if data["url"].as_str().filter(|s| !s.is_empty()).is_some() {
        let mut hub = parse_webapp(data, None);
        hub.source = "webapp".to_string();
        entries.push(finalise(hub));
    }

    let inherited = (
        data["categories"].as_array().cloned().unwrap_or_default(),
        data["keywords"].as_array().cloned().unwrap_or_default(),
        data["url"].as_str().unwrap_or("").to_string(),
        data["id"].as_str().unwrap_or("").to_string(),
    );

    for sub in sub_apps.unwrap() {
        let mut app = parse_webapp(sub, Some(&inherited));
        if app.session_group.is_empty() {
            app.session_group = inherited.3.clone();
        }
        entries.push(finalise(app));
    }

    entries
}

fn parse_webapp(
    v: &serde_json::Value,
    inherited: Option<&(Vec<serde_json::Value>, Vec<serde_json::Value>, String, String)>,
) -> WebApp {
    let mut app = WebApp {
        id: v["id"].as_str().unwrap_or("").to_string(),
        name: v["name"].as_str().unwrap_or("").to_string(),
        url: v["url"].as_str().unwrap_or("").to_string(),
        website: v["website"].as_str().unwrap_or("").to_string(),
        summary: v["summary"].as_str().unwrap_or("").to_string(),
        description: v["description"].as_str().unwrap_or("").to_string(),
        custom_css: v["custom_css"].as_str().unwrap_or("").to_string(),
        session_group: v["session_group"].as_str().unwrap_or("").to_string(),
        categories: v["categories"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        keywords: v["keywords"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        mimetypes: v["mimetypes"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        screenshots: v["screenshots"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        icon_path: String::new(),
        source: "webapp".to_string(),
        installed: false,
    };

    // Apply inherited fields where sub-app doesn't override
    if let Some((cats, kws, parent_url, _)) = inherited {
        if app.categories.is_empty() {
            app.categories = cats.iter().filter_map(|v| v.as_str().map(String::from)).collect();
        }
        if app.keywords.is_empty() {
            app.keywords = kws.iter().filter_map(|v| v.as_str().map(String::from)).collect();
        }
        if app.website.is_empty() {
            app.website = parent_url.clone();
        }
    }

    // icon_url stored separately for download resolution
    if let Some(icon_url) = v["icon_url"].as_str() {
        app.icon_path = icon_url.to_string(); // temporary — resolved in finalise
    }

    app
}

fn finalise(mut app: WebApp) -> WebApp {
    app.installed = is_installed(&app.id);
    // icon_path resolved lazily when displayed (download on demand)
    app
}

fn resolve_icon(app: &WebApp) -> String {
    let ext = "png";
    let cached = icon_dir().join(format!("{}.{}", app.id, ext));
    if cached.exists() {
        return cached.to_string_lossy().to_string();
    }
    // icon_path holds the icon_url temporarily before install
    if !app.icon_path.is_empty() && app.icon_path.starts_with("http") {
        if let Ok(bytes) = download_bytes(&app.icon_path) {
            let _ = ensure_dirs();
            let _ = std::fs::write(&cached, bytes);
            if cached.exists() {
                return cached.to_string_lossy().to_string();
            }
        }
    }
    String::new()
}

fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "10", url])
        .output()?;
    Ok(out.stdout)
}

fn ensure_dirs() -> Result<()> {
    std::fs::create_dir_all(install_dir())?;
    std::fs::create_dir_all(icon_dir())?;
    std::fs::create_dir_all(desktop_dir())?;
    Ok(())
}

fn write_desktop(app: &WebApp, icon_path: &str) -> Result<()> {
    let icon = if icon_path.is_empty() { "web-browser" } else { icon_path };
    let cats = if app.categories.is_empty() {
        "Network;".to_string()
    } else {
        app.categories.join(";") + ";"
    };
    let exec = if app.session_group.is_empty() {
        format!("/usr/bin/rakuos-webapp-launcher '{}' '{}'", app.url, app.name)
    } else {
        format!(
            "/usr/bin/rakuos-webapp-launcher '{}' '{}' '{}'",
            app.url, app.name, app.session_group
        )
    };

    let content = format!(
        "[Desktop Entry]\nName={}\nComment={}\nExec={}\nIcon={}\n\
         Terminal=false\nType=Application\nCategories={}\n\
         StartupNotify=true\nStartupWMClass=rakuos-webapp-{}\n\
         X-RakuOS-WebApp=true\nX-RakuOS-WebApp-ID={}\nX-RakuOS-WebApp-URL={}\n",
        app.name, app.summary, exec, icon, cats, app.id, app.id, app.url
    );

    let path = desktop_dir().join(format!("{}{}.desktop", DESKTOP_PREFIX, app.id));
    std::fs::write(&path, content)?;
    Ok(())
}
