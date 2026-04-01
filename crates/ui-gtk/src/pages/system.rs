// pages/system.rs — System info and management page

use gtk4::prelude::*;
use gtk4::{glib, Align, Box as GBox, Button, Label, Orientation, ScrolledWindow, Widget};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, PreferencesGroup, PreferencesPage};
use std::sync::mpsc;
use std::time::Duration;

use rakuos_updates::{PkgrootStatus, SystemStatus};

pub fn build() -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let pref_page = PreferencesPage::new();
    scroll.set_child(Some(&pref_page));

    // ── System Info group ─────────────────────────────────────────────────
    let sys_group = PreferencesGroup::builder()
        .title("System Information")
        .build();

    // OS version row
    let os_row = ActionRow::builder()
        .title("OS Image")
        .subtitle("Loading…")
        .build();
    sys_group.add(&os_row);

    let version_row = ActionRow::builder()
        .title("Version")
        .subtitle("Loading…")
        .build();
    sys_group.add(&version_row);

    let digest_row = ActionRow::builder()
        .title("Image Digest")
        .subtitle("Loading…")
        .build();
    sys_group.add(&digest_row);

    // Kernel version
    let kernel_version = get_kernel_version();
    let kernel_row = ActionRow::builder()
        .title("Kernel")
        .subtitle(&kernel_version)
        .build();
    sys_group.add(&kernel_row);

    // Flatpak version
    let flatpak_version = get_flatpak_version();
    let fp_row = ActionRow::builder()
        .title("Flatpak")
        .subtitle(&flatpak_version)
        .build();
    sys_group.add(&fp_row);

    pref_page.add(&sys_group);

    // ── Installed Packages group ──────────────────────────────────────────
    let overlay_group = PreferencesGroup::builder()
        .title("Installed Packages")
        .description("Packages installed into the RakuOS package root")
        .build();

    let overlay_count_row = ActionRow::builder()
        .title("Installed Packages")
        .subtitle("Loading…")
        .build();
    overlay_group.add(&overlay_count_row);

    let overlay_dirty_row = ActionRow::builder()
        .title("Package Root Status")
        .subtitle("Loading…")
        .build();
    overlay_group.add(&overlay_dirty_row);

    pref_page.add(&overlay_group);

    // ── Image Management group ────────────────────────────────────────────
    let image_group = PreferencesGroup::builder()
        .title("Image Management")
        .build();

    let rollback_row = ActionRow::builder()
        .title("Rollback to Previous Image")
        .subtitle("Revert to the previously booted OS image")
        .build();

    let rollback_btn = Button::builder()
        .label("Rollback")
        .valign(Align::Center)
        .css_classes(vec!["destructive-action".to_string()])
        .build();

    rollback_btn.connect_clicked(move |btn| {
        btn.set_sensitive(false);
        btn.set_label("Rolling back…");
        let (tx, rx) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            let _: Vec<_> = rakuos_updates::rollback_stream().collect();
            let _ = tx.send(());
        });
        let btn_c = btn.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            match rx.try_recv() {
                Ok(_) => {
                    btn_c.set_label("Reboot to Apply");
                    let _ = rakuos_updates::schedule_reboot();
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });

    rollback_row.add_suffix(&rollback_btn);
    image_group.add(&rollback_row);

    pref_page.add(&image_group);

    // ── Firmware group ────────────────────────────────────────────────────
    let firmware_group = PreferencesGroup::builder()
        .title("Firmware Updates")
        .description("Check for firmware updates via LVFS")
        .build();

    let firmware_row = ActionRow::builder()
        .title("Check for Firmware Updates")
        .subtitle("Uses fwupdmgr to query LVFS")
        .build();

    let fw_btn = Button::builder()
        .label("Refresh")
        .valign(Align::Center)
        .css_classes(vec!["pill".to_string()])
        .build();

    fw_btn.connect_clicked(move |btn| {
        btn.set_sensitive(false);
        btn.set_label("Checking…");
        let (tx, rx) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            let _ = std::process::Command::new("fwupdmgr")
                .arg("refresh")
                .status();
            let _ = tx.send(());
        });
        let btn_c = btn.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            match rx.try_recv() {
                Ok(_) => {
                    btn_c.set_label("Refresh");
                    btn_c.set_sensitive(true);
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });

    firmware_row.add_suffix(&fw_btn);
    firmware_group.add(&firmware_row);

    pref_page.add(&firmware_group);

    // ── Load system status in background ─────────────────────────────────
    let (tx, rx) = mpsc::channel::<(SystemStatus, PkgrootStatus)>();

    std::thread::spawn(move || {
        let status = rakuos_updates::get_system_status();
        let overlay = rakuos_updates::get_pkgroot_status();
        let _ = tx.send((status, overlay));
    });

    glib::timeout_add_local(Duration::from_millis(80), move || {
        match rx.try_recv() {
            Ok((status, overlay)) => {
                if !status.image.is_empty() {
                    os_row.set_subtitle(&status.image);
                } else {
                    os_row.set_subtitle("Unknown");
                }

                if !status.version.is_empty() {
                    version_row.set_subtitle(&status.version);
                } else {
                    version_row.set_subtitle("Unknown");
                }

                if !status.digest.is_empty() {
                    let short = if status.digest.len() > 20 {
                        format!("{}…", &status.digest[..20])
                    } else {
                        status.digest.clone()
                    };
                    digest_row.set_subtitle(&short);
                } else {
                    digest_row.set_subtitle("Unknown");
                }

                overlay_count_row.set_subtitle(&format!("{} packages", overlay.package_count));

                let overlay_status_text = if overlay.is_dirty {
                    "Modified (sync recommended)"
                } else if overlay.has_digest {
                    "In sync with base image"
                } else {
                    "No package root state recorded"
                };
                overlay_dirty_row.set_subtitle(overlay_status_text);

                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });

    scroll.upcast()
}

fn get_kernel_version() -> String {
    std::process::Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn get_flatpak_version() -> String {
    std::process::Command::new("flatpak")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Not installed".to_string())
}
