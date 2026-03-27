// pages/updates.rs — System and Flatpak updates page

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, Label, Orientation, ScrolledWindow, Widget,
};
use libadwaita::prelude::*;
use libadwaita::NavigationView;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use rakuos_flatpak::FlatpakApp;
use rakuos_updates::UpdateInfo;

pub fn build(_nav: Arc<NavigationView>) -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let outer = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .build();
    scroll.set_child(Some(&outer));

    // Toolbar
    let toolbar = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(12)
        .margin_start(16)
        .margin_end(16)
        .margin_bottom(8)
        .build();

    let update_all_btn = Button::builder()
        .label("Update All")
        .css_classes(vec!["suggested-action".to_string(), "pill".to_string()])
        .sensitive(false)
        .build();

    let restart_btn = Button::builder()
        .label("Restart & Update")
        .css_classes(vec!["pill".to_string()])
        .sensitive(false)
        .visible(false)
        .build();

    let refresh_btn = Button::builder()
        .icon_name("view-refresh-symbolic")
        .css_classes(vec!["circular".to_string()])
        .build();

    let spacer = GBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    toolbar.append(&update_all_btn);
    toolbar.append(&restart_btn);
    toolbar.append(&spacer);
    toolbar.append(&refresh_btn);
    outer.append(&toolbar);

    // Content area
    let content = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(8)
        .margin_bottom(24)
        .margin_start(16)
        .margin_end(16)
        .build();
    outer.append(&content);

    let spinner = gtk4::Spinner::builder()
        .spinning(true)
        .halign(Align::Center)
        .margin_top(48)
        .build();
    content.append(&spinner);

    load_updates(
        content.clone(),
        spinner,
        update_all_btn.clone(),
        restart_btn.clone(),
    );

    // Refresh button
    let content_c = content.clone();
    let update_all_c = update_all_btn.clone();
    let restart_c = restart_btn.clone();
    refresh_btn.connect_clicked(move |_| {
        while let Some(child) = content_c.first_child() {
            content_c.remove(&child);
        }
        update_all_c.set_sensitive(false);
        restart_c.set_visible(false);
        let sp2 = gtk4::Spinner::builder()
            .spinning(true)
            .halign(Align::Center)
            .margin_top(48)
            .build();
        content_c.append(&sp2);
        load_updates(
            content_c.clone(),
            sp2,
            update_all_c.clone(),
            restart_c.clone(),
        );
    });

    scroll.upcast()
}

fn load_updates(
    content: GBox,
    spinner: gtk4::Spinner,
    update_all_btn: Button,
    restart_btn: Button,
) {
    type UpdateData = (UpdateInfo, Vec<FlatpakApp>);
    let (tx, rx) = mpsc::channel::<UpdateData>();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let system = rt.block_on(rakuos_updates::check_for_update());
        let flatpaks = rakuos_flatpak::get_updates();
        let _ = tx.send((system, flatpaks));
    });

    glib::timeout_add_local(Duration::from_millis(80), move || {
        match rx.try_recv() {
            Ok((system, flatpaks)) => {
                spinner.set_spinning(false);
                spinner.set_visible(false);

                let has_system = system.available;
                let has_flatpaks = !flatpaks.is_empty();
                let has_any = has_system || has_flatpaks;

                if !has_any {
                    let sp = libadwaita::StatusPage::builder()
                        .title("Up to Date")
                        .description("Your system and apps are up to date")
                        .icon_name("emblem-default-symbolic")
                        .build();
                    content.append(&sp);
                } else {
                    update_all_btn.set_sensitive(true);

                    if has_system {
                        restart_btn.set_visible(true);
                        restart_btn.set_sensitive(true);

                        let section_lbl = Label::builder()
                            .label("System Update (Requires Restart)")
                            .halign(Align::Start)
                            .css_classes(vec!["title-3".to_string()])
                            .build();
                        content.append(&section_lbl);

                        let row = build_system_update_row(&system.current_version, &system.new_version);
                        content.append(&row);

                        let note = Label::builder()
                            .label("A restart is required to apply the system update.")
                            .halign(Align::Start)
                            .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
                            .build();
                        content.append(&note);

                        if has_flatpaks {
                            content.append(&gtk4::Separator::new(Orientation::Horizontal));
                        }
                    }

                    if has_flatpaks {
                        let fp_lbl = Label::builder()
                            .label("Flatpak Updates")
                            .halign(Align::Start)
                            .css_classes(vec!["title-3".to_string()])
                            .build();
                        content.append(&fp_lbl);

                        let list = gtk4::ListBox::builder()
                            .selection_mode(gtk4::SelectionMode::None)
                            .css_classes(vec!["boxed-list".to_string()])
                            .build();
                        for fp in &flatpaks {
                            let row = build_flatpak_update_row(fp);
                            list.append(&row);
                        }
                        content.append(&list);
                    }

                    // Wire Update All
                    let repo = system.repo_url.clone();
                    let tag = system.new_tag.clone();
                    let do_sys = system.available;
                    update_all_btn.connect_clicked(move |btn| {
                        btn.set_sensitive(false);
                        btn.set_label("Updating…");
                        let repo_c = repo.clone();
                        let tag_c = tag.clone();
                        let (tx2, rx2) = mpsc::channel::<()>();
                        std::thread::spawn(move || {
                            if do_sys {
                                let _: Vec<_> = rakuos_updates::upgrade_image_stream("switch", &repo_c, &tag_c).collect();
                            }
                            let _: Vec<_> = rakuos_flatpak::update_stream().collect();
                            let _ = tx2.send(());
                        });
                        let btn_c = btn.clone();
                        glib::timeout_add_local(Duration::from_millis(50), move || {
                            match rx2.try_recv() {
                                Ok(_) => {
                                    btn_c.set_label("Update All");
                                    btn_c.set_sensitive(true);
                                    glib::ControlFlow::Break
                                }
                                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                                Err(_) => glib::ControlFlow::Break,
                            }
                        });
                    });

                    // Wire Restart & Update
                    let repo2 = system.repo_url.clone();
                    let tag2 = system.new_tag.clone();
                    restart_btn.connect_clicked(move |btn| {
                        btn.set_sensitive(false);
                        let r = repo2.clone();
                        let t = tag2.clone();
                        let (tx3, rx3) = mpsc::channel::<()>();
                        std::thread::spawn(move || {
                            let _: Vec<_> = rakuos_updates::upgrade_image_stream("switch", &r, &t).collect();
                            let _ = tx3.send(());
                        });
                        let btn_c = btn.clone();
                        glib::timeout_add_local(Duration::from_millis(50), move || {
                            match rx3.try_recv() {
                                Ok(_) => {
                                    let _ = rakuos_updates::schedule_reboot();
                                    glib::ControlFlow::Break
                                }
                                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                                Err(_) => glib::ControlFlow::Break,
                            }
                        });
                        drop(btn_c);
                    });
                }
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });
}

fn build_system_update_row(current: &str, new_ver: &str) -> Widget {
    let row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .css_classes(vec!["card".to_string()])
        .margin_top(4)
        .margin_bottom(4)
        .build();

    let icon = gtk4::Image::builder()
        .icon_name("system-software-update-symbolic")
        .pixel_size(48)
        .margin_start(12)
        .margin_top(12)
        .margin_bottom(12)
        .build();
    row.append(&icon);

    let text = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .valign(Align::Center)
        .hexpand(true)
        .build();

    let name_lbl = Label::builder()
        .label("System Image")
        .halign(Align::Start)
        .css_classes(vec!["heading".to_string()])
        .build();

    let version_text = if !current.is_empty() && !new_ver.is_empty() {
        format!("{} → {}", current, new_ver)
    } else if !new_ver.is_empty() {
        format!("Update available: {}", new_ver)
    } else {
        "Update available".to_string()
    };

    let ver_lbl = Label::builder()
        .label(&version_text)
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string()])
        .build();

    text.append(&name_lbl);
    text.append(&ver_lbl);
    row.append(&text);
    row.upcast()
}

fn build_flatpak_update_row(fp: &FlatpakApp) -> Widget {
    let row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let icon = super::icon_helper::load_app_icon("", &fp.icon_url, 40, &fp.name);
    row.append(&icon);

    let text = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .valign(Align::Center)
        .hexpand(true)
        .build();

    let name_lbl = Label::builder()
        .label(&fp.name)
        .halign(Align::Start)
        .css_classes(vec!["heading".to_string()])
        .build();

    let ver_lbl = Label::builder()
        .label(&fp.version)
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string()])
        .build();

    text.append(&name_lbl);
    text.append(&ver_lbl);
    row.append(&text);
    row.upcast()
}
