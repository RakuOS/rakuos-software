// pages/system.rs — System info and management page (matches Qt SystemPage layout)

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, DropDown, Label, Orientation,
    ScrolledWindow, Separator, StringList, Widget,
};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, Dialog, HeaderBar, PreferencesGroup, ToolbarView};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use rakuos_updates::{OverlayStatus, SystemStatus};

const DE_LABELS: &[(&str, &str)] = &[
    ("KDE Plasma", "rakuos-kde"),
    ("GNOME",      "rakuos-gnome"),
    ("COSMIC",     "rakuos-cosmic"),
];

const BRANCH_LABELS: &[(&str, &str)] = &[
    ("Stable", "latest"),
    ("Staging", "staging"),
];

fn parse_image_ref(image: &str) -> (String, String, bool) {
    let (image_path, tag) = image.rsplit_once(':').unwrap_or((image, ""));
    let name = image_path.split('/').last().unwrap_or("").to_string();
    let is_nvidia = name.ends_with("-nvidia");
    (name, tag.to_string(), is_nvidia)
}

fn de_label_from_image(image: &str) -> String {
    let (name, _, is_nvidia) = parse_image_ref(image);
    let base = name.trim_end_matches("-nvidia");
    let label = DE_LABELS.iter()
        .find(|(_, id)| *id == base)
        .map(|(lbl, _)| *lbl)
        .unwrap_or(base);
    if is_nvidia { format!("{} (Nvidia)", label) } else { label.to_string() }
}

fn branch_label_from_tag(tag: &str) -> &'static str {
    if tag.starts_with("staging") {
        "Staging"
    } else {
        "Stable"
    }
}

pub fn build() -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let outer = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .build();
    scroll.set_child(Some(&outer));

    let title = Label::builder()
        .label("System")
        .halign(Align::Start)
        .css_classes(vec!["title-1".to_string()])
        .build();
    outer.append(&title);

    // ── Booted Image ──────────────────────────────────────────────────────────
    let image_group = PreferencesGroup::builder()
        .title("Booted Image")
        .build();

    let image_row     = ActionRow::builder().title("OS Image").subtitle("Loading…").build();
    let version_row   = ActionRow::builder().title("Version").subtitle("Loading…").build();
    let digest_row    = ActionRow::builder().title("Digest").subtitle("Loading…").build();
    let timestamp_row = ActionRow::builder().title("Timestamp").subtitle("Loading…").build();
    let nvidia_row    = ActionRow::builder().title("Nvidia").subtitle("No").build();
    image_group.add(&image_row);
    image_group.add(&version_row);
    image_group.add(&digest_row);
    image_group.add(&timestamp_row);
    image_group.add(&nvidia_row);

    // Action buttons row
    let img_btn_row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(8)
        .margin_bottom(4)
        .build();

    let rollback_btn = Button::builder()
        .label("Rollback")
        .css_classes(vec!["pill".to_string()])
        .build();
    let img_reboot_btn = Button::builder()
        .label("Reboot to Apply")
        .css_classes(vec!["suggested-action".to_string(), "pill".to_string()])
        .visible(false)
        .build();
    let img_status_lbl = Label::builder()
        .halign(Align::Start)
        .hexpand(true)
        .css_classes(vec!["caption".to_string()])
        .visible(false)
        .build();

    img_btn_row.append(&rollback_btn);
    img_btn_row.append(&img_reboot_btn);
    img_btn_row.append(&img_status_lbl);
    image_group.add(&img_btn_row);

    outer.append(&image_group);

    // ── Desktop Environment ───────────────────────────────────────────────────
    let de_group = PreferencesGroup::builder()
        .title("Desktop Environment")
        .build();

    let current_de_row = ActionRow::builder()
        .title("Current")
        .subtitle("Loading…")
        .build();
    let current_branch_row = ActionRow::builder()
        .title("Branch")
        .subtitle("Loading…")
        .build();
    de_group.add(&current_de_row);
    de_group.add(&current_branch_row);

    let de_switch_row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(4)
        .margin_bottom(4)
        .build();

    let de_lbl = Label::builder()
        .label("Switch to:")
        .halign(Align::Start)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let de_list = StringList::new(&DE_LABELS.iter().map(|(l, _)| *l).collect::<Vec<_>>());
    let de_combo = DropDown::builder()
        .model(&de_list)
        .hexpand(true)
        .build();
    let branch_list = StringList::new(&BRANCH_LABELS.iter().map(|(l, _)| *l).collect::<Vec<_>>());
    let branch_combo = DropDown::builder()
        .model(&branch_list)
        .hexpand(true)
        .build();
    let switch_btn = Button::builder()
        .label("Switch Image")
        .css_classes(vec!["pill".to_string()])
        .build();
    let switch_status_lbl = Label::builder()
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .visible(false)
        .build();

    de_switch_row.append(&de_lbl);
    de_switch_row.append(&de_combo);
    de_switch_row.append(&branch_combo);
    de_switch_row.append(&switch_btn);
    de_switch_row.append(&switch_status_lbl);
    de_group.add(&de_switch_row);
    outer.append(&de_group);

    // ── Overlay Packages ──────────────────────────────────────────────────────
    let overlay_group = PreferencesGroup::builder()
        .title("Overlay Packages")
        .description("Packages installed on top of the base OS image")
        .build();

    // Header row: count + reset button
    let overlay_hdr = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(4)
        .margin_bottom(4)
        .build();
    let overlay_count_lbl = Label::builder()
        .label("Loading…")
        .halign(Align::Start)
        .hexpand(true)
        .css_classes(vec!["dim-label".to_string(), "caption".to_string()])
        .build();
    let reset_btn = Button::builder()
        .label("Reset Overlay…")
        .css_classes(vec!["pill".to_string(), "destructive-action".to_string()])
        .halign(Align::End)
        .build();
    let overlay_status_lbl = Label::builder()
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string()])
        .visible(false)
        .build();
    let overlay_reboot_btn = Button::builder()
        .label("Reboot to Apply")
        .css_classes(vec!["suggested-action".to_string(), "pill".to_string()])
        .halign(Align::End)
        .visible(false)
        .build();

    overlay_hdr.append(&overlay_count_lbl);
    overlay_hdr.append(&overlay_status_lbl);
    overlay_hdr.append(&overlay_reboot_btn);
    overlay_hdr.append(&reset_btn);
    overlay_group.add(&overlay_hdr);
    overlay_group.add(&Separator::new(Orientation::Horizontal));

    // Package list box — populated after load
    let pkg_list_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .margin_top(4)
        .build();
    let no_pkgs_lbl = Label::builder()
        .label("No overlay packages installed.")
        .halign(Align::Start)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    pkg_list_box.append(&no_pkgs_lbl);
    overlay_group.add(&pkg_list_box);

    outer.append(&overlay_group);

    // ── Firmware ──────────────────────────────────────────────────────────────
    let fw_group = PreferencesGroup::builder()
        .title("Firmware Updates")
        .description("Check for firmware updates via LVFS")
        .build();

    let fw_row = ActionRow::builder()
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
            let _ = std::process::Command::new("fwupdmgr").arg("refresh").status();
            let _ = tx.send(());
        });
        let btn_c = btn.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            match rx.try_recv() {
                Ok(_) => { btn_c.set_label("Refresh"); btn_c.set_sensitive(true); glib::ControlFlow::Break }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });

    fw_row.add_suffix(&fw_btn);
    fw_group.add(&fw_row);
    outer.append(&fw_group);

    // ── Wire Rollback ─────────────────────────────────────────────────────────
    {
        let reboot_b  = img_reboot_btn.clone();
        let status_l  = img_status_lbl.clone();
        let outer_w   = outer.clone();

        rollback_btn.connect_clicked(move |btn| {
            // Show warning dialog before proceeding — rollback also triggers a
            // soft overlay reset so the user knows both operations will run.
            let dlg = Dialog::builder()
                .title("Roll Back System?")
                .build();

            let toolbar = ToolbarView::new();
            let header = HeaderBar::new();
            toolbar.add_top_bar(&header);

            let body = GBox::builder()
                .orientation(Orientation::Vertical)
                .spacing(12)
                .margin_top(12)
                .margin_bottom(20)
                .margin_start(24)
                .margin_end(24)
                .build();

            let msg = Label::builder()
                .label("This will roll back to the previous OS image and perform a soft overlay reset.\n\nYour installed packages list is preserved — the overlay will be rebuilt from it on next boot.\n\npkexec will prompt for your password twice.")
                .wrap(true)
                .xalign(0.0)
                .build();

            let btn_row = GBox::builder()
                .orientation(Orientation::Horizontal)
                .spacing(8)
                .halign(Align::End)
                .margin_top(8)
                .build();

            let cancel_btn = Button::builder()
                .label("Cancel")
                .build();
            let confirm_btn = Button::builder()
                .label("Roll Back & Reset Overlay")
                .css_classes(vec!["destructive-action".to_string()])
                .build();

            btn_row.append(&cancel_btn);
            btn_row.append(&confirm_btn);
            body.append(&msg);
            body.append(&btn_row);
            toolbar.set_content(Some(&body));
            dlg.set_child(Some(&toolbar));

            let dlg_c = dlg.clone();
            cancel_btn.connect_clicked(move |_| { let _ = dlg_c.close(); });

            let dlg_c2   = dlg.clone();
            let btn_c    = btn.clone();
            let reboot_c = reboot_b.clone();
            let status_c = status_l.clone();

            confirm_btn.connect_clicked(move |_| {
                let _ = dlg_c2.close();
                btn_c.set_sensitive(false);
                btn_c.set_label("Rolling back…");
                status_c.set_visible(false);

                let (tx, rx) = mpsc::channel::<()>();
                std::thread::spawn(move || {
                    let _: Vec<_> = rakuos_updates::rollback_stream().collect();
                    let _ = tx.send(());
                });
                let btn_c2    = btn_c.clone();
                let reboot_c2 = reboot_c.clone();
                let status_c2 = status_c.clone();

                glib::timeout_add_local(Duration::from_millis(50), move || {
                    match rx.try_recv() {
                        Ok(_) => {
                            btn_c2.set_label("Rollback");
                            btn_c2.set_sensitive(true);
                            reboot_c2.set_visible(true);
                            status_c2.set_label("Rollback staged — reboot to apply.");
                            status_c2.set_visible(true);
                            glib::ControlFlow::Break
                        }
                        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(_) => glib::ControlFlow::Break,
                    }
                });
            });

            dlg.present(Some(&outer_w));
        });
    }

    img_reboot_btn.connect_clicked(|_| { rakuos_updates::schedule_reboot(); });
    overlay_reboot_btn.connect_clicked(|_| { rakuos_updates::schedule_reboot(); });

    // ── Wire Switch DE ────────────────────────────────────────────────────────
    // current_image shared state populated after load
    let current_image_state: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    {
        let img_state = Rc::clone(&current_image_state);
        let combo_c   = de_combo.clone();
        let branch_c  = branch_combo.clone();
        let status_c  = switch_status_lbl.clone();
        let switch_c  = switch_btn.clone();
        let reboot_c  = img_reboot_btn.clone();

        switch_btn.connect_clicked(move |btn| {
            let img = img_state.borrow().clone();
            if img.is_empty() { return; }
            let idx = combo_c.selected() as usize;
            let branch_idx = branch_c.selected() as usize;
            let Some(&(_, de_id)) = DE_LABELS.get(idx) else { return; };
            let Some(&(branch_label, branch_tag)) = BRANCH_LABELS.get(branch_idx) else { return; };
            let (_, _, is_nvidia) = parse_image_ref(&img);
            let new_name = if is_nvidia { format!("{}-nvidia", de_id) } else { de_id.to_string() };
            // Derive base repo URL: strip everything from last '/' before ':'
            let base = img.split(':').next().unwrap_or("");
            let repo_base = base.rsplitn(2, '/').nth(1).unwrap_or(base);
            let target = format!("{}/{}:{}", repo_base, new_name, branch_tag);

            btn.set_sensitive(false);
            btn.set_label("Switching…");
            status_c.set_label(&format!("Switching to {} on {}…", de_id, branch_label));
            status_c.set_visible(true);

            let (tx, rx) = mpsc::channel::<()>();
            std::thread::spawn(move || {
                let _: Vec<_> = rakuos_updates::pkexec_switch_stream(&target).collect();
                let _ = tx.send(());
            });

            let btn_c    = switch_c.clone();
            let status_r = status_c.clone();
            let reboot_r = reboot_c.clone();

            glib::timeout_add_local(Duration::from_millis(50), move || {
                match rx.try_recv() {
                    Ok(_) => {
                        btn_c.set_label("Switch Image");
                        btn_c.set_sensitive(true);
                        reboot_r.set_visible(true);
                        status_r.set_label("Switch staged — reboot to apply.");
                        glib::ControlFlow::Break
                    }
                    Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                }
            });
        });
    }

    // ── Wire Reset Overlay ────────────────────────────────────────────────────
    {
        let reset_b      = reset_btn.clone();
        let ovl_status   = overlay_status_lbl.clone();
        let ovl_reboot   = overlay_reboot_btn.clone();

        reset_btn.connect_clicked(move |_| {
            show_reset_dialog(
                reset_b.upcast_ref::<gtk4::Widget>(),
                ovl_status.clone(),
                ovl_reboot.clone(),
            );
        });
    }

    // ── Load system status ────────────────────────────────────────────────────
    {
        let (tx, rx) = mpsc::channel::<(SystemStatus, OverlayStatus)>();
        std::thread::spawn(move || {
            let status  = rakuos_updates::get_system_status();
            let overlay = rakuos_updates::get_overlay_status();
            let _ = tx.send((status, overlay));
        });

        let img_state = Rc::clone(&current_image_state);

        glib::timeout_add_local(Duration::from_millis(80), move || {
            match rx.try_recv() {
                Ok((status, overlay)) => {
                    img_state.replace(status.image.clone());

                    image_row.set_subtitle(if status.image.is_empty() { "Unknown" } else { &status.image });
                    version_row.set_subtitle(if status.version.is_empty() { "Unknown" } else { &status.version });
                    digest_row.set_subtitle(if status.digest.is_empty() {
                        "Unknown".to_string()
                    } else {
                        format!("{}…", &status.digest[..status.digest.len().min(24)])
                    }.as_str());
                    timestamp_row.set_subtitle(if status.timestamp.is_empty() { "Unknown" } else { &status.timestamp });
                    let (_, tag, is_nvidia) = parse_image_ref(&status.image);
                    nvidia_row.set_subtitle(if is_nvidia { "Yes" } else { "No" });

                    // Set DE combo to current DE
                    let (name, _, _) = parse_image_ref(&status.image);
                    let base = name.trim_end_matches("-nvidia");
                    if let Some(idx) = DE_LABELS.iter().position(|(_, id)| *id == base) {
                        de_combo.set_selected(idx as u32);
                    }
                    if let Some(idx) = BRANCH_LABELS.iter().position(|(_, branch_tag)| {
                        if *branch_tag == "staging" { tag.starts_with("staging") } else { !tag.starts_with("staging") }
                    }) {
                        branch_combo.set_selected(idx as u32);
                    }
                    current_de_row.set_subtitle(&de_label_from_image(&status.image));
                    current_branch_row.set_subtitle(branch_label_from_tag(&tag));

                    // Overlay packages
                    overlay_count_lbl.set_label(&format!(
                        "{} package{}", overlay.package_count,
                        if overlay.package_count == 1 { "" } else { "s" }
                    ));

                    // Remove placeholder
                    if let Some(c) = pkg_list_box.first_child() {
                        pkg_list_box.remove(&c);
                    }
                    if overlay.packages.is_empty() {
                        pkg_list_box.append(&no_pkgs_lbl);
                    } else {
                        for pkg in &overlay.packages {
                            let row = ActionRow::builder()
                                .title(pkg.as_str())
                                .build();
                            pkg_list_box.append(&row);
                        }
                    }

                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    }

    scroll.upcast()
}

// ── Reset Overlay dialog ──────────────────────────────────────────────────────

fn show_reset_dialog(
    parent_widget: &gtk4::Widget,
    status_lbl:    Label,
    reboot_btn:    Button,
) {
    let dialog = Dialog::builder()
        .title("Reset Overlay")
        .build();

    let content = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(20)
        .margin_end(20)
        .build();

    let header = HeaderBar::new();
    let toolbar = ToolbarView::builder().content(&content).build();
    toolbar.add_top_bar(&header);
    dialog.set_child(Some(&toolbar));

    let intro = Label::builder()
        .label("Choose how to reset the overlay. Both options require a reboot.\npkexec will prompt for your password.")
        .halign(Align::Start)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    content.append(&intro);

    // Soft Reset card
    let soft_card = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .css_classes(vec!["card".to_string()])
        .margin_top(4)
        .build();
    let soft_inner = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(12).margin_bottom(12).margin_start(12).margin_end(12)
        .build();
    let soft_text = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .build();
    let soft_title = Label::builder()
        .label("Soft Reset (Rebuild)")
        .halign(Align::Start)
        .css_classes(vec!["heading".to_string()])
        .build();
    let soft_desc = Label::builder()
        .label("Wipes the overlay and reinstalls all packages\nfrom your packages list on next boot.\nYour installed package list is preserved.")
        .halign(Align::Start)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let soft_btn = Button::builder()
        .label("Soft Reset")
        .valign(Align::Center)
        .css_classes(vec!["pill".to_string(), "suggested-action".to_string()])
        .build();
    soft_text.append(&soft_title);
    soft_text.append(&soft_desc);
    soft_inner.append(&soft_text);
    soft_inner.append(&soft_btn);
    soft_card.append(&soft_inner);
    content.append(&soft_card);

    // Full Reset card
    let full_card = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .css_classes(vec!["card".to_string()])
        .build();
    let full_inner = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(12).margin_bottom(12).margin_start(12).margin_end(12)
        .build();
    let full_text = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .build();
    let full_title = Label::builder()
        .label("Full Reset (Stock)")
        .halign(Align::Start)
        .css_classes(vec!["heading".to_string()])
        .build();
    let full_desc = Label::builder()
        .label("Completely wipes the overlay and your packages list.\nThe system returns to the base image state.\nAll installed packages will be removed.")
        .halign(Align::Start)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let full_btn = Button::builder()
        .label("Full Reset")
        .valign(Align::Center)
        .css_classes(vec!["pill".to_string(), "destructive-action".to_string()])
        .build();
    full_text.append(&full_title);
    full_text.append(&full_desc);
    full_inner.append(&full_text);
    full_inner.append(&full_btn);
    full_card.append(&full_inner);
    content.append(&full_card);

    // Wire Soft Reset button
    {
        let d      = dialog.clone();
        let s_lbl  = status_lbl.clone();
        let r_btn  = reboot_btn.clone();

        soft_btn.connect_clicked(move |_| {
            let _ = d.close();
            run_overlay_reset("soft", s_lbl.clone(), r_btn.clone());
        });
    }

    // Wire Full Reset button
    {
        let d     = dialog.clone();
        let s_lbl = status_lbl.clone();
        let r_btn = reboot_btn.clone();

        full_btn.connect_clicked(move |_| {
            let _ = d.close();
            run_overlay_reset("full", s_lbl.clone(), r_btn.clone());
        });
    }

    let parent_window = parent_widget
        .root()
        .and_then(|r| r.downcast::<gtk4::Window>().ok());
    dialog.present(parent_window.as_ref());
}

fn run_overlay_reset(mode: &'static str, status_lbl: Label, reboot_btn: Button) {
    status_lbl.set_label("Running reset…");
    status_lbl.remove_css_class("success");
    status_lbl.remove_css_class("error");
    status_lbl.set_visible(true);
    reboot_btn.set_visible(false);

    let (tx, rx) = mpsc::channel::<bool>();
    std::thread::spawn(move || {
        let success = rakuos_updates::reset_overlay_stream(mode)
            .last()
            .map(|last| !last.starts_with("__done__2"))
            .unwrap_or(true);
        let _ = tx.send(success);
    });

    glib::timeout_add_local(Duration::from_millis(80), move || {
        match rx.try_recv() {
            Ok(ok) => {
                if ok {
                    status_lbl.set_label("Reset scheduled — reboot to apply.");
                    status_lbl.add_css_class("success");
                    reboot_btn.set_visible(true);
                } else {
                    status_lbl.set_label("Reset failed. Check system logs.");
                    status_lbl.add_css_class("error");
                }
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });
}

fn parse_bootc_progress(line: &str) -> Option<f64> {
    // "[N/M] ..." style from bootc/skopeo output
    let trimmed = line.trim();
    let bracket_start = trimmed.find('[')?;
    let bracket_end   = trimmed[bracket_start..].find(']')?;
    let inner = &trimmed[bracket_start + 1..bracket_start + bracket_end];
    let slash = inner.find('/')?;
    let n: f64 = inner[..slash].trim().parse().ok()?;
    let total: f64 = inner[slash + 1..].trim().parse().ok()?;
    if total > 0.0 { Some((n / total).min(1.0)) } else { None }
}
