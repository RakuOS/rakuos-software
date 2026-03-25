// rakuos-home — Home page data (popular, picks, recently updated, new apps)
// Mirrors src/backend/home.py

use anyhow::Result;
use rakuos_appstream::{AppInfo, load_appstream};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── Constants ─────────────────────────────────────────────────────────────────

const CACHE_TTL_SECS: u64 = 6 * 3600;
const PICKS_URL: &str = "https://rakuos.org/api/picks.json";
const UPDATED_FEED: &str = "https://flathub.org/api/v2/feed/recently-updated";
const NEW_FEED: &str = "https://flathub.org/api/v2/feed/new";

const POPULAR_SEED: &[&str] = &[
    "com.valvesoftware.Steam",
    "org.mozilla.firefox",
    "com.spotify.Client",
    "com.discordapp.Discord",
    "org.videolan.vlc",
    "com.obsproject.Studio",
    "org.gimp.GIMP",
    "org.inkscape.Inkscape",
    "org.libreoffice.LibreOffice",
    "org.kde.kdenlive",
    "org.blender.Blender",
    "com.google.Chrome",
    "org.signal.Signal",
    "net.lutris.Lutris",
    "com.heroicgameslauncher.hgl",
    "org.freedesktop.Piper",
    "org.gnome.Boxes",
    "io.github.celluloid_player.Celluloid",
    "com.github.tchx84.Flatseal",
    "org.kde.okular",
];

const FALLBACK_PICKS: &[&str] = &[
    "com.valvesoftware.Steam",
    "com.obsproject.Studio",
    "org.kde.kdenlive",
    "org.gimp.GIMP",
    "net.lutris.Lutris",
    "com.heroicgameslauncher.hgl",
    "org.mozilla.firefox",
    "com.discordapp.Discord",
    "org.blender.Blender",
    "com.github.tchx84.Flatseal",
    "io.github.Faugus.faugus-launcher",
    "org.libreoffice.LibreOffice",
];

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeApp {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub icon: String,
    pub icon_url: String,
    pub icon_path: String,
    pub source: String,
}

impl From<&AppInfo> for HomeApp {
    fn from(a: &AppInfo) -> Self {
        HomeApp {
            id: a.id.clone(),
            name: a.name.clone(),
            summary: a.summary.clone(),
            icon: a.icon.clone(),
            icon_url: a.icon_url.clone(),
            icon_path: a.icon_path.clone(),
            source: a.source.clone(),
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Return popular apps sorted by live Flathub install stats (cached for 6h).
/// Mirrors Python's get_popular() which fetches stats per app and sorts descending.
pub async fn get_popular() -> Vec<HomeApp> {
    let cache = cache_path("popular.json");
    if cache_valid(&cache) {
        if let Some(apps) = read_cache(&cache) {
            return apps;
        }
    }

    let appstream = load_appstream().unwrap_or_default();

    // Fetch live stats for each seed app and sort by install count descending
    let client = reqwest::Client::new();
    let mut scored: Vec<(u64, &str)> = Vec::new();
    for id in POPULAR_SEED {
        let count = fetch_app_stats(&client, id).await;
        scored.push((count, id));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0));

    let apps: Vec<HomeApp> = scored
        .iter()
        .filter_map(|(_, id)| appstream.get(*id).map(HomeApp::from))
        .collect();

    write_cache(&cache, &apps);
    apps
}

async fn fetch_app_stats(client: &reqwest::Client, app_id: &str) -> u64 {
    let url = format!("https://flathub.org/api/v2/stats/{}", app_id);
    let Ok(resp) = client
        .get(&url)
        .header("User-Agent", "RakuOS-Software/1.0")
        .timeout(Duration::from_secs(5))
        .send()
        .await
    else {
        return 0;
    };
    let Ok(json) = resp.json::<serde_json::Value>().await else { return 0 };
    json["installs_total"]
        .as_u64()
        .or_else(|| json["downloads_total"].as_u64())
        .unwrap_or(0)
}

/// Return editor's picks from rakuos.org API (with local fallback).
pub async fn get_picks() -> Vec<HomeApp> {
    let cache = cache_path("picks.json");
    if cache_valid(&cache) {
        if let Some(apps) = read_cache(&cache) {
            return apps;
        }
    }

    let appstream = load_appstream().unwrap_or_default();

    let ids: Vec<String> = match fetch_json(PICKS_URL).await {
        Ok(v) => v
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(|| FALLBACK_PICKS.iter().map(|s| s.to_string()).collect()),
        Err(_) => FALLBACK_PICKS.iter().map(|s| s.to_string()).collect(),
    };

    let apps: Vec<HomeApp> = ids
        .iter()
        .filter_map(|id| appstream.get(id.as_str()).map(HomeApp::from))
        .collect();

    write_cache(&cache, &apps);
    apps
}

/// Return recently updated apps from Flathub RSS feed.
pub async fn get_recently_updated() -> Vec<HomeApp> {
    let cache = cache_path("recently_updated.json");
    if cache_valid(&cache) {
        if let Some(apps) = read_cache(&cache) {
            return apps;
        }
    }

    let appstream = load_appstream().unwrap_or_default();
    let apps = fetch_feed_apps(UPDATED_FEED, &appstream).await;
    write_cache(&cache, &apps);
    apps
}

/// Return new apps from Flathub RSS feed.
pub async fn get_new_apps() -> Vec<HomeApp> {
    let cache = cache_path("new_apps.json");
    if cache_valid(&cache) {
        if let Some(apps) = read_cache(&cache) {
            return apps;
        }
    }

    let appstream = load_appstream().unwrap_or_default();
    let apps = fetch_feed_apps(NEW_FEED, &appstream).await;
    write_cache(&cache, &apps);
    apps
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".cache/rakuos")
}

fn cache_path(name: &str) -> PathBuf {
    cache_dir().join(name)
}

fn cache_valid(path: &PathBuf) -> bool {
    path.metadata()
        .and_then(|m| m.modified())
        .map(|t| {
            SystemTime::now()
                .duration_since(t)
                .unwrap_or(Duration::MAX)
                .as_secs()
                < CACHE_TTL_SECS
        })
        .unwrap_or(false)
}

fn read_cache(path: &PathBuf) -> Option<Vec<HomeApp>> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn write_cache(path: &PathBuf, data: &Vec<HomeApp>) {
    let _ = std::fs::create_dir_all(cache_dir());
    if let Ok(s) = serde_json::to_string(data) {
        let _ = std::fs::write(path, s);
    }
}

async fn fetch_json(url: &str) -> Result<serde_json::Value> {
    let resp = reqwest::Client::new()
        .get(url)
        .header("User-Agent", "RakuOS-Software/1.0")
        .timeout(Duration::from_secs(8))
        .send()
        .await?
        .json()
        .await?;
    Ok(resp)
}

async fn fetch_feed_apps(
    url: &str,
    appstream: &HashMap<String, AppInfo>,
) -> Vec<HomeApp> {
    let Ok(resp) = reqwest::Client::new()
        .get(url)
        .header("User-Agent", "RakuOS-Software/1.0")
        .timeout(Duration::from_secs(8))
        .send()
        .await
    else {
        return Vec::new();
    };

    let Ok(text) = resp.text().await else {
        return Vec::new();
    };

    // Parse RSS/Atom — extract app IDs from <link> fields
    // URL format: https://flathub.org/apps/org.kde.okular
    let ids: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.contains("/apps/") {
                let after = line.split("/apps/").last()?;
                let id = after
                    .trim_end_matches("</link>")
                    .trim_end_matches('"')
                    .trim_end_matches('\'')
                    .trim_end_matches('>')
                    .trim();
                if id.contains('.') {
                    return Some(id.to_string());
                }
            }
            None
        })
        .take(20)
        .collect();

    ids.iter()
        .filter_map(|id| appstream.get(id.as_str()).map(HomeApp::from))
        .collect()
}

/// Refresh all caches — mirrors Python's refresh_all(), called by tray daemon.
pub async fn refresh_all() {
    get_picks().await;
    get_recently_updated().await;
    get_new_apps().await;
    get_popular().await; // last — slowest (fetches stats per app)
}
