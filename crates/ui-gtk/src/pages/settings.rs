// pages/settings.rs — Settings page with 3 tabs matching Qt SettingsPage.qml

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, CheckButton, Entry, Label, Orientation,
    ScrolledWindow, SelectionMode, Widget,
};
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, PreferencesGroup, PreferencesPage, SwitchRow, ViewStack, ViewSwitcherBar,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

// ── Settings persistence ────────────────────────────────────────────────────

fn settings_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    std::path::PathBuf::from(home).join(".config/rakuos/software-settings.json")
}

#[derive(Debug, Clone)]
struct Settings {
    update_interval: u64,
    auto_check_packages: bool,
    auto_check_flatpak: bool,
    auto_check_image: bool,
    auto_check_appimages: bool,
    auto_update: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            update_interval: 1440,
            auto_check_packages: true,
            auto_check_flatpak: true,
            auto_check_image: true,
            auto_check_appimages: true,
            auto_update: false,
        }
    }
}

impl Settings {
    fn load() -> Self {
        settings_path()
            .pipe(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .map(|v| Settings {
                update_interval:      v["update_interval"].as_u64().unwrap_or(1440),
                auto_check_packages:  v["auto_check_packages"].as_bool().unwrap_or(true),
                auto_check_flatpak:   v["auto_check_flatpak"].as_bool().unwrap_or(true),
                auto_check_image:     v["auto_check_image"].as_bool().unwrap_or(true),
                auto_check_appimages: v["auto_check_appimages"].as_bool().unwrap_or(true),
                auto_update:          v["auto_update"].as_bool().unwrap_or(false),
            })
            .unwrap_or_default()
    }

    fn save(&self) {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let v = serde_json::json!({
            "update_interval":      self.update_interval,
            "auto_check_packages":  self.auto_check_packages,
            "auto_check_flatpak":   self.auto_check_flatpak,
            "auto_check_image":     self.auto_check_image,
            "auto_check_appimages": self.auto_check_appimages,
            "auto_update":          self.auto_update,
        });
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap_or_default());
    }
}

// interval index ↔ minutes
const INTERVALS: &[u64] = &[360, 720, 1440, 10080, 0];

fn interval_to_index(v: u64) -> u32 {
    INTERVALS.iter().position(|&x| x == v).unwrap_or(2) as u32
}

// Trait so we can call .pipe() on PathBuf (avoids temp var)
trait Pipe: Sized {
    fn pipe<F: FnOnce(Self) -> R, R>(self, f: F) -> R { f(self) }
}
impl<T> Pipe for T {}

// ── Public entry point ─────────────────────────────────────────────────────

pub fn build() -> Widget {
    let outer = GBox::builder()
        .orientation(Orientation::Vertical)
        .build();

    let view_stack = ViewStack::new();
    view_stack.set_vexpand(true);

    let updates_tab = build_updates_tab();
    view_stack.add_titled_with_icon(
        &updates_tab, Some("updates"), "Updates", "software-update-available-symbolic");

    let flatpak_tab = build_flatpak_tab();
    view_stack.add_titled_with_icon(
        &flatpak_tab, Some("flatpak"), "Flatpak Repos", "package-x-generic-symbolic");

    let firmware_tab = build_firmware_tab();
    view_stack.add_titled_with_icon(
        &firmware_tab, Some("firmware"), "Firmware / LVFS", "computer-symbolic");

    let switcher = ViewSwitcherBar::builder()
        .stack(&view_stack)
        .reveal(true)
        .build();

    outer.append(&view_stack);
    outer.append(&switcher);
    outer.upcast()
}

// ── Tab 1: Updates ─────────────────────────────────────────────────────────

fn build_updates_tab() -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let pref_page = PreferencesPage::new();
    scroll.set_child(Some(&pref_page));

    let settings = Settings::load();

    // ── Update Schedule group ────────────────────────────────────────────
    let sched_group = PreferencesGroup::builder()
        .title("Update Schedule")
        .description("How often RakuOS Software checks for available updates in the background.")
        .build();

    let interval_items = gtk4::StringList::new(&[
        "Every 6 hours",
        "Every 12 hours",
        "Daily",
        "Weekly",
        "Manual only",
    ]);
    let interval_row = libadwaita::ComboRow::builder()
        .title("Check for updates")
        .model(&interval_items)
        .selected(interval_to_index(settings.update_interval))
        .build();
    sched_group.add(&interval_row);
    pref_page.add(&sched_group);

    // ── What to check group ──────────────────────────────────────────────
    let check_group = PreferencesGroup::builder()
        .title("Check for")
        .build();

    let check_pkgs = SwitchRow::builder()
        .title("Overlay package updates")
        .active(settings.auto_check_packages)
        .build();
    let check_flatpak = SwitchRow::builder()
        .title("Flatpak updates")
        .active(settings.auto_check_flatpak)
        .build();
    let check_image = SwitchRow::builder()
        .title("System image updates (bootc)")
        .active(settings.auto_check_image)
        .build();
    let check_ai = SwitchRow::builder()
        .title("AppImage updates")
        .active(settings.auto_check_appimages)
        .build();

    check_group.add(&check_pkgs);
    check_group.add(&check_flatpak);
    check_group.add(&check_image);
    check_group.add(&check_ai);
    pref_page.add(&check_group);

    // ── Auto-update group ────────────────────────────────────────────────
    let auto_group = PreferencesGroup::builder()
        .title("Automatic Updates")
        .description("System image updates always require manual approval and a reboot.")
        .build();

    let auto_update_row = SwitchRow::builder()
        .title("Automatically install package and Flatpak updates when found")
        .active(settings.auto_update)
        .build();
    auto_group.add(&auto_update_row);
    pref_page.add(&auto_group);

    // ── Save button group ────────────────────────────────────────────────
    let save_group = PreferencesGroup::new();

    let save_row = ActionRow::builder()
        .title("Save Settings")
        .build();

    let saved_lbl = Label::builder()
        .label("✓ Settings saved")
        .css_classes(vec!["success".to_string()])
        .halign(Align::Start)
        .visible(false)
        .build();

    let save_btn = Button::builder()
        .label("Save")
        .valign(Align::Center)
        .css_classes(vec!["suggested-action".to_string()])
        .build();

    save_row.add_suffix(&saved_lbl);
    save_row.add_suffix(&save_btn);
    save_group.add(&save_row);
    pref_page.add(&save_group);

    save_btn.connect_clicked(move |_| {
        let idx = interval_row.selected() as usize;
        let s = Settings {
            update_interval:      INTERVALS.get(idx).copied().unwrap_or(1440),
            auto_check_packages:  check_pkgs.is_active(),
            auto_check_flatpak:   check_flatpak.is_active(),
            auto_check_image:     check_image.is_active(),
            auto_check_appimages: check_ai.is_active(),
            auto_update:          auto_update_row.is_active(),
        };
        s.save();
        saved_lbl.set_visible(true);
        let lbl_c = saved_lbl.clone();
        glib::timeout_add_local_once(Duration::from_secs(2), move || {
            lbl_c.set_visible(false);
        });
    });

    scroll.upcast()
}

// ── Tab 2: Flatpak Repos ───────────────────────────────────────────────────

fn build_flatpak_tab() -> Widget {
    let outer = GBox::builder()
        .orientation(Orientation::Vertical)
        .build();

    // Header bar
    let header_box = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(8)
        .margin_start(16)
        .margin_end(16)
        .build();

    let title_col = GBox::builder()
        .orientation(Orientation::Vertical)
        .hexpand(true)
        .build();
    title_col.append(&Label::builder()
        .label("Flatpak Repositories")
        .halign(Align::Start)
        .css_classes(vec!["title-4".to_string()])
        .build());
    title_col.append(&Label::builder()
        .label("Manage Flatpak repositories. System remotes are available to all users.")
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .wrap(true)
        .build());
    header_box.append(&title_col);

    let status_lbl: Rc<RefCell<Label>> = Rc::new(RefCell::new(
        Label::builder()
            .label("")
            .halign(Align::Start)
            .visible(false)
            .build()
    ));

    // The remotes list container (refreshable)
    let list_box = gtk4::ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .margin_top(8)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .build();

    let list_rc: Rc<gtk4::ListBox> = Rc::new(list_box);

    // Add Flathub button (hidden if already present)
    let add_flathub_btn = Button::builder()
        .label("Add Flathub")
        .valign(Align::Center)
        .css_classes(vec!["suggested-action".to_string()])
        .visible(!rakuos_flatpak::has_flathub())
        .build();
    header_box.append(&add_flathub_btn);

    // Add Remote button
    let add_remote_btn = Button::builder()
        .label("Add Remote")
        .valign(Align::Center)
        .build();
    header_box.append(&add_remote_btn);

    outer.append(&header_box);

    // Status label row
    let status_lbl_widget = status_lbl.borrow().clone();
    let status_row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .margin_start(16)
        .margin_end(16)
        .build();
    status_row.append(&status_lbl_widget);
    outer.append(&status_row);

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();
    scroll.set_child(Some(&*list_rc));
    outer.append(&scroll);

    // Populate remotes
    let list_c = Rc::clone(&list_rc);
    let status_c = Rc::clone(&status_lbl);
    let populate = move || {
        let list = &*list_c;
        while let Some(row) = list.first_child() {
            list.remove(&row);
        }
        let remotes = rakuos_flatpak::get_remotes();
        if remotes.is_empty() {
            list.append(&build_empty_list_row("No Flatpak remotes configured."));
        } else {
            for remote in &remotes {
                let row = build_flatpak_remote_row(remote, Rc::clone(&status_c), Rc::clone(&list_c));
                list.append(&row);
            }
        }
    };

    populate();

    // Wrap populate in Rc for button closures
    let list_pop = Rc::clone(&list_rc);
    let status_pop = Rc::clone(&status_lbl);
    let add_flathub_btn_c = add_flathub_btn.clone();

    add_flathub_btn.connect_clicked(move |btn| {
        btn.set_sensitive(false);
        let (tx, rx) = mpsc::channel::<(bool, String)>();
        std::thread::spawn(move || {
            let (ok, msg) = rakuos_flatpak::add_flathub(true);
            let _ = tx.send((ok, msg));
        });
        let list_c2 = Rc::clone(&list_pop);
        let status_c2 = Rc::clone(&status_pop);
        let btn_c = btn.clone();
        let flathub_btn_c = add_flathub_btn_c.clone();
        glib::timeout_add_local(Duration::from_millis(80), move || {
            match rx.try_recv() {
                Ok((ok, msg)) => {
                    set_status(&status_c2, &msg, ok);
                    refresh_flatpak_list(&list_c2, &status_c2);
                    if ok { flathub_btn_c.set_visible(false); }
                    btn_c.set_sensitive(true);
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });

    // Add Remote dialog
    let list_for_dialog = Rc::clone(&list_rc);
    let status_for_dialog = Rc::clone(&status_lbl);
    add_remote_btn.connect_clicked(move |btn| {
        show_add_remote_dialog(
            btn.upcast_ref::<gtk4::Widget>(),
            Rc::clone(&list_for_dialog),
            Rc::clone(&status_for_dialog),
        );
    });

    outer.upcast()
}

fn build_flatpak_remote_row(
    remote: &rakuos_flatpak::FlatpakRemote,
    status: Rc<RefCell<Label>>,
    list: Rc<gtk4::ListBox>,
) -> Widget {
    let row_box = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .build();

    // Enable/disable toggle
    let toggle = CheckButton::new();
    toggle.set_active(remote.enabled);
    toggle.set_valign(Align::Center);
    row_box.append(&toggle);

    let info_col = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .valign(Align::Center)
        .build();

    let name_row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .build();
    name_row.append(&Label::builder()
        .label(if remote.title.is_empty() { &remote.name } else { &remote.title })
        .css_classes(vec!["heading".to_string()])
        .halign(Align::Start)
        .build());

    let scope_lbl = Label::builder()
        .label(if remote.system { "system" } else { "user" })
        .css_classes(vec!["caption".to_string(), if remote.system { "accent" } else { "dim-label" }.to_string()])
        .halign(Align::Start)
        .build();
    name_row.append(&scope_lbl);
    info_col.append(&name_row);

    if !remote.url.is_empty() {
        info_col.append(&Label::builder()
            .label(&remote.url)
            .halign(Align::Start)
            .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build());
    }
    row_box.append(&info_col);

    let remove_btn = Button::builder()
        .label("Remove")
        .valign(Align::Center)
        .css_classes(vec!["destructive-action".to_string()])
        .build();
    row_box.append(&remove_btn);

    // Toggle handler
    let name_t = remote.name.clone();
    let system_t = remote.system;
    let status_t = Rc::clone(&status);
    let list_t = Rc::clone(&list);
    toggle.connect_toggled(move |chk| {
        let enabled = chk.is_active();
        let n = name_t.clone();
        let (tx, rx) = mpsc::channel::<(bool, String)>();
        std::thread::spawn(move || {
            let res = rakuos_flatpak::set_remote_enabled(&n, enabled, system_t);
            let _ = tx.send(res);
        });
        let s = Rc::clone(&status_t);
        let l = Rc::clone(&list_t);
        let chk_c = chk.clone();
        glib::timeout_add_local(Duration::from_millis(80), move || {
            match rx.try_recv() {
                Ok((ok, msg)) => {
                    set_status(&s, &msg, ok);
                    if !ok { chk_c.set_active(!enabled); } // revert
                    if ok { refresh_flatpak_list(&l, &s); }
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });

    // Remove handler
    let name_r = remote.name.clone();
    let system_r = remote.system;
    let status_r = Rc::clone(&status);
    let list_r = Rc::clone(&list);
    remove_btn.connect_clicked(move |btn| {
        btn.set_sensitive(false);
        let n = name_r.clone();
        let (tx, rx) = mpsc::channel::<(bool, String)>();
        std::thread::spawn(move || {
            let res = rakuos_flatpak::remove_remote(&n, system_r);
            let _ = tx.send(res);
        });
        let s = Rc::clone(&status_r);
        let l = Rc::clone(&list_r);
        let btn_c = btn.clone();
        glib::timeout_add_local(Duration::from_millis(80), move || {
            match rx.try_recv() {
                Ok((ok, msg)) => {
                    set_status(&s, &msg, ok);
                    if ok { refresh_flatpak_list(&l, &s); }
                    else  { btn_c.set_sensitive(true); }
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });

    let action_row = libadwaita::ActionRow::new();
    action_row.set_child(Some(&row_box));
    action_row.upcast()
}

fn show_add_remote_dialog(
    parent: &impl IsA<gtk4::Widget>,
    list: Rc<gtk4::ListBox>,
    status: Rc<RefCell<Label>>,
) {
    let dialog = libadwaita::Dialog::builder()
        .title("Add Flatpak Remote")
        .content_width(420)
        .build();

    let toolbar = libadwaita::ToolbarView::new();
    toolbar.add_top_bar(&libadwaita::HeaderBar::new());

    let content = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(16).margin_bottom(16)
        .margin_start(16).margin_end(16)
        .build();

    let name_entry = Entry::builder()
        .placeholder_text("remote-name")
        .hexpand(true)
        .build();
    let url_entry = Entry::builder()
        .placeholder_text("https://…")
        .hexpand(true)
        .build();
    let system_chk = CheckButton::builder()
        .label("Install as system-wide remote (recommended)")
        .active(true)
        .build();
    let dlg_status = Label::builder()
        .label("")
        .halign(Align::Start)
        .css_classes(vec!["error".to_string()])
        .visible(false)
        .wrap(true)
        .build();

    content.append(&Label::builder().label("Name (e.g. flathub):").halign(Align::Start).build());
    content.append(&name_entry);
    content.append(&Label::builder().label("URL (.flatpakrepo or repository URL):").halign(Align::Start).build());
    content.append(&url_entry);
    content.append(&system_chk);
    content.append(&dlg_status);

    let btn_row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .halign(Align::End)
        .build();

    let cancel_btn = Button::builder().label("Cancel").build();
    let ok_btn = Button::builder()
        .label("Add Remote")
        .css_classes(vec!["suggested-action".to_string()])
        .build();
    btn_row.append(&cancel_btn);
    btn_row.append(&ok_btn);
    content.append(&btn_row);

    toolbar.set_content(Some(&content));
    dialog.set_child(Some(&toolbar));

    let dialog_c = dialog.clone();
    cancel_btn.connect_clicked(move |_| { dialog_c.close(); });

    let dialog_ok = dialog.clone();
    let name_e = name_entry.clone();
    let url_e = url_entry.clone();
    let sys_chk = system_chk.clone();
    let dlg_s = dlg_status.clone();
    ok_btn.connect_clicked(move |btn| {
        let name = name_e.text().to_string();
        let url  = url_e.text().to_string();
        if name.trim().is_empty() || url.trim().is_empty() {
            dlg_s.set_label("Name and URL are required.");
            dlg_s.set_visible(true);
            return;
        }
        btn.set_sensitive(false);
        let system = sys_chk.is_active();
        let n = name.clone(); let u = url.clone();
        let (tx, rx) = mpsc::channel::<(bool, String)>();
        std::thread::spawn(move || {
            let res = rakuos_flatpak::add_remote(&n, &u, system);
            let _ = tx.send(res);
        });
        let s = Rc::clone(&status);
        let l = Rc::clone(&list);
        let btn_c = btn.clone();
        let dlg_cc = dialog_ok.clone();
        let dlg_sc = dlg_s.clone();
        glib::timeout_add_local(Duration::from_millis(80), move || {
            match rx.try_recv() {
                Ok((ok, msg)) => {
                    if ok {
                        set_status(&s, &msg, true);
                        refresh_flatpak_list(&l, &s);
                        dlg_cc.close();
                    } else {
                        dlg_sc.set_label(&msg);
                        dlg_sc.set_visible(true);
                        btn_c.set_sensitive(true);
                    }
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });

    dialog.present(Some(parent));
}

fn refresh_flatpak_list(list: &gtk4::ListBox, status: &Rc<RefCell<Label>>) {
    while let Some(row) = list.first_child() {
        list.remove(&row);
    }
    let remotes = rakuos_flatpak::get_remotes();
    if remotes.is_empty() {
        list.append(&build_empty_list_row("No Flatpak remotes configured."));
    } else {
        for remote in &remotes {
            let row = build_flatpak_remote_row(remote, Rc::clone(status), Rc::new(list.clone()));
            list.append(&row);
        }
    }
}

fn set_status(status: &Rc<RefCell<Label>>, msg: &str, ok: bool) {
    let lbl = status.borrow().clone();
    lbl.set_label(msg);
    if ok {
        lbl.remove_css_class("error");
        lbl.add_css_class("success");
    } else {
        lbl.remove_css_class("success");
        lbl.add_css_class("error");
    }
    lbl.set_visible(!msg.is_empty());
}

// ── Tab 3: Firmware / LVFS ─────────────────────────────────────────────────

fn build_firmware_tab() -> Widget {
    let outer = GBox::builder()
        .orientation(Orientation::Vertical)
        .build();

    // Header
    let header_box = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(12).margin_bottom(8)
        .margin_start(16).margin_end(16)
        .build();

    let title_col = GBox::builder()
        .orientation(Orientation::Vertical)
        .hexpand(true)
        .build();
    title_col.append(&Label::builder()
        .label("Firmware & LVFS")
        .halign(Align::Start)
        .css_classes(vec!["title-4".to_string()])
        .build());
    title_col.append(&Label::builder()
        .label("Manage firmware update sources via fwupd. LVFS provides updates from hardware vendors.")
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .wrap(true)
        .build());
    header_box.append(&title_col);

    let refresh_btn = Button::builder()
        .label("Refresh Metadata")
        .valign(Align::Center)
        .sensitive(rakuos_firmware::fwupd_available())
        .build();
    header_box.append(&refresh_btn);
    outer.append(&header_box);

    // Not available warning
    if !rakuos_firmware::fwupd_available() {
        let warn = Label::builder()
            .label("⚠  fwupd is not installed. Install it to manage firmware updates.")
            .halign(Align::Start)
            .css_classes(vec!["warning".to_string()])
            .margin_start(16).margin_end(16)
            .wrap(true)
            .build();
        outer.append(&warn);
    }

    // Status + log
    let fw_status = Label::builder()
        .label("")
        .halign(Align::Start)
        .margin_start(16).margin_end(16)
        .visible(false)
        .wrap(true)
        .build();
    outer.append(&fw_status);

    let log_lbl = Label::builder()
        .label("")
        .halign(Align::Start)
        .css_classes(vec!["monospace".to_string()])
        .margin_start(16).margin_end(16)
        .visible(false)
        .wrap(true)
        .build();
    outer.append(&log_lbl);

    // Remotes list
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let list = gtk4::ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .margin_top(8).margin_bottom(12)
        .margin_start(16).margin_end(16)
        .build();

    // Populate firmware remotes
    let remotes = rakuos_firmware::get_remotes();
    if remotes.is_empty() {
        list.append(&build_empty_list_row("No firmware remotes configured."));
    } else {
        for remote in &remotes {
            list.append(&build_firmware_remote_row(remote));
        }
    }

    // Vendor dirs
    let vendor_dirs = rakuos_firmware::get_vendor_dirs();
    if !vendor_dirs.is_empty() {
        list.append(&build_empty_list_row(&format!(
            "Vendor Directories: {} found", vendor_dirs.len()
        )));
        for vd in &vendor_dirs {
            let row = ActionRow::builder()
                .title(if vd.title.is_empty() { &vd.id } else { &vd.title })
                .subtitle(&vd.filename)
                .build();
            list.append(&row);
        }
    }

    scroll.set_child(Some(&list));
    outer.append(&scroll);

    // Refresh button
    let fw_status_c = fw_status.clone();
    let log_lbl_c = log_lbl.clone();
    refresh_btn.connect_clicked(move |btn| {
        if !rakuos_firmware::fwupd_available() { return; }
        btn.set_sensitive(false);
        btn.set_label("Refreshing…");
        fw_status_c.set_visible(false);
        log_lbl_c.set_label("");
        log_lbl_c.set_visible(true);

        let (tx, rx) = mpsc::channel::<(bool, Vec<String>)>();
        std::thread::spawn(move || {
            let mut lines = Vec::new();
            let mut ok = false;
            for line in rakuos_firmware::refresh_stream() {
                if let Some(code) = line.strip_prefix("__done__") {
                    ok = code.trim() == "0";
                } else {
                    lines.push(line);
                }
            }
            let _ = tx.send((ok, lines));
        });

        let btn_c = btn.clone();
        let fw_s = fw_status_c.clone();
        let log_c = log_lbl_c.clone();
        glib::timeout_add_local(Duration::from_millis(80), move || {
            match rx.try_recv() {
                Ok((ok, lines)) => {
                    log_c.set_label(&lines.join("\n"));
                    fw_s.set_label(if ok {
                        "✓ Firmware metadata refreshed."
                    } else {
                        "✗ Refresh failed."
                    });
                    if ok {
                        fw_s.remove_css_class("error");
                        fw_s.add_css_class("success");
                    } else {
                        fw_s.remove_css_class("success");
                        fw_s.add_css_class("error");
                    }
                    fw_s.set_visible(true);
                    btn_c.set_label("Refresh Metadata");
                    btn_c.set_sensitive(true);
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });

    outer.upcast()
}

fn build_firmware_remote_row(remote: &rakuos_firmware::FwupdRemote) -> Widget {
    let row_box = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(10).margin_bottom(10)
        .margin_start(12).margin_end(12)
        .build();

    let toggle = CheckButton::new();
    toggle.set_active(remote.enabled);
    toggle.set_valign(Align::Center);
    row_box.append(&toggle);

    let info_col = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .valign(Align::Center)
        .build();

    let name_row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .build();
    name_row.append(&Label::builder()
        .label(if remote.title.is_empty() { &remote.id } else { &remote.title })
        .css_classes(vec!["heading".to_string()])
        .halign(Align::Start)
        .build());
    if !remote.kind.is_empty() {
        name_row.append(&Label::builder()
            .label(&remote.kind)
            .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
            .build());
    }
    info_col.append(&name_row);

    let url_str = if !remote.url.is_empty() { &remote.url } else { &remote.metadata_uri };
    if !url_str.is_empty() {
        info_col.append(&Label::builder()
            .label(url_str)
            .halign(Align::Start)
            .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build());
    }
    if !remote.last_updated.is_empty() {
        info_col.append(&Label::builder()
            .label(&format!("Updated: {}", remote.last_updated))
            .halign(Align::Start)
            .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
            .build());
    }
    row_box.append(&info_col);

    let remote_id = remote.id.clone();
    toggle.connect_toggled(move |chk| {
        let enabled = chk.is_active();
        let id = remote_id.clone();
        let (tx, rx) = mpsc::channel::<(bool, String)>();
        std::thread::spawn(move || {
            let res = rakuos_firmware::set_remote_enabled(&id, enabled);
            let _ = tx.send(res);
        });
        let chk_c = chk.clone();
        glib::timeout_add_local(Duration::from_millis(80), move || {
            match rx.try_recv() {
                Ok((ok, _)) => {
                    if !ok { chk_c.set_active(!enabled); }
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });

    let action_row = ActionRow::new();
    action_row.set_child(Some(&row_box));
    action_row.upcast()
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn build_empty_list_row(msg: &str) -> Widget {
    let lbl = Label::builder()
        .label(msg)
        .halign(Align::Center)
        .margin_top(24).margin_bottom(24)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let row = ActionRow::new();
    row.set_child(Some(&lbl));
    row.upcast()
}
