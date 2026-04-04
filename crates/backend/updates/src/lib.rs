// rakuos-updates — System update management via bootc
// Mirrors src/backend/updates.py

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemStatus {
    pub image: String,
    pub version: String,
    pub digest: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateInfo {
    pub available: bool,
    /// True when a staged image is waiting for reboot (digest differs from booted).
    /// When set, `available` is false and the UI should show a reboot prompt.
    pub reboot_required: bool,
    pub latest_version: String,
    pub latest_digest: String,
    pub current_version: String,
    pub current_digest: String,
    pub new_version: String,
    pub new_tag: String,
    pub repo_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OverlayStatus {
    pub package_count: usize,
    pub packages: Vec<String>,
    pub has_digest: bool,
    pub is_dirty: bool,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Get current bootc image status.
pub fn get_system_status() -> SystemStatus {
    match bootc_status_json() {
        Ok(data) => {
            let booted = &data["status"]["booted"];
            let image = &booted["image"];
            let image_ref = &image["image"];
            SystemStatus {
                image: image_ref["image"].as_str().unwrap_or("").to_string(),
                version: image["version"].as_str().unwrap_or("").to_string(),
                digest: image["imageDigest"].as_str().unwrap_or("").to_string(),
                timestamp: image["timestamp"].as_str().unwrap_or("").to_string(),
                error: None,
            }
        }
        Err(e) => SystemStatus {
            error: Some(e.to_string()),
            ..Default::default()
        },
    }
}

/// Check GHCR for a newer image using version annotation + date tag comparison.
/// Mirrors Python's check_for_update() logic exactly.
pub async fn check_for_update() -> UpdateInfo {
    let status = get_system_status();
    if let Some(e) = status.error {
        return UpdateInfo { error: Some(e), ..Default::default() };
    }

    let image_full = &status.image; // e.g. ghcr.io/rakuos/rakuos-kde:latest
    if image_full.is_empty() {
        return UpdateInfo {
            error: Some("Could not detect booted image".to_string()),
            ..Default::default()
        };
    }

    // Parse repo URL (strip :tag)
    let repo_url = image_full.rsplit_once(':').map(|(u, _)| u).unwrap_or(image_full);
    let repo_path = repo_url
        .trim_start_matches("ghcr.io/")
        .trim_start_matches("docker.io/");

    // Current date tag from version string (e.g. "latest.20260308" → "20260308")
    let current_tag = status.version.trim_start_matches("latest.").trim().to_string();

    match get_latest_version_annotation(repo_path).await {
        Ok((version_annotation, latest_digest)) => {
            let newest_tag = version_annotation.trim_start_matches("latest.").trim().to_string();
            let newest_tag_clean = if newest_tag.chars().all(|c| c.is_ascii_digit()) && newest_tag.len() == 8 {
                newest_tag.clone()
            } else {
                String::new()
            };

            if newest_tag_clean.is_empty() {
                return UpdateInfo {
                    error: Some(format!("Could not parse version annotation: {}", version_annotation)),
                    current_version: status.version.clone(),
                    current_digest: status.digest.clone(),
                    ..Default::default()
                };
            }

            let update_available = if current_tag.chars().all(|c| c.is_ascii_digit()) && current_tag.len() == 8 {
                newest_tag_clean.parse::<u64>().unwrap_or(0)
                    > current_tag.parse::<u64>().unwrap_or(0)
            } else {
                true // non-date tag — treat as needing update
            };

            UpdateInfo {
                available: update_available,
                reboot_required: false,
                current_version: status.version.clone(),
                current_digest: status.digest.clone(),
                latest_version: version_annotation,
                latest_digest,
                new_version: format!("latest.{}", newest_tag_clean),
                new_tag: newest_tag_clean,
                repo_url: repo_url.to_string(),
                error: None,
            }
        }
        Err(e) => UpdateInfo {
            current_version: status.version,
            current_digest: status.digest,
            error: Some(e.to_string()),
            ..Default::default()
        },
    }
}

/// Stream output from `bootc upgrade`. Yields log lines then "__done__<code>".
pub fn upgrade_stream() -> impl Iterator<Item = String> {
    run_stream_owned(vec!["sudo".into(), "bootc".into(), "upgrade".into()])
}

/// Stream output from `bootc upgrade --check` (dry-run).
pub fn check_stream() -> impl Iterator<Item = String> {
    run_stream_owned(vec!["sudo".into(), "bootc".into(), "upgrade".into(), "--check".into()])
}

/// Stream output from `bootc switch <target>` or `bootc upgrade`.
/// update_type: "switch" = new tag available, "upgrade" = hotfix on same tag.
pub fn upgrade_image_stream(update_type: &str, repo_url: &str, new_tag: &str) -> impl Iterator<Item = String> {
    let cmd: Vec<String> = if update_type == "upgrade" {
        vec!["sudo".into(), "bootc".into(), "upgrade".into()]
    } else {
        let target = if !repo_url.is_empty() && !new_tag.is_empty() {
            format!("{}:{}", repo_url, new_tag)
        } else {
            String::new()
        };
        if target.is_empty() {
            vec!["sudo".into(), "bootc".into(), "upgrade".into()]
        } else {
            vec!["sudo".into(), "bootc".into(), "switch".into(), target]
        }
    };

    run_stream_owned(cmd)
}

/// Stream output from overlay package upgrade via rakuos-update.
pub fn upgrade_packages_stream() -> impl Iterator<Item = String> {
    run_stream_owned(vec!["sudo".into(), "/usr/libexec/rakuos/rakuos-update".into(), "upgrade".into()])
}

/// Stream output from `bootc rollback`.
pub fn rollback_stream() -> impl Iterator<Item = String> {
    run_stream_owned(vec!["sudo".into(), "bootc".into(), "rollback".into()])
}

/// Check for overlay package updates using `rakuos-update check`.
/// Returns a Vec of raw JSON values, one per updatable package.
pub fn check_packages_script() -> Vec<serde_json::Value> {
    let out = Command::new("/usr/libexec/rakuos/rakuos-update")
        .arg("check")
        .output();
    let Ok(out) = out else { return Vec::new() };
    let data: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_default();
    data["updates"].as_array().cloned().unwrap_or_default()
}

/// Check for a system image update using the `rakuos-update check-image` script.
/// Returns (update_available, raw_json_value).
/// This matches what the daemon uses and produces the same cache format.
pub fn check_image_script() -> (bool, serde_json::Value) {
    // Before checking for new updates, see if bootc already has a staged image
    // waiting for reboot (digest differs from booted).  If so, no need to hit
    // GHCR — just tell the UI to prompt for a reboot.
    if let Ok(status_out) = Command::new("bootc")
        .args(["status", "--json"])
        .output()
    {
        if let Ok(status) = serde_json::from_slice::<serde_json::Value>(&status_out.stdout) {
            let booted_digest = status["status"]["booted"]["image"]["imageDigest"]
                .as_str().unwrap_or("");
            let staged_digest = status["status"]["staged"]["image"]["imageDigest"]
                .as_str().unwrap_or("");
            if !staged_digest.is_empty()
                && !booted_digest.is_empty()
                && staged_digest != booted_digest
            {
                let booted_ver = status["status"]["booted"]["image"]["version"]
                    .as_str().unwrap_or("").to_string();
                let staged_ver = status["status"]["staged"]["image"]["version"]
                    .as_str().unwrap_or("").to_string();
                return (false, serde_json::json!({
                    "update":          false,
                    "reboot_required": true,
                    "booted":          booted_ver,
                    "available":       staged_ver,
                }));
            }
        }
    }

    // No pending staged image — check GHCR/bootc for available updates.
    let out = Command::new("/usr/libexec/rakuos/rakuos-update")
        .arg("check-image")
        .output();
    let Ok(out) = out else {
        return (false, serde_json::json!({"error": "failed to run check-image"}));
    };
    let data: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or(serde_json::json!({}));
    let available = data["update"].as_bool().unwrap_or(false);
    (available, data)
}

/// Schedule a system reboot. Returns (success, error_message).
pub fn schedule_reboot() -> (bool, String) {
    match Command::new("systemctl").arg("reboot").output() {
        Ok(o) if o.status.success() => (true, String::new()),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// Get overlay package count and dirty/digest state.
pub fn get_overlay_status() -> OverlayStatus {
    let packages_list = Path::new("/var/lib/rakuos/packages.list");
    let rpm_list      = Path::new("/var/lib/rakuos/packages-rpm.list");
    let state_file    = Path::new("/var/lib/rakuos/overlay.state");
    let dirty_file    = Path::new("/var/lib/rakuos/overlay.dirty");

    let read_list = |p: &Path| -> Vec<String> {
        if !p.exists() { return Vec::new(); }
        std::fs::read_to_string(p)
            .unwrap_or_default()
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    };

    let mut packages = read_list(packages_list);
    packages.extend(read_list(rpm_list));
    // Sort A-Z (case-insensitive), deduplicate
    packages.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    packages.dedup();

    OverlayStatus {
        package_count: packages.len(),
        packages,
        has_digest: state_file.exists(),
        is_dirty: dirty_file.exists(),
    }
}

/// Stream output from `pkexec bootc upgrade` (system image upgrade with polkit auth).
pub fn pkexec_upgrade_stream() -> impl Iterator<Item = String> {
    run_stream_owned(vec!["pkexec".into(), "bootc".into(), "upgrade".into()])
}

/// Stream output from `pkexec bootc switch <target>` (DE/image switch with polkit auth).
pub fn pkexec_switch_stream(target: &str) -> impl Iterator<Item = String> {
    run_stream_owned(vec!["pkexec".into(), "bootc".into(), "switch".into(), target.to_string()])
}

/// Run overlay reset via pkexec and stream output.
/// mode: "soft" → --soft (preserves packages.list), "full" → --confirm (wipes everything).
pub fn reset_overlay_stream(mode: &str) -> impl Iterator<Item = String> {
    let flag = if mode == "soft" { "--soft" } else { "--confirm" };
    run_stream_owned(vec![
        "pkexec".into(),
        "/usr/libexec/rakuos/rakuos-reset-overlay".into(),
        flag.into(),
    ])
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn bootc_status_json() -> Result<serde_json::Value> {
    let out = Command::new("sudo")
        .args(["bootc", "status", "--json"])
        .output()?;
    Ok(serde_json::from_slice(&out.stdout)?)
}

/// Fetch GHCR token + version annotation for a repo. Returns (version, digest).
async fn get_latest_version_annotation(repo_path: &str) -> Result<(String, String)> {
    let token = get_ghcr_token(repo_path).await?;
    let url = format!("https://ghcr.io/v2/{}/manifests/latest", repo_path);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.oci.image.manifest.v1+json")
        .send().await?
        .json::<serde_json::Value>().await?;

    // Try manifest-level annotations first
    let mut version = resp["annotations"]
        .get("org.opencontainers.image.version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Fallback: check config blob labels
    if version.is_empty() {
        if let Some(config_digest) = resp["config"]["digest"].as_str() {
            let blob_url = format!("https://ghcr.io/v2/{}/blobs/{}", repo_path, config_digest);
            if let Ok(blob) = client
                .get(&blob_url)
                .header("Authorization", format!("Bearer {}", token))
                .send().await?
                .json::<serde_json::Value>().await
            {
                version = blob["config"]["Labels"]
                    .get("org.opencontainers.image.version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
        }
    }

    let digest = resp["config"]["digest"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok((version, digest))
}

async fn get_ghcr_token(repo_path: &str) -> Result<String> {
    let url = format!(
        "https://ghcr.io/token?scope=repository:{}:pull&service=ghcr.io",
        repo_path
    );
    let resp = reqwest::get(&url).await?.json::<serde_json::Value>().await?;
    Ok(resp["token"].as_str().unwrap_or("").to_string())
}

fn run_stream_owned(cmd: Vec<String>) -> impl Iterator<Item = String> {
    use std::io::BufRead;
    use std::process::Stdio;
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel::<String>();

    std::thread::spawn(move || {
        let mut child = match Command::new(&cmd[0])
            .args(&cmd[1..])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(format!("Error: {}", e));
                let _ = tx.send("__done__1".to_string());
                return;
            }
        };

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        // Stream stdout and stderr concurrently so neither blocks the other
        let tx_out = tx.clone();
        let stdout_thread = std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = tx_out.send(line);
            }
        });

        let tx_err = tx.clone();
        let stderr_thread = std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = tx_err.send(line);
            }
        });

        stdout_thread.join().ok();
        stderr_thread.join().ok();

        let code = child.wait().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
        let _ = tx.send(format!("__done__{}", code));
        // tx drops here, closing the channel and ending the receiver iterator
    });

    rx.into_iter()
}
