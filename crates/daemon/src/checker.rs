// daemon/checker.rs — Background update check logic

use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;

const RAKUOS_UPDATE: &str = "/usr/libexec/rakuos/rakuos-update";
// Package check runs `dnf check-update --refresh` which can be slow on first run
// or on a slow mirror — allow up to 3 minutes.
const PKG_TIMEOUT: Duration = Duration::from_secs(180);
// Flatpak and image checks are faster; 60s is plenty.
const CMD_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateResult {
    pub packages: Vec<serde_json::Value>,
    pub flatpak: Vec<serde_json::Value>,
    pub appimages: Vec<serde_json::Value>,
    pub image_available: bool,
    pub image_info: serde_json::Value,
    pub total: usize,
}

impl UpdateResult {
    pub fn notification_body(&self) -> Option<String> {
        if self.total == 0 {
            return None;
        }
        let mut parts = Vec::new();
        if !self.packages.is_empty() {
            let n = self.packages.len();
            parts.push(format!("{} package update{}", n, if n != 1 { "s" } else { "" }));
        }
        if !self.flatpak.is_empty() {
            let n = self.flatpak.len();
            parts.push(format!("{} Flatpak update{}", n, if n != 1 { "s" } else { "" }));
        }
        if !self.appimages.is_empty() {
            let n = self.appimages.len();
            parts.push(format!("{} AppImage update{}", n, if n != 1 { "s" } else { "" }));
        }
        if self.image_available {
            let ver = self.image_info["available"]
                .as_str()
                .unwrap_or("new version");
            parts.push(format!("System image {} available", ver));
        }
        Some(parts.join("\n"))
    }
}

/// Run all checks in parallel so total time ≈ slowest single check.
pub async fn run_checks(settings: &Settings) -> UpdateResult {
    let check_pkgs   = settings.auto_check_packages;
    let check_fp     = settings.auto_check_flatpak;
    let check_img    = settings.auto_check_image;
    let check_ai     = settings.auto_check_appimages;
    let auto_update  = settings.auto_update;

    log::info!("Starting update checks (packages={check_pkgs}, flatpak={check_fp}, image={check_img}, appimages={check_ai})");

    // Run all four checks concurrently
    let (pkg_res, fp_res, img_res, ai_res) = tokio::join!(
        async {
            if check_pkgs {
                log::info!("Running: {} check", RAKUOS_UPDATE);
                let res = run_rakuos_update_with_timeout("check", PKG_TIMEOUT).await;
                match &res {
                    Ok((_, pkgs)) => log::info!("Package check done: {} update(s)", pkgs.len()),
                    Err(e)        => log::warn!("Package check failed: {}", e),
                }
                res.ok()
            } else {
                log::info!("Package check skipped (disabled in settings)");
                None
            }
        },
        async {
            if check_fp {
                log::info!("Checking flatpak updates via Rust backend (icon-enriched)");
                let updates: Vec<serde_json::Value> =
                    tokio::task::spawn_blocking(|| {
                        rakuos_flatpak::get_all_updates()
                            .into_iter()
                            .filter_map(|f| serde_json::to_value(f).ok())
                            .collect()
                    })
                    .await
                    .unwrap_or_default();
                log::info!("Flatpak check done: {} update(s)", updates.len());
                Some((true, updates))
            } else {
                log::info!("Flatpak check skipped (disabled in settings)");
                None
            }
        },
        async {
            if check_img {
                log::info!("Running: {} check-image", RAKUOS_UPDATE);
                let res = run_rakuos_update_image().await;
                match &res {
                    Ok((avail, _)) => log::info!("Image check done: available={avail}"),
                    Err(e)         => log::warn!("Image check failed: {}", e),
                }
                res.ok()
            } else {
                log::info!("Image check skipped (disabled in settings)");
                None
            }
        },
        async {
            if check_ai {
                log::info!("Running AppImage update checks");
                let res = check_appimages().await;
                log::info!("AppImage check done: {} update(s)", res.len());
                res
            } else {
                log::info!("AppImage check skipped (disabled in settings)");
                vec![]
            }
        },
    );

    let mut result = UpdateResult {
        packages:        pkg_res.map(|(_, v)| v).unwrap_or_default(),
        flatpak:         fp_res.map(|(_, v)| v).unwrap_or_default(),
        appimages:       ai_res,
        image_available: img_res.as_ref().map(|(ok, _)| *ok).unwrap_or(false),
        image_info:      img_res.map(|(_, v)| v).unwrap_or(serde_json::json!({})),
        total: 0,
    };

    result.total = result.packages.len()
        + result.flatpak.len()
        + result.appimages.len()
        + if result.image_available { 1 } else { 0 };

    // Auto-install if enabled (re-check after to get accurate count)
    if auto_update {
        let (new_pkg, new_fp) = tokio::join!(
            async {
                if !result.packages.is_empty() {
                    let _ = run_command(&[RAKUOS_UPDATE, "upgrade"]).await;
                    run_rakuos_update("check").await.ok().map(|(_, v)| v)
                } else {
                    None
                }
            },
            async {
                if !result.flatpak.is_empty() {
                    let _ = run_command(&["flatpak", "update", "-y", "--noninteractive"]).await;
                    let updates: Vec<serde_json::Value> =
                        tokio::task::spawn_blocking(|| {
                            rakuos_flatpak::get_all_updates()
                                .into_iter()
                                .filter_map(|f| serde_json::to_value(f).ok())
                                .collect()
                        })
                        .await
                        .ok()
                        .unwrap_or_default();
                    Some(updates)
                } else {
                    None
                }
            },
        );
        if let Some(pkgs) = new_pkg { result.packages = pkgs; }
        if let Some(fps)  = new_fp  { result.flatpak  = fps;  }
        result.total = result.packages.len()
            + result.flatpak.len()
            + result.appimages.len()
            + if result.image_available { 1 } else { 0 };
    }

    result
}

async fn run_rakuos_update(
    command: &str,
) -> anyhow::Result<(bool, Vec<serde_json::Value>)> {
    run_rakuos_update_with_timeout(command, CMD_TIMEOUT).await
}

async fn run_rakuos_update_with_timeout(
    command: &str,
    t: Duration,
) -> anyhow::Result<(bool, Vec<serde_json::Value>)> {
    let out = timeout(
        t,
        tokio::process::Command::new(RAKUOS_UPDATE)
            .arg(command)
            .output(),
    )
    .await??;

    // exit 0 = updates available, exit 1 = none, other = error
    let code = out.status.code().unwrap_or(2);
    if code != 0 && code != 1 {
        return Ok((false, Vec::new()));
    }

    let data: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or(serde_json::json!({}));
    let updates = data["updates"].as_array().cloned().unwrap_or_default();

    Ok((out.status.success(), updates))
}

async fn run_rakuos_update_image() -> anyhow::Result<(bool, serde_json::Value)> {
    let out = timeout(
        CMD_TIMEOUT,
        tokio::process::Command::new(RAKUOS_UPDATE)
            .arg("check-image")
            .output(),
    )
    .await??;

    let data: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or(serde_json::json!({}));

    Ok((out.status.success(), data))
}

/// Check all installed AppImages in parallel, each with an individual timeout.
async fn check_appimages() -> Vec<serde_json::Value> {
    let installed = rakuos_appimages::get_installed();
    if installed.is_empty() {
        return vec![];
    }

    let tasks: Vec<_> = installed
        .into_iter()
        .map(|app| {
            tokio::spawn(async move {
                match timeout(CMD_TIMEOUT, rakuos_appimages::check_update(&app)).await {
                    Ok(Some(result)) => Some(serde_json::json!({
                        "id":              result.id,
                        "name":            result.name,
                        "current_version": result.current_version,
                        "new_version":     result.new_version,
                        "download_url":    result.download_url,
                    })),
                    _ => None,
                }
            })
        })
        .collect();

    let mut updates = Vec::new();
    for task in tasks {
        if let Ok(Some(val)) = task.await {
            updates.push(val);
        }
    }
    updates
}

async fn run_command(args: &[&str]) -> anyhow::Result<()> {
    timeout(
        CMD_TIMEOUT,
        tokio::process::Command::new(args[0])
            .args(&args[1..])
            .output(),
    )
    .await??;
    Ok(())
}
