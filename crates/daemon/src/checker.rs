// daemon/checker.rs — Background update check logic

use crate::settings::Settings;
use serde::{Deserialize, Serialize};

const RAKUOS_UPDATE: &str = "/usr/libexec/rakuos/rakuos-update";

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

pub async fn run_checks(settings: &Settings) -> UpdateResult {
    let mut result = UpdateResult::default();

    if settings.auto_check_packages {
        if let Ok((_, pkgs)) = run_rakuos_update("check").await {
            result.packages = pkgs;
        }
    }

    if settings.auto_check_flatpak {
        if let Ok((_, fps)) = run_rakuos_update("check-flatpak").await {
            result.flatpak = fps;
        }
    }

    if settings.auto_check_image {
        if let Ok((avail, info)) = run_rakuos_update_image().await {
            result.image_available = avail;
            result.image_info = info;
        }
    }

    if settings.auto_check_appimages {
        // AppImage checks are lightweight — no sudo needed
        result.appimages = check_appimages().await;
    }

    result.total = result.packages.len()
        + result.flatpak.len()
        + result.appimages.len()
        + if result.image_available { 1 } else { 0 };

    // Auto-install packages/flatpaks if enabled (never auto-update image)
    if settings.auto_update {
        if !result.packages.is_empty() {
            let _ = run_command(&[RAKUOS_UPDATE, "upgrade"]).await;
            if let Ok((_, pkgs)) = run_rakuos_update("check").await {
                result.packages = pkgs;
            }
        }
        if !result.flatpak.is_empty() {
            let _ = run_command(&["flatpak", "update", "-y", "--noninteractive"]).await;
            if let Ok((_, fps)) = run_rakuos_update("check-flatpak").await {
                result.flatpak = fps;
            }
        }
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
    let out = tokio::process::Command::new(RAKUOS_UPDATE)
        .arg(command)
        .output()
        .await?;

    // exit 0 = updates available, exit 1 = none, other = error
    let code = out.status.code().unwrap_or(2);
    if code != 0 && code != 1 {
        return Ok((false, Vec::new()));
    }

    let data: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or(serde_json::json!({}));
    let updates = data["updates"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    Ok((out.status.success(), updates))
}

async fn run_rakuos_update_image() -> anyhow::Result<(bool, serde_json::Value)> {
    let out = tokio::process::Command::new(RAKUOS_UPDATE)
        .arg("check-image")
        .output()
        .await?;

    let data: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or(serde_json::json!({}));

    Ok((out.status.success(), data))
}

async fn check_appimages() -> Vec<serde_json::Value> {
    // Reads installed AppImage sidecars and checks for updates
    let installed = rakuos_appimages::get_installed();
    let mut updates = Vec::new();
    for app in &installed {
        if let Some(result) = rakuos_appimages::check_update(app).await {
            updates.push(serde_json::json!({
                "id": result.id,
                "name": result.name,
                "current_version": result.current_version,
                "new_version": result.new_version,
                "download_url": result.download_url,
            }));
        }
    }
    updates
}

async fn run_command(args: &[&str]) -> anyhow::Result<()> {
    tokio::process::Command::new(args[0])
        .args(&args[1..])
        .output()
        .await?;
    Ok(())
}
