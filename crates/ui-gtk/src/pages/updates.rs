// pages/updates.rs — System and Flatpak updates page

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, Label, Orientation, ProgressBar,
    ScrolledWindow, Spinner, Widget,
};
use libadwaita::prelude::*;
use libadwaita::NavigationView;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use rakuos_flatpak::FlatpakUpdate;
use rakuos_updates::UpdateInfo;

// ── Shared operation state ────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct UpdateState {
    active:  Rc<RefCell<String>>,
    running: Rc<RefCell<bool>>,
}

impl UpdateState {
    fn is_running(&self) -> bool { *self.running.borrow() }
    fn start(&self, tag: &str) {
        *self.running.borrow_mut() = true;
        *self.active.borrow_mut() = tag.to_string();
    }
    fn stop(&self) {
        *self.running.borrow_mut() = false;
        *self.active.borrow_mut() = String::new();
    }
}

// ── Page entry point ──────────────────────────────────────────────────────────

pub fn build(_nav: Arc<NavigationView>) -> Widget {
    let root = GBox::builder()
        .orientation(Orientation::Vertical)
        .build();

    // ── Top action bar ─────────────────────────────────────────────────────
    let toolbar = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(16)
        .margin_end(16)
        .build();

    let status_lbl = Label::builder()
        .label("Checking for updates…")
        .halign(Align::Start)
        .hexpand(true)
        .css_classes(vec!["heading".to_string()])
        .build();

    let busy = Spinner::builder()
        .spinning(true)
        .build();

    let check_btn = Button::builder()
        .label("Check for Updates")
        .build();

    let update_all_btn = Button::builder()
        .label("Update All")
        .css_classes(vec!["suggested-action".to_string()])
        .sensitive(false)
        .visible(false)
        .build();

    let reboot_btn = Button::builder()
        .label("Reboot Now")
        .css_classes(vec!["destructive-action".to_string()])
        .visible(false)
        .build();

    toolbar.append(&status_lbl);
    toolbar.append(&busy);
    toolbar.append(&check_btn);
    toolbar.append(&update_all_btn);
    toolbar.append(&reboot_btn);
    root.append(&toolbar);
    root.append(&gtk4::Separator::new(Orientation::Horizontal));

    // ── Scrollable content ─────────────────────────────────────────────────
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let content = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(16)
        .margin_bottom(24)
        .margin_start(16)
        .margin_end(16)
        .build();
    scroll.set_child(Some(&content));
    root.append(&scroll);

    let init_spinner = Spinner::builder()
        .spinning(true)
        .halign(Align::Center)
        .margin_top(48)
        .build();
    content.append(&init_spinner);

    load_updates(
        content.clone(),
        init_spinner,
        status_lbl.clone(),
        busy.clone(),
        update_all_btn.clone(),
        reboot_btn.clone(),
    );

    // ── Check / Refresh button ─────────────────────────────────────────────
    {
        let content_c = content.clone();
        let status_c  = status_lbl.clone();
        let busy_c    = busy.clone();
        let upd_all_c = update_all_btn.clone();
        let reboot_c  = reboot_btn.clone();

        check_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            while let Some(child) = content_c.first_child() {
                content_c.remove(&child);
            }
            upd_all_c.set_visible(false);
            upd_all_c.set_sensitive(false);
            reboot_c.set_visible(false);
            status_c.set_label("Checking for updates…");
            busy_c.set_spinning(true);
            busy_c.set_visible(true);

            let sp = Spinner::builder()
                .spinning(true)
                .halign(Align::Center)
                .margin_top(48)
                .build();
            content_c.append(&sp);

            load_updates(
                content_c.clone(),
                sp,
                status_c.clone(),
                busy_c.clone(),
                upd_all_c.clone(),
                reboot_c.clone(),
            );

            let btn_c = btn.clone();
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                btn_c.set_sensitive(true);
            });
        });
    }

    reboot_btn.connect_clicked(|_| {
        rakuos_updates::schedule_reboot();
    });

    root.upcast()
}

// ── Data loading ──────────────────────────────────────────────────────────────

fn load_updates(
    content:        GBox,
    spinner:        Spinner,
    status_lbl:     Label,
    busy:           Spinner,
    update_all_btn: Button,
    reboot_btn:     Button,
) {
    let (tx, rx) = mpsc::channel::<(UpdateInfo, Vec<FlatpakUpdate>)>();

    std::thread::spawn(move || {
        let rt       = tokio::runtime::Runtime::new().expect("tokio runtime");
        let system   = rt.block_on(rakuos_updates::check_for_update());
        let flatpaks = rakuos_flatpak::get_all_updates();

        // Keep daemon cache in sync
        let total = (if system.available { 1 } else { 0 }) + flatpaks.len();
        let cache = serde_json::json!({
            "total":           total,
            "packages":        [],
            "flatpak":         serde_json::to_value(&flatpaks).unwrap_or_default(),
            "appimages":       [],
            "image_available": system.available,
            "image_info":      serde_json::to_value(&system).unwrap_or_default(),
        });
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let cache_path = std::path::PathBuf::from(home)
            .join(".cache/rakuos/daemon-update-cache.json");
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&cache_path, serde_json::to_string_pretty(&cache).unwrap_or_default());

        let _ = tx.send((system, flatpaks));
    });

    glib::timeout_add_local(Duration::from_millis(80), move || {
        match rx.try_recv() {
            Ok((system, flatpaks)) => {
                spinner.set_spinning(false);
                spinner.set_visible(false);
                busy.set_spinning(false);
                busy.set_visible(false);

                let apps:     Vec<FlatpakUpdate> = flatpaks.iter().filter(|f| !f.runtime).cloned().collect();
                let runtimes: Vec<FlatpakUpdate> = flatpaks.iter().filter(|f|  f.runtime).cloned().collect();

                let has_image   = system.available;
                let has_apps    = !apps.is_empty();
                let has_runtimes = !runtimes.is_empty();

                if !has_image && !has_apps && !has_runtimes {
                    status_lbl.set_label("Your system is up to date");
                    let sp = libadwaita::StatusPage::builder()
                        .title("Up to Date")
                        .description("Your system and apps are up to date")
                        .icon_name("emblem-default-symbolic")
                        .build();
                    content.append(&sp);
                } else {
                    let count = (if has_image { 1 } else { 0 }) + flatpaks.len();
                    status_lbl.set_label(&format!(
                        "{} update{} available",
                        count,
                        if count == 1 { "" } else { "s" }
                    ));

                    update_all_btn.set_visible(true);
                    update_all_btn.set_sensitive(true);

                    let state = UpdateState::default();

                    // OS Image card
                    if has_image {
                        content.append(&build_image_card(
                            &system,
                            state.clone(),
                            reboot_btn.clone(),
                            status_lbl.clone(),
                            update_all_btn.clone(),
                        ));
                    }

                    // Applications section (non-runtime flatpaks)
                    if has_apps {
                        content.append(&build_flatpak_section("Applications", &apps, state.clone()));
                    }

                    // Runtimes / Add-ons section
                    if has_runtimes {
                        content.append(&build_flatpak_section("Runtimes / Add-ons", &runtimes, state.clone()));
                    }

                    // Wire Update All
                    let sys_repo = system.repo_url.clone();
                    let sys_tag  = system.new_tag.clone();
                    let do_image = system.available;
                    let all_fps  = flatpaks.clone();
                    let state_a  = state.clone();
                    let status_a = status_lbl.clone();
                    let upd_btn  = update_all_btn.clone();
                    let reboot_a = reboot_btn.clone();

                    update_all_btn.connect_clicked(move |btn| {
                        if state_a.is_running() { return; }
                        state_a.start("all");
                        btn.set_sensitive(false);
                        btn.set_label("Updating…");
                        status_a.set_label("Updating…");

                        let repo   = sys_repo.clone();
                        let tag    = sys_tag.clone();
                        let fps    = all_fps.clone();
                        let do_img = do_image;
                        let (tx2, rx2) = mpsc::channel::<bool>();

                        std::thread::spawn(move || {
                            if do_img {
                                let _: Vec<_> = rakuos_updates::upgrade_image_stream("switch", &repo, &tag).collect();
                            }
                            for fp in &fps {
                                let id = fp.app_id.clone();
                                let _: Vec<_> = rakuos_flatpak::update_single_stream(&id).collect();
                            }
                            let _ = tx2.send(do_img);
                        });

                        let state_r  = state_a.clone();
                        let btn_r    = upd_btn.clone();
                        let status_r = status_a.clone();
                        let reboot_r = reboot_a.clone();

                        glib::timeout_add_local(Duration::from_millis(100), move || {
                            match rx2.try_recv() {
                                Ok(did_image) => {
                                    state_r.stop();
                                    btn_r.set_sensitive(false);
                                    btn_r.set_label("Done");
                                    if did_image {
                                        status_r.set_label("System update staged — reboot to apply");
                                        reboot_r.set_visible(true);
                                    } else {
                                        status_r.set_label("Updates complete");
                                    }
                                    glib::ControlFlow::Break
                                }
                                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                                Err(_) => { state_r.stop(); glib::ControlFlow::Break }
                            }
                        });
                    });
                }
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });
}

// ── OS Image update card ───────────────────────────────────────────────────────

fn build_image_card(
    system:     &UpdateInfo,
    state:      UpdateState,
    reboot_btn: Button,
    status_lbl: Label,
    upd_all:    Button,
) -> Widget {
    let card = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .css_classes(vec!["card".to_string()])
        .build();

    // Header
    let header = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();
    header.append(
        &Label::builder()
            .label("Operating System")
            .halign(Align::Start)
            .hexpand(true)
            .css_classes(vec!["heading".to_string()])
            .build(),
    );
    header.append(
        &Label::builder()
            .label("1 update")
            .halign(Align::End)
            .css_classes(vec!["dim-label".to_string(), "caption".to_string()])
            .build(),
    );
    card.append(&header);
    card.append(&gtk4::Separator::new(Orientation::Horizontal));

    // Item row
    let item_row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .build();

    item_row.append(
        &gtk4::Image::builder()
            .icon_name("system-software-update-symbolic")
            .pixel_size(36)
            .build(),
    );

    let info = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .valign(Align::Center)
        .hexpand(true)
        .build();
    info.append(
        &Label::builder()
            .label("RakuOS")
            .halign(Align::Start)
            .css_classes(vec!["heading".to_string()])
            .build(),
    );
    let version_text = if !system.current_version.is_empty() && !system.new_version.is_empty() {
        format!("{} → {}", system.current_version, system.new_version)
    } else if !system.new_version.is_empty() {
        format!("→ {}", system.new_version)
    } else {
        "Update available".to_string()
    };
    info.append(
        &Label::builder()
            .label(&version_text)
            .halign(Align::Start)
            .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
            .build(),
    );
    item_row.append(&info);

    let update_btn = Button::builder()
        .label("Update")
        .valign(Align::Center)
        .build();
    item_row.append(&update_btn);
    card.append(&item_row);

    let progress = ProgressBar::builder()
        .margin_start(12)
        .margin_end(12)
        .margin_bottom(4)
        .visible(false)
        .build();
    card.append(&progress);

    let done_lbl = Label::builder()
        .halign(Align::Center)
        .margin_bottom(8)
        .visible(false)
        .css_classes(vec!["caption".to_string()])
        .build();
    card.append(&done_lbl);

    // Wire button
    let repo     = system.repo_url.clone();
    let tag      = system.new_tag.clone();
    let prog_c   = progress.clone();
    let done_c   = done_lbl.clone();
    let btn_ref  = update_btn.clone();
    let state_c  = state.clone();
    let reboot_c = reboot_btn.clone();
    let status_c = status_lbl.clone();
    let upd_c    = upd_all.clone();

    update_btn.connect_clicked(move |btn| {
        if state_c.is_running() { return; }
        state_c.start("image");
        btn.set_sensitive(false);
        btn.set_label("Updating…");
        prog_c.set_visible(true);

        let repo_t = repo.clone();
        let tag_t  = tag.clone();
        let (tx, rx) = mpsc::channel::<bool>();
        std::thread::spawn(move || {
            let mut ok = false;
            for line in rakuos_updates::upgrade_image_stream("switch", &repo_t, &tag_t) {
                if let Some(code) = line.strip_prefix("__done__") {
                    ok = code.trim() == "0";
                }
            }
            let _ = tx.send(ok);
        });

        let state_r  = state_c.clone();
        let prog_r   = prog_c.clone();
        let done_r   = done_c.clone();
        let btn_r    = btn_ref.clone();
        let reboot_r = reboot_c.clone();
        let status_r = status_c.clone();
        let upd_r    = upd_c.clone();

        glib::timeout_add_local(Duration::from_millis(100), move || {
            prog_r.pulse();
            match rx.try_recv() {
                Ok(ok) => {
                    state_r.stop();
                    prog_r.set_visible(false);
                    if ok {
                        done_r.set_label("System update staged — reboot to apply");
                        done_r.set_visible(true);
                        btn_r.set_label("Done");
                        reboot_r.set_visible(true);
                        upd_r.set_visible(false);
                        status_r.set_label("System update staged — reboot to apply");
                    } else {
                        done_r.set_label("Update failed");
                        done_r.set_visible(true);
                        btn_r.set_label("Retry");
                        btn_r.set_sensitive(true);
                    }
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => {
                    state_r.stop();
                    prog_r.set_visible(false);
                    done_r.set_label("Update failed");
                    done_r.set_visible(true);
                    btn_r.set_label("Retry");
                    btn_r.set_sensitive(true);
                    glib::ControlFlow::Break
                }
            }
        });
    });

    card.upcast()
}

// ── Flatpak section card (apps or runtimes) ───────────────────────────────────

fn build_flatpak_section(title: &str, flatpaks: &[FlatpakUpdate], state: UpdateState) -> Widget {
    let card = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .css_classes(vec!["card".to_string()])
        .build();

    // Section header
    let header = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    header.append(
        &Label::builder()
            .label(title)
            .halign(Align::Start)
            .hexpand(true)
            .css_classes(vec!["heading".to_string()])
            .build(),
    );
    header.append(
        &Label::builder()
            .label(&format!(
                "{} update{}",
                flatpaks.len(),
                if flatpaks.len() == 1 { "" } else { "s" }
            ))
            .halign(Align::End)
            .css_classes(vec!["dim-label".to_string(), "caption".to_string()])
            .build(),
    );

    let sec_upd_btn = Button::builder()
        .label("Update All")
        .valign(Align::Center)
        .build();
    header.append(&sec_upd_btn);
    card.append(&header);
    card.append(&gtk4::Separator::new(Orientation::Horizontal));

    // Items
    let count = flatpaks.len();
    for (i, fp) in flatpaks.iter().enumerate() {
        card.append(&build_flatpak_row(fp, state.clone()));
        if i + 1 < count {
            card.append(&gtk4::Separator::new(Orientation::Horizontal));
        }
    }

    // Wire section Update All
    let fps_all  = flatpaks.to_vec();
    let state_sa = state.clone();
    let btn_sa   = sec_upd_btn.clone();

    sec_upd_btn.connect_clicked(move |btn| {
        if state_sa.is_running() { return; }
        state_sa.start("flatpak-all");
        btn.set_sensitive(false);
        btn.set_label("Updating…");

        let fps = fps_all.clone();
        let (tx, rx) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            for fp in &fps {
                let id = fp.app_id.clone();
                let _: Vec<_> = rakuos_flatpak::update_single_stream(&id).collect();
            }
            let _ = tx.send(());
        });

        let state_r = state_sa.clone();
        let btn_r   = btn_sa.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            match rx.try_recv() {
                Ok(_) => {
                    state_r.stop();
                    btn_r.set_label("Done");
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => { state_r.stop(); glib::ControlFlow::Break }
            }
        });
    });

    card.upcast()
}

fn build_flatpak_row(fp: &FlatpakUpdate, state: UpdateState) -> Widget {
    let row = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let item = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();

    // Icon — look up by app_id in flatpak icon dirs
    let icon_path = format!(
        "/var/lib/flatpak/appstream/flathub/x86_64/active/icons/128x128/{}.png",
        fp.app_id
    );
    let icon = super::icon_helper::load_app_icon(&icon_path, "", 36, &fp.name);
    item.append(&icon);

    let info = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .valign(Align::Center)
        .hexpand(true)
        .build();
    info.append(
        &Label::builder()
            .label(&fp.name)
            .halign(Align::Start)
            .css_classes(vec!["heading".to_string()])
            .build(),
    );

    let badge_row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .build();

    if !fp.version.is_empty() {
        let ver_text = if !fp.current_version.is_empty() && fp.current_version != fp.version {
            format!("{} → {}", fp.current_version, fp.version)
        } else {
            fp.version.clone()
        };
        badge_row.append(
            &Label::builder()
                .label(&ver_text)
                .halign(Align::Start)
                .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
                .build(),
        );
    }

    let badge_text = if fp.runtime { "Runtime" } else { "Flatpak" };
    badge_row.append(
        &Label::builder()
            .label(badge_text)
            .halign(Align::Start)
            .css_classes(vec!["tag".to_string()])
            .build(),
    );

    info.append(&badge_row);
    item.append(&info);

    let update_btn = Button::builder()
        .label("Update")
        .valign(Align::Center)
        .build();
    item.append(&update_btn);
    row.append(&item);

    let progress = ProgressBar::builder()
        .visible(false)
        .build();
    row.append(&progress);

    // Wire individual Update button
    let app_id  = fp.app_id.clone();
    let prog_c  = progress.clone();
    let btn_ref = update_btn.clone();
    let state_c = state.clone();

    update_btn.connect_clicked(move |btn| {
        if state_c.is_running() { return; }
        state_c.start(&app_id);
        btn.set_sensitive(false);
        btn.set_label("Updating…");
        prog_c.set_visible(true);

        let id = app_id.clone();
        let (tx, rx) = mpsc::channel::<bool>();
        std::thread::spawn(move || {
            let mut ok = false;
            for line in rakuos_flatpak::update_single_stream(&id) {
                if let Some(code) = line.strip_prefix("__done__") {
                    ok = code.trim() == "0";
                }
            }
            let _ = tx.send(ok);
        });

        let state_r = state_c.clone();
        let prog_r  = prog_c.clone();
        let btn_r   = btn_ref.clone();

        glib::timeout_add_local(Duration::from_millis(100), move || {
            prog_r.pulse();
            match rx.try_recv() {
                Ok(ok) => {
                    state_r.stop();
                    prog_r.set_visible(false);
                    if ok {
                        btn_r.set_label("Done");
                    } else {
                        btn_r.set_label("Retry");
                        btn_r.set_sensitive(true);
                    }
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => {
                    state_r.stop();
                    prog_r.set_visible(false);
                    btn_r.set_label("Retry");
                    btn_r.set_sensitive(true);
                    glib::ControlFlow::Break
                }
            }
        });
    });

    row.upcast()
}
