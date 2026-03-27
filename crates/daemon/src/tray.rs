// daemon/tray.rs — System tray via D-Bus StatusNotifierItem (ksni)
// Works on KDE Plasma, GNOME (with AppIndicator extension), COSMIC.

use ksni::Tray;
use std::sync::mpsc;

use crate::DaemonMsg;

pub struct SoftwareTray {
    pub update_count: usize,
    pub tx: mpsc::SyncSender<DaemonMsg>,
}

impl Tray for SoftwareTray {
    fn id(&self) -> String {
        "org.rakuos.Software".into()
    }

    fn icon_name(&self) -> String {
        if self.update_count > 0 {
            "software-update-available".into()
        } else {
            "system-software-update".into()
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        Vec::new()
    }

    fn title(&self) -> String {
        if self.update_count > 0 {
            format!(
                "RakuOS Software — {} update{} available",
                self.update_count,
                if self.update_count != 1 { "s" } else { "" }
            )
        } else {
            "RakuOS Software".into()
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: self.icon_name(),
            icon_pixmap: Vec::new(),
            title: self.title(),
            description: String::new(),
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Open RakuOS Software".into(),
                icon_name: "system-software-install".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(DaemonMsg::OpenUi);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Check for Updates".into(),
                icon_name: "view-refresh".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(DaemonMsg::CheckNow);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(DaemonMsg::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Spawn the ksni tray service. Returns a (handle, std receiver) pair.
/// The caller bridges the std receiver into the async message loop.
pub fn spawn(
    tx: mpsc::SyncSender<DaemonMsg>,
) -> anyhow::Result<ksni::Handle<SoftwareTray>> {
    let service = ksni::TrayService::new(SoftwareTray {
        update_count: 0,
        tx,
    });
    let handle = service.handle();
    service.spawn();
    Ok(handle)
}
