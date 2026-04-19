// rakuos-updates — System update management via bootc
// Mirrors src/backend/updates.py

use anyhow::{Result, anyhow};
use reqwest::header::{ACCEPT, AUTHORIZATION, WWW_AUTHENTICATE};
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

#[derive(Debug, Clone)]
struct ImageChannelInfo {
    original_repo_url: String,
    repo_url: String,
    registry_host: String,
    repo_path: String,
    channel_tag: String,
    current_date_tag: String,
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

/// Check the current OCI registry for a newer image using version annotation + date tag comparison.
pub async fn check_for_update() -> UpdateInfo {
    let status = get_system_status();
    if let Some(e) = status.error {
        return UpdateInfo { error: Some(e), ..Default::default() };
    }

    let channel = match parse_image_channel_info(&status.image, &status.version) {
        Ok(info) => info,
        Err(e) => {
            return UpdateInfo {
                error: Some(e),
                current_version: status.version.clone(),
                current_digest: status.digest.clone(),
                ..Default::default()
            };
        }
    };

    match get_latest_version_annotation(&channel.registry_host, &channel.repo_path, &channel.channel_tag).await {
        Ok((version_annotation, latest_digest)) => {
            let newest_tag_clean = extract_date_tag(&version_annotation, &channel.channel_tag);

            if newest_tag_clean.is_empty() {
                return UpdateInfo {
                    error: Some(format!("Could not parse version annotation: {}", version_annotation)),
                    current_version: status.version.clone(),
                    current_digest: status.digest.clone(),
                    ..Default::default()
                };
            }

            let update_available = if channel.current_date_tag.chars().all(|c| c.is_ascii_digit())
                && channel.current_date_tag.len() == 8
            {
                newest_tag_clean.parse::<u64>().unwrap_or(0)
                    > channel.current_date_tag.parse::<u64>().unwrap_or(0)
            } else {
                true // non-date tag — treat as needing update
            };

            let target_tag = if channel.channel_tag == "latest" {
                newest_tag_clean.clone()
            } else {
                format!("{}.{}", channel.channel_tag, newest_tag_clean)
            };

            let is_legacy_ghcr = channel.original_repo_url.starts_with("ghcr.io/rakuos/")
                && channel.original_repo_url != channel.repo_url;

            UpdateInfo {
                available: is_legacy_ghcr || update_available,
                reboot_required: false,
                current_version: status.version.clone(),
                current_digest: status.digest.clone(),
                latest_version: version_annotation,
                latest_digest,
                new_version: if channel.channel_tag == "latest" {
                    format!("latest.{}", newest_tag_clean)
                } else {
                    format!("{}.{}", channel.channel_tag, newest_tag_clean)
                },
                new_tag: target_tag,
                repo_url: channel.repo_url,
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

/// Stream output from upgrading a single overlay package by name.
pub fn upgrade_single_package_stream(name: &str) -> impl Iterator<Item = String> {
    run_stream_owned(vec![
        "sudo".into(),
        "/usr/libexec/rakuos/rakuos-update".into(),
        "upgrade-package".into(),
        name.to_string(),
    ])
}

/// Stream output from `bootc rollback` followed by a soft overlay reset.
/// Both run via pkexec so the user is prompted for auth once per command.
/// The overlay soft-reset ensures the overlay is rebuilt from the packages
/// list on next boot, preventing stale packages after an image rollback.
pub fn rollback_stream() -> impl Iterator<Item = String> {
    let rollback = run_stream_owned(vec![
        "pkexec".into(), "bootc".into(), "rollback".into(),
    ]);
    let soft_reset = run_stream_owned(vec![
        "pkexec".into(),
        "/usr/libexec/rakuos/rakuos-reset-overlay".into(),
        "--soft".into(),
    ]);
    rollback.chain(soft_reset)
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

    // No pending staged image — check the registry/bootc for available updates.
    let out = Command::new("/usr/libexec/rakuos/rakuos-update")
        .arg("check-image")
        .output();
    let Ok(out) = out else {
        return (false, serde_json::json!({"error": "failed to run check-image"}));
    };
    let data: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or(serde_json::json!({}));
    let available = data["update"].as_bool().unwrap_or_else(|| out.status.success());
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

/// Run a full overlay reset and then switch to a new image target.
/// This is used by the system page image switch flow so the overlay is reset
/// before staging the new image.
pub fn pkexec_reset_overlay_and_switch_stream(target: &str) -> impl Iterator<Item = String> {
    use std::sync::mpsc;

    let target = target.to_string();
    let (tx, rx) = mpsc::channel::<String>();

    std::thread::spawn(move || {
        let _ = tx.send("Resetting overlay before image switch…".to_string());

        let mut reset_exit = 1i32;
        for line in reset_overlay_stream("full") {
            if let Some(code) = line.strip_prefix("__done__") {
                reset_exit = code.trim().parse().unwrap_or(1);
            } else {
                let _ = tx.send(line);
            }
        }

        if reset_exit != 0 {
            let _ = tx.send(format!("__done__{}", reset_exit));
            return;
        }

        let _ = tx.send("Overlay reset complete. Switching image…".to_string());

        let mut switch_exit = 1i32;
        for line in pkexec_switch_stream(&target) {
            if let Some(code) = line.strip_prefix("__done__") {
                switch_exit = code.trim().parse().unwrap_or(1);
            } else {
                let _ = tx.send(line);
            }
        }

        let _ = tx.send(format!("__done__{}", switch_exit));
    });

    rx.into_iter()
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
    let out = Command::new("bootc")
        .args(["status", "--json"])
        .output()?;

    if out.status.success() {
        return Ok(serde_json::from_slice(&out.stdout)?);
    }

    let sudo_out = Command::new("sudo")
        .args(["bootc", "status", "--json"])
        .output()?;

    if sudo_out.status.success() {
        return Ok(serde_json::from_slice(&sudo_out.stdout)?);
    }

    let stderr = String::from_utf8_lossy(&sudo_out.stderr).trim().to_string();
    Err(anyhow!(
        "failed to read bootc status: {}",
        if stderr.is_empty() { "unknown error" } else { &stderr }
    ))
}

/// Fetch version annotation for a repo from the current OCI registry. Returns (version, digest).
async fn get_latest_version_annotation(
    registry_host: &str,
    repo_path: &str,
    channel_tag: &str,
) -> Result<(String, String)> {
    let url = format!("https://{}/v2/{}/manifests/{}", registry_host, repo_path, channel_tag);
    let client = reqwest::Client::new();
    let token = get_registry_token(&client, registry_host, repo_path, channel_tag).await?;
    let mut manifest_req = client
        .get(&url)
        .header(ACCEPT, "application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json");
    if let Some(token) = token.as_deref() {
        manifest_req = manifest_req.header(AUTHORIZATION, format!("Bearer {}", token));
    }
    let resp = manifest_req
        .send().await?
        .error_for_status()?
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
            let blob_url = format!("https://{}/v2/{}/blobs/{}", registry_host, repo_path, config_digest);
            let mut blob_req = client.get(&blob_url);
            if let Some(token) = token.as_deref() {
                blob_req = blob_req.header(AUTHORIZATION, format!("Bearer {}", token));
            }
            if let Ok(blob) = blob_req
                .send().await?
                .error_for_status()?
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

fn parse_image_channel_info(image_full: &str, version: &str) -> std::result::Result<ImageChannelInfo, String> {
    if image_full.is_empty() {
        return Err("Could not detect booted image".to_string());
    }

    let (original_repo_url, current_image_tag) = image_full
        .rsplit_once(':')
        .unwrap_or((image_full, ""));
    if original_repo_url.is_empty() || current_image_tag.is_empty() {
        return Err("Could not parse booted image reference".to_string());
    }

    let repo_url = canonicalize_repo_url(original_repo_url);

    let (registry_host, repo_path) = repo_url
        .split_once('/')
        .ok_or_else(|| format!("Could not parse registry repo path from: {}", repo_url))?;
    if registry_host.is_empty() || repo_path.is_empty() {
        return Err(format!("Could not parse registry repo path from: {}", repo_url));
    }

    let trimmed_version = version.trim().to_string();
    let (channel_tag, current_date_tag) = if current_image_tag.len() == 8
        && current_image_tag.chars().all(|c| c.is_ascii_digit())
    {
        ("latest".to_string(), current_image_tag.to_string())
    } else if let Some((channel, date)) = current_image_tag.rsplit_once('.') {
        if date.len() == 8 && date.chars().all(|c| c.is_ascii_digit()) {
            (channel.to_string(), date.to_string())
        } else {
            (
                current_image_tag.to_string(),
                extract_date_tag(&trimmed_version, current_image_tag),
            )
        }
    } else {
        (
            current_image_tag.to_string(),
            extract_date_tag(&trimmed_version, current_image_tag),
        )
    };

    Ok(ImageChannelInfo {
        original_repo_url: original_repo_url.to_string(),
        repo_url: repo_url.clone(),
        registry_host: registry_host.to_string(),
        repo_path: repo_path.to_string(),
        channel_tag,
        current_date_tag,
    })
}

fn extract_date_tag(version_annotation: &str, channel_tag: &str) -> String {
    let trimmed = version_annotation.trim();
    if let Some(stripped) = trimmed.strip_prefix(&format!("{}.", channel_tag)) {
        if let Some(date) = stripped
            .split(|c: char| !c.is_ascii_digit())
            .find(|part| part.len() == 8 && part.chars().all(|c| c.is_ascii_digit()))
        {
            return date.to_string();
        }
    }
    if trimmed.len() == 8 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        return trimmed.to_string();
    }
    if let Some(date) = trimmed
        .split(|c: char| !c.is_ascii_digit())
        .find(|part| part.len() == 8 && part.chars().all(|c| c.is_ascii_digit()))
    {
        return date.to_string();
    }
    String::new()
}

async fn get_registry_token(
    client: &reqwest::Client,
    registry_host: &str,
    repo_path: &str,
    channel_tag: &str,
) -> Result<Option<String>> {
    let manifest_url = format!("https://{}/v2/{}/manifests/{}", registry_host, repo_path, channel_tag);
    let resp = client
        .get(&manifest_url)
        .header(ACCEPT, "application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json")
        .send()
        .await?;

    if resp.status().is_success() {
        return Ok(None);
    }

    if resp.status() != reqwest::StatusCode::UNAUTHORIZED {
        return Err(anyhow!("Unexpected registry response from {}: HTTP {}", manifest_url, resp.status()));
    }

    let challenge = resp
        .headers()
        .get(WWW_AUTHENTICATE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| anyhow!("Registry did not provide an authentication challenge"))?;

    let realm = parse_auth_challenge_value(challenge, "realm")
        .ok_or_else(|| anyhow!("Could not parse registry auth realm"))?;
    let service = parse_auth_challenge_value(challenge, "service");
    let scope = parse_auth_challenge_value(challenge, "scope")
        .unwrap_or_else(|| format!("repository:{}:pull", repo_path));

    let mut token_req = client.get(&realm).query(&[("scope", scope.as_str())]);
    if let Some(service) = service.as_deref() {
        token_req = token_req.query(&[("service", service)]);
    }

    let token_resp = token_req
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let token = token_resp["token"]
        .as_str()
        .or_else(|| token_resp["access_token"].as_str())
        .unwrap_or("")
        .to_string();

    if token.is_empty() {
        return Err(anyhow!("Failed to get registry token for {}", repo_path));
    }

    Ok(Some(token))
}

fn parse_auth_challenge_value(header: &str, key: &str) -> Option<String> {
    let pattern = format!(r#"{}=""#, key);
    let start = header.find(&pattern)? + pattern.len();
    let rest = &header[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn canonicalize_repo_url(repo_url: &str) -> String {
    match repo_url {
        "ghcr.io/rakuos/rakuos-base" | "registry.gitlab.com/rakuos/images/rakuos-base" => {
            "registry.gitlab.com/rakuos/images/rakuos-base".to_string()
        }
        "ghcr.io/rakuos/rakuos-base-nvidia" | "registry.gitlab.com/rakuos/images/rakuos-base/nvidia" => {
            "registry.gitlab.com/rakuos/images/rakuos-base/nvidia".to_string()
        }
        "ghcr.io/rakuos/rakuos-kde" | "registry.gitlab.com/rakuos/images/rakuos-kde" => {
            "registry.gitlab.com/rakuos/images/rakuos-kde".to_string()
        }
        "ghcr.io/rakuos/rakuos-kde-nvidia" | "registry.gitlab.com/rakuos/images/rakuos-kde/nvidia" => {
            "registry.gitlab.com/rakuos/images/rakuos-kde/nvidia".to_string()
        }
        "ghcr.io/rakuos/rakuos-gnome" | "registry.gitlab.com/rakuos/images/rakuos-gnome" => {
            "registry.gitlab.com/rakuos/images/rakuos-gnome".to_string()
        }
        "ghcr.io/rakuos/rakuos-gnome-nvidia" | "registry.gitlab.com/rakuos/images/rakuos-gnome/nvidia" => {
            "registry.gitlab.com/rakuos/images/rakuos-gnome/nvidia".to_string()
        }
        "ghcr.io/rakuos/rakuos-cosmic" | "registry.gitlab.com/rakuos/images/rakuos-cosmic" => {
            "registry.gitlab.com/rakuos/images/rakuos-cosmic".to_string()
        }
        "ghcr.io/rakuos/rakuos-cosmic-nvidia" | "registry.gitlab.com/rakuos/images/rakuos-cosmic/nvidia" => {
            "registry.gitlab.com/rakuos/images/rakuos-cosmic/nvidia".to_string()
        }
        _ => repo_url.to_string(),
    }
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
