// rakuos-software-tray — Background daemon + system tray for RakuOS Software Center

mod checker;
mod settings;
mod tray;

use checker::run_checks;
use settings::Settings;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum DaemonMsg {
    CheckNow,
    Quit,
    OpenUi,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let (tx, mut rx) = mpsc::channel::<DaemonMsg>(8);

    // Spawn tray icon (registers D-Bus StatusNotifierItem)
    let tray_handle = tray::spawn(tx.clone())?;

    // ── Scheduled check loop ──────────────────────────────────────────────────
    let tx_sched = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            let _ = tx_sched.send(DaemonMsg::CheckNow).await;
            let settings = Settings::load();
            match settings.effective_interval_secs() {
                Some(secs) => tokio::time::sleep(Duration::from_secs(secs)).await,
                None => tokio::time::sleep(Duration::from_secs(u64::MAX)).await,
            }
        }
    });

    // ── Message loop ──────────────────────────────────────────────────────────
    while let Some(msg) = rx.recv().await {
        match msg {
            DaemonMsg::Quit => {
                log::info!("Quitting daemon.");
                std::process::exit(0);
            }
            DaemonMsg::OpenUi => {
                let _ = std::process::Command::new("rakuos-software").spawn();
            }
            DaemonMsg::CheckNow => {
                log::info!("Running update check...");
                let settings = Settings::load();
                let result = run_checks(&settings).await;
                let count = result.total;

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
        .icon("software-update-available")
        .timeout(notify_rust::Timeout::Milliseconds(8000))
        .show();
}
