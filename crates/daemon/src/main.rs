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

fn quit_flag_path() -> std::path::PathBuf {
    std::env::temp_dir().join("rakuos-software-quit")
}

fn check_trigger_path() -> std::path::PathBuf {
    std::env::temp_dir().join("rakuos-software-check-requested")
}

/// Returns true if the UI process is currently running (pid file + /proc check).
fn ui_is_running() -> bool {
    std::fs::read_to_string(pid_file())
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .map(|pid| std::path::Path::new(&format!("/proc/{}", pid)).exists())
        .unwrap_or(false)
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

/// If the UI is already running, write the show-flag so its polling timer
/// picks it up. Otherwise spawn a fresh instance.
fn signal_or_spawn_ui() {
    if ui_is_running() {
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
                // Signal the UI to quit via flag file (SIGTERM is blocked by the
                // close-request handler that hides instead of closing the window).
                let _ = std::fs::write(quit_flag_path(), "1");
                std::process::exit(0);
            }
            DaemonMsg::OpenUi => {
                signal_or_spawn_ui();
            }
            DaemonMsg::CheckNow => {
                if ui_is_running() {
                    // Signal the UI to run its own icon-enriched check.
                    // The UI watches for this flag, runs its full check, and writes
                    // the cache back — giving us icon-enriched results for free.
                    log::info!("UI is running — writing check trigger for UI to handle.");
                    let _ = std::fs::write(check_trigger_path(), "1");

                    // Spawn a background task that waits for the UI to write the
                    // cache (mtime change) and then syncs the badge count.
                    let tray_c = tray_handle.clone();
                    let cache_path = daemon_cache_path();
                    let initial_mtime = std::fs::metadata(&cache_path)
                        .and_then(|m| m.modified()).ok();
                    tokio::spawn(async move {
                        // Poll up to 3 minutes (36 × 5 s) for the UI to complete its check.
                        for _ in 0..36u32 {
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            let mtime = std::fs::metadata(&cache_path)
                                .and_then(|m| m.modified()).ok();
                            if mtime.is_some() && mtime != initial_mtime {
                                if let Ok(j) = std::fs::read_to_string(&cache_path) {
                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&j) {
                                        let count = v["total"].as_i64().unwrap_or(0) as usize;
                                        tray_c.update(|t| t.update_count = count);
                                        log::info!("Badge synced from UI cache: {} update(s)", count);
                                    }
                                }
                                break;
                            }
                        }
                    });
                } else {
                    // UI is not running — run our own check so the badge and
                    // notifications still work even when the window is closed.
                    log::info!("UI not running — running own update check.");
                    let settings = Settings::load();
                    let result = run_checks(&settings).await;
                    let count = result.total;

                    let cache = serde_json::json!({
                        "total":           count,
                        "packages":        result.packages,
                        "flatpak":         result.flatpak,
                        "appimages":       result.appimages,
                        "image_available": result.image_available,
                        "image_info":      result.image_info,
                    });
                    let cache_path = daemon_cache_path();
                    if let Some(parent) = cache_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(&cache_path,
                        serde_json::to_string_pretty(&cache).unwrap_or_default());

                    tray_handle.update(|t| t.update_count = count);

                    if let Some(body) = result.notification_body() {
                        send_notification("Updates Available", &body).await;
                    }

                    log::info!("Own check complete: {} update(s) found.", count);
                }
            }
        }
    }

    Ok(())
}

async fn send_notification(summary: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(summary)
        .body(body)
        .appname("RakuOS Software")
        .icon("system-software-update")
        .timeout(notify_rust::Timeout::Milliseconds(8000))
        .show_async()
        .await;
}
