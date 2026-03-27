// pages/settings.rs — Settings / preferences page

use gtk4::prelude::*;
use gtk4::{Orientation, ScrolledWindow, Widget};
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, PreferencesGroup, PreferencesPage, SpinRow, SwitchRow,
};

pub fn build() -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let pref_page = PreferencesPage::new();
    scroll.set_child(Some(&pref_page));

    // ── Update Settings group ─────────────────────────────────────────────
    let updates_group = PreferencesGroup::builder()
        .title("Updates")
        .description("Configure automatic update checks")
        .build();

    let auto_check_row = SwitchRow::builder()
        .title("Check for Updates Automatically")
        .subtitle("Periodically check for system and app updates")
        .active(true)
        .build();
    updates_group.add(&auto_check_row);

    let auto_notify_row = SwitchRow::builder()
        .title("Show Update Notifications")
        .subtitle("Notify when updates are available")
        .active(true)
        .build();
    updates_group.add(&auto_notify_row);

    // Update interval spin row
    let interval_row = SpinRow::builder()
        .title("Check Interval (hours)")
        .subtitle("How often to check for updates")
        .value(6.0)
        .build();
    interval_row.set_range(1.0, 168.0);
    interval_row.set_increments(1.0, 6.0);
    updates_group.add(&interval_row);

    pref_page.add(&updates_group);

    // ── Sources group ─────────────────────────────────────────────────────
    let sources_group = PreferencesGroup::builder()
        .title("Software Sources")
        .description("Manage where software is installed from")
        .build();

    let flathub_row = SwitchRow::builder()
        .title("Flathub")
        .subtitle("The largest repository of Flatpak apps")
        .active(rakuos_flatpak::has_flathub())
        .build();
    sources_group.add(&flathub_row);

    flathub_row.connect_active_notify(move |row| {
        let enabled = row.is_active();
        std::thread::spawn(move || {
            if enabled {
                rakuos_flatpak::add_flathub(false);
            } else {
                rakuos_flatpak::remove_remote("flathub", false);
            }
        });
    });

    let terra_row = SwitchRow::builder()
        .title("Terra Repository")
        .subtitle("RakuOS community package repository")
        .active(true)
        .build();
    sources_group.add(&terra_row);

    pref_page.add(&sources_group);

    // ── Appearance group ──────────────────────────────────────────────────
    let appearance_group = PreferencesGroup::builder()
        .title("Appearance")
        .build();

    let show_source_row = SwitchRow::builder()
        .title("Show Package Source")
        .subtitle("Display whether an app is RPM or Flatpak")
        .active(true)
        .build();
    appearance_group.add(&show_source_row);

    pref_page.add(&appearance_group);

    scroll.upcast()
}
