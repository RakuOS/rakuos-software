// rakuos-firmware — Firmware update management via fwupd/fwupdmgr
// Mirrors src/backend/firmware.py

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FwupdRemote {
    pub id: String,
    pub title: String,
    pub url: String,
    pub enabled: bool,
    pub kind: String,
    pub metadata_uri: String,
    pub last_updated: String,
    pub agreement_required: bool,
    pub priority: i32,
    pub filename: String,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Check if fwupdmgr is available on the system.
pub fn fwupd_available() -> bool {
    Command::new("fwupdmgr")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Return all fwupd remotes.
pub fn get_remotes() -> Vec<FwupdRemote> {
    let out = Command::new("fwupdmgr")
        .args(["get-remotes", "--json"])
        .output();

    match out {
        Ok(o) if o.status.success() => {
            if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&o.stdout) {
                if let Some(remotes) = data["Remotes"].as_array() {
                    return remotes.iter().map(parse_remote_json).collect();
                }
            }
            parse_remotes_text()
        }
        _ => parse_remotes_text(),
    }
}

/// Enable or disable a fwupd remote. Returns (success, message).
pub fn set_remote_enabled(remote_id: &str, enabled: bool) -> (bool, String) {
    let action = if enabled { "enable-remote" } else { "disable-remote" };
    let out = Command::new("fwupdmgr")
        .args([action, remote_id])
        .output();

    match out {
        Ok(o) if o.status.success() => (
            true,
            format!(
                "Remote '{}' {}.",
                remote_id,
                if enabled { "enabled" } else { "disabled" }
            ),
        ),
        Ok(o) => (
            false,
            String::from_utf8_lossy(&o.stderr).trim().to_string(),
        ),
        Err(e) => (false, e.to_string()),
    }
}

/// Stream output from `fwupdmgr refresh --force`. Last line is "__done__<code>".
pub fn refresh_stream() -> impl Iterator<Item = String> {
    run_stream(&["refresh", "--force"])
}

/// Stream output from `fwupdmgr update`. Last line is "__done__<code>".
pub fn update_stream() -> impl Iterator<Item = String> {
    run_stream(&["update"])
}

/// Return vendor-supplied remote directories that fwupd scans.
/// Mirrors Python's get_vendor_dirs().
pub fn get_vendor_dirs() -> Vec<FwupdRemote> {
    let dirs = [
        "/usr/share/fwupd/remotes.d",
        "/etc/fwupd/remotes.d",
        "/usr/lib/fwupd/remotes.d",
    ];
    let mut found = Vec::new();
    for dir in &dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        let mut sorted: Vec<_> = entries.flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("conf"))
            .collect();
        sorted.sort_by_key(|e| e.file_name());
        for entry in sorted {
            if let Some(mut remote) = parse_remote_conf(&entry.path()) {
                found.push(remote);
            }
        }
    }
    found
}

fn parse_remote_conf(path: &std::path::Path) -> Option<FwupdRemote> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut remote = FwupdRemote::default();
    remote.filename = path.to_string_lossy().to_string();
    for line in text.lines() {
        let line = line.trim();
        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "Enabled"     => remote.enabled = v.trim().eq_ignore_ascii_case("true"),
                "Title"       => remote.title = v.trim().to_string(),
                "MetadataURI" | "Uri" => remote.url = v.trim().to_string(),
                "Id"          => remote.id = v.trim().to_string(),
                _ => {}
            }
        }
    }
    if remote.id.is_empty() {
        remote.id = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
    }
    if remote.title.is_empty() {
        remote.title = remote.id.clone();
    }
    Some(remote)
}

/// Get firmware updates available.
pub fn get_updates() -> Vec<serde_json::Value> {
    let out = Command::new("fwupdmgr")
        .args(["get-updates", "--json"])
        .output();

    match out {
        Ok(o) => serde_json::from_slice::<serde_json::Value>(&o.stdout)
            .ok()
            .and_then(|v| v["Devices"].as_array().cloned())
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn parse_remote_json(r: &serde_json::Value) -> FwupdRemote {
    FwupdRemote {
        id: r["Id"].as_str().unwrap_or("").to_string(),
        title: r["Title"]
            .as_str()
            .or_else(|| r["Id"].as_str())
            .unwrap_or("")
            .to_string(),
        url: r["Uri"]
            .as_str()
            .or_else(|| r["Url"].as_str())
            .unwrap_or("")
            .to_string(),
        enabled: r["Enabled"].as_bool().unwrap_or(false),
        kind: r["Kind"].as_str().unwrap_or("unknown").to_string(),
        metadata_uri: r["MetadataUri"].as_str().unwrap_or("").to_string(),
        last_updated: r["Timestamp"].as_str().unwrap_or("").to_string(),
        agreement_required: r["AgreementRequired"].as_bool().unwrap_or(false),
        priority: r["Priority"].as_i64().unwrap_or(0) as i32,
        filename: r["FilenameCache"].as_str().unwrap_or("").to_string(),
    }
}

fn parse_remotes_text() -> Vec<FwupdRemote> {
    let out = Command::new("fwupdmgr")
        .arg("get-remotes")
        .output();

    let Ok(o) = out else { return Vec::new() };
    let text = String::from_utf8_lossy(&o.stdout);
    let mut remotes: Vec<FwupdRemote> = Vec::new();
    let mut current = FwupdRemote::default();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            if !current.id.is_empty() {
                if current.title.is_empty() {
                    current.title = current.id.clone();
                }
                remotes.push(current);
                current = FwupdRemote::default();
            }
            continue;
        }
        if let Some((_, v)) = line.split_once(':') {
            let v = v.trim();
            if line.starts_with("Remote ID") {
                current.id = v.to_string();
            } else if line.starts_with("Enabled") {
                current.enabled = v.to_lowercase() == "true";
            } else if line.starts_with("URL") {
                current.url = v.to_string();
            } else if line.starts_with("Type") || line.starts_with("Kind") {
                current.kind = v.to_lowercase();
            } else if line.starts_with("Last") {
                current.last_updated = v.to_string();
            }
        }
    }
    if !current.id.is_empty() {
        if current.title.is_empty() {
            current.title = current.id.clone();
        }
        remotes.push(current);
    }
    remotes
}

fn run_stream(args: &[&str]) -> impl Iterator<Item = String> {
    let mut child = Command::new("fwupdmgr")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn fwupdmgr");

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
