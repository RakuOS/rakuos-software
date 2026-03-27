// rakuos-software-tray — Background daemon + system tray for RakuOS Software Center

mod checker;
mod settings;
mod tray;

use checker::run_checks;
use settings::Settings;
use std::sync::mpsc as stdmpsc;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum DaemonMsg {
    CheckNow,
    Quit,
    OpenUi,
}

fn pid_file() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    std::path::PathBuf::from(home).join(".cache/rakuos/software-ui.pid")
}

fn show_flag_path() -> std::path::PathBuf {
    std::env::temp_dir().join("rakuos-software-show")
}

fn daemon_cache_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    std::path::PathBuf::from(home).join(".cache/rakuos/daemon-update-cache.json")
}

/// Locate the installed UI frontend binary (gtk preferred, then qt).
/// Checks sibling directory of this binary first (covers dev builds), then libexec.
fn ui_binary() -> std::path::PathBuf {
    let libexec = std::path::Path::new("/usr/libexec/rakuos/software");
    let candidates = ["rakuos-software-gtk", "rakuos-software-qt"];

    // Check sibling directory (dev builds in target/debug/)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in candidates {
                let p = dir.join(name);
                if p.exists() {
                    return p;
                }
            }
        }
    }

    // Check installed libexec location
    for name in candidates {
        let p = libexec.join(name);
        if p.exists() {
            return p;
        }
    }

    // Last resort: hope it's on PATH
    std::path::PathBuf::from("rakuos-software-gtk")
}

/// If the UI is already running (PID file + /proc), write the show-flag so
/// the UI's polling timer picks it up. Otherwise spawn a fresh instance.
fn signal_or_spawn_ui() {
    let alive = std::fs::read_to_string(pid_file())
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .map(|pid| std::path::Path::new(&format!("/proc/{}", pid)).exists())
        .unwrap_or(false);

    if alive {
        let _ = std::fs::write(show_flag_path(), "1");
    } else {
        let _ = std::process::Command::new(ui_binary()).spawn();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // std::sync::mpsc so ksni's non-tokio thread can send without needing a runtime.
    let (std_tx, std_rx) = stdmpsc::sync_channel::<DaemonMsg>(8);

    // Spawn tray icon (registers D-Bus StatusNotifierItem)
    let tray_handle = tray::spawn(std_tx.clone())?;

    // Bridge: read from the std channel in a blocking thread and forward into
    // the tokio channel so the async loop below can await on it normally.
    let (tok_tx, mut tok_rx) = mpsc::channel::<DaemonMsg>(8);
    std::thread::spawn(move || {
        while let Ok(msg) = std_rx.recv() {
            if tok_tx.blocking_send(msg).is_err() {
                break;
            }
        }
    });

    // ── Scheduled check loop ──────────────────────────────────────────────────
    let sched_tx = std_tx.clone();
    tokio::spawn(async move {
        // Short delay so the tray registers before the first check runs
        tokio::time::sleep(Duration::from_secs(10)).await;
        loop {
            let _ = sched_tx.send(DaemonMsg::CheckNow);
            let settings = Settings::load();
            match settings.effective_interval_secs() {
                Some(secs) => tokio::time::sleep(Duration::from_secs(secs)).await,
                None => tokio::time::sleep(Duration::from_secs(u64::MAX)).await,
            }
        }
    });

    // ── Message loop ──────────────────────────────────────────────────────────
    while let Some(msg) = tok_rx.recv().await {
        match msg {
            DaemonMsg::Quit => {
                log::info!("Quitting daemon.");
                // Kill the UI if it's running
                if let Some(pid) = std::fs::read_to_string(pid_file())
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())
                {
                    if std::path::Path::new(&format!("/proc/{}", pid)).exists() {
                        unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM); }
                    }
                }
                std::process::exit(0);
            }
            DaemonMsg::OpenUi => {
                signal_or_spawn_ui();
            }
            DaemonMsg::CheckNow => {
                log::info!("Running update check...");
                let settings = Settings::load();
                let result = run_checks(&settings).await;
                let count = result.total;

                // Write results to cache file so UI can display a badge on
                // next startup without waiting for a fresh check.
                let cache = serde_json::json!({
                    "total":           count,
                    "packages":        result.packages,
                    "flatpak":         result.flatpak,
                    "appimages":       result.appimages,
                    "image_available": result.image_available,
                    "image_info":      result.image_info,
                });
                let cache_path = daemon_cache_path();
                log::info!("Writing update cache to {:?}", cache_path);
                if let Some(parent) = cache_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&cache_path, serde_json::to_string_pretty(&cache).unwrap_or_default()) {
                    Ok(_)  => log::info!("Cache written successfully ({} total update(s))", count),
                    Err(e) => log::error!("Failed to write cache: {}", e),
                }

                // Update tray icon/tooltip
                tray_handle.update(|t| t.update_count = count);

                // Desktop notification if updates found
                if let Some(body) = result.notification_body() {
                    send_notification("Updates Available", &body);
                }

                log::info!("Check complete: {} update(s) found.", count);
            }
        }
    }

    Ok(())
}

fn send_notification(summary: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(summary)
        .body(body)
        .appname("RakuOS Software")
        .icon("system-software-update")
        .timeout(notify_rust::Timeout::Milliseconds(8000))
        .show();
}
