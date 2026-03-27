// pages/local_install.rs — Install a local .rpm / .flatpak / .flatpakref / .AppImage

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, Label, Orientation, ProgressBar,
    ScrolledWindow, Spinner, Widget,
};
use libadwaita::prelude::*;
use libadwaita::{HeaderBar, NavigationView, StatusPage, ToolbarView};
use std::sync::{mpsc, Arc};
use std::time::Duration;

/// Determine the file kind from its extension.
pub fn file_kind(path: &str) -> Option<&'static str> {
    let lower = path.to_lowercase();
    if lower.ends_with(".rpm")        { return Some("rpm"); }
    if lower.ends_with(".flatpak")    { return Some("flatpak"); }
    if lower.ends_with(".flatpakref") { return Some("flatpakref"); }
    if lower.ends_with(".appimage")   { return Some("appimage"); }
    None
}

/// Push a local-file install page onto the navigation view.
pub fn push_local_install(nav: &Arc<NavigationView>, path: &str) {
    let Some(kind) = file_kind(path) else { return; };
    let page_w = build(path, kind);
    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&HeaderBar::new());
    toolbar.set_content(Some(&page_w));
    let nav_page = libadwaita::NavigationPage::builder()
        .title("Install Package")
        .child(&toolbar)
        .build();
    nav.push(&nav_page);
}

fn build(path: &str, kind: &'static str) -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let main_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(24)
        .margin_bottom(32)
        .margin_start(24)
        .margin_end(24)
        .build();
    scroll.set_child(Some(&main_box));

    // ── Spinner while loading metadata ──────────────────────────────────────
    let spinner = Spinner::builder()
        .spinning(true)
        .halign(Align::Center)
        .build();
    main_box.append(&spinner);

    let path_owned = path.to_string();
    let main_box_c = main_box.clone();

    // Load metadata in background thread
    let (tx, rx) = mpsc::channel::<serde_json::Value>();
    let path_thread = path_owned.clone();
    std::thread::spawn(move || {
        let info = match kind {
            "rpm"        => rakuos_packages::get_local_rpm_info(&path_thread),
            "flatpak"    => rakuos_flatpak::get_local_flatpak_info(&path_thread),
            "flatpakref" => rakuos_flatpak::get_flatpakref_info(&path_thread),
            "appimage"   => rakuos_appimages::get_appimage_info_for_display(&path_thread),
            _            => serde_json::json!({}),
        };
        let _ = tx.send(info);
    });

    glib::timeout_add_local(Duration::from_millis(80), move || {
        match rx.try_recv() {
            Ok(info) => {
                spinner.set_spinning(false);
                spinner.set_visible(false);
                build_content(&main_box_c, &path_owned, kind, &info);
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => {
                spinner.set_spinning(false);
                spinner.set_visible(false);
                let sp = StatusPage::builder()
                    .title("Could not read file")
                    .icon_name("dialog-error-symbolic")
                    .build();
                main_box_c.append(&sp);
                glib::ControlFlow::Break
            }
        }
    });

    scroll.upcast()
}

fn build_content(
    main_box: &GBox,
    path: &str,
    kind: &'static str,
    info: &serde_json::Value,
) {
    let name           = info["name"].as_str().unwrap_or(path);
    let version        = info["version"].as_str().unwrap_or("");
    let summary        = info["summary"].as_str().unwrap_or("");
    let desc           = info["description"].as_str().unwrap_or("");
    let already        = info["installed"].as_bool().unwrap_or(false);
    let install_action = info["install_action"].as_str().unwrap_or(if already { "reinstall" } else { "install" }).to_string();

    // ── Hero row ────────────────────────────────────────────────────────────
    let hero = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(20)
        .build();

    // Icon placeholder using a symbolic icon
    let icon_name = match kind {
        "rpm"        | "flatpak" | "flatpakref" => "package-x-generic-symbolic",
        "appimage"   => "application-x-executable-symbolic",
        _            => "document-symbolic",
    };
    let icon = gtk4::Image::builder()
        .icon_name(icon_name)
        .pixel_size(80)
        .build();
    icon.add_css_class("dim-label");
    hero.append(&icon);

    let meta = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .valign(Align::Center)
        .hexpand(true)
        .build();

    let name_lbl = Label::builder()
        .label(name)
        .halign(Align::Start)
        .wrap(true)
        .css_classes(vec!["title-1".to_string()])
        .build();
    meta.append(&name_lbl);

    if !version.is_empty() {
        let ver_lbl = Label::builder()
            .label(version)
            .halign(Align::Start)
            .css_classes(vec!["dim-label".to_string(), "caption".to_string()])
            .build();
        meta.append(&ver_lbl);
    }

    // Kind badge
    let kind_label = match kind {
        "rpm"        => "RPM Package",
        "flatpak"    => "Flatpak Bundle",
        "flatpakref" => "Flatpak Ref",
        "appimage"   => "AppImage",
        _            => "Package",
    };
    let badge = Label::builder()
        .label(kind_label)
        .halign(Align::Start)
        .css_classes(vec!["tag".to_string()])
        .build();
    meta.append(&badge);

    if !summary.is_empty() {
        let sum_lbl = Label::builder()
            .label(summary)
            .halign(Align::Start)
            .wrap(true)
            .build();
        meta.append(&sum_lbl);
    }

    hero.append(&meta);

    // Install button — label/sensitivity depends on install_action
    let btn_label = match install_action.as_str() {
        "reinstall" => "Reinstall",
        "upgrade"   => "Upgrade",
        _           => "Install",
    };
    let install_btn = Button::builder()
        .label(btn_label)
        .valign(Align::Center)
        .sensitive(true)
        .css_classes(vec!["suggested-action".to_string(), "pill".to_string()])
        .build();
    hero.append(&install_btn);

    main_box.append(&hero);

    // File path
    let path_lbl = Label::builder()
        .label(path)
        .halign(Align::Start)
        .wrap(true)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .build();
    main_box.append(&path_lbl);

    main_box.append(&gtk4::Separator::new(Orientation::Horizontal));

    // Description
    if !desc.is_empty() {
        let desc_lbl = Label::builder()
            .label(desc)
            .halign(Align::Start)
            .wrap(true)
            .selectable(true)
            .build();
        main_box.append(&desc_lbl);
    }

    // ── Status / progress area ──────────────────────────────────────────────
    let status_lbl = Label::builder()
        .halign(Align::Start)
        .wrap(true)
        .visible(false)
        .build();
    main_box.append(&status_lbl);

    let progress = ProgressBar::builder()
        .visible(false)
        .build();
    progress.set_pulse_step(0.1);
    main_box.append(&progress);

    // Log area (revealed after install attempt)
    let log_frame = gtk4::Frame::new(Some("Install Log"));
    log_frame.set_visible(false);
    let log_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .height_request(180)
        .build();
    let log_view = gtk4::TextView::builder()
        .editable(false)
        .monospace(true)
        .cursor_visible(false)
        .wrap_mode(gtk4::WrapMode::Word)
        .build();
    log_scroll.set_child(Some(&log_view));
    log_frame.set_child(Some(&log_scroll));
    main_box.append(&log_frame);

    // ── Install button handler ──────────────────────────────────────────────
    let path_owned      = path.to_string();
    let name_owned      = name.to_string();
    let action_owned    = install_action.clone();
    let status_c        = status_lbl.clone();
    let progress_c      = progress.clone();
    let log_frame_c     = log_frame.clone();
    let log_view_c      = log_view.clone();
    let install_c       = install_btn.clone();

    install_btn.connect_clicked(move |btn| {
        btn.set_label("Installing…");
        btn.set_sensitive(false);
        status_c.set_visible(false);
        progress_c.set_visible(true);
        log_frame_c.set_visible(false);
        log_view_c.buffer().set_text("");

        let (tx, rx) = mpsc::channel::<(bool, String)>();
        let path_t   = path_owned.clone();
        let name_t   = name_owned.clone();
        let action_t = action_owned.clone();

        std::thread::spawn(move || {
            let (ok, log) = match kind {
                "rpm" => {
                    let mut log = String::new();
                    let mut ok  = false;
                    let iter: Box<dyn Iterator<Item = String> + Send> = if action_t == "reinstall" {
                        Box::new(rakuos_packages::reinstall_local_rpm_stream(&name_t, &path_t))
                    } else {
                        Box::new(rakuos_packages::install_local_rpm_stream(&path_t))
                    };
                    for line in iter {
                        if let Some(code) = line.strip_prefix("__done__") {
                            ok = code.trim() == "0";
                        } else if !line.is_empty() {
                            log.push_str(&line);
                            log.push('\n');
                        }
                    }
                    (ok, log)
                }
                "flatpak" => {
                    let mut log = String::new();
                    let mut ok  = false;
                    for line in rakuos_flatpak::install_local_bundle_stream(&path_t) {
                        if let Some(code) = line.strip_prefix("__done__") {
                            ok = code.trim() == "0";
                        } else if !line.is_empty() {
                            log.push_str(&line);
                            log.push('\n');
                        }
                    }
                    (ok, log)
                }
                "flatpakref" => {
                    let mut log = String::new();
                    let mut ok  = false;
                    for line in rakuos_flatpak::install_flatpakref_stream(&path_t) {
                        if let Some(code) = line.strip_prefix("__done__") {
                            ok = code.trim() == "0";
                        } else if !line.is_empty() {
                            log.push_str(&line);
                            log.push('\n');
                        }
                    }
                    (ok, log)
                }
                "appimage" => {
                    let (ok, msg, _) = rakuos_appimages::install_appimage(&path_t);
                    (ok, msg)
                }
                _ => (false, "Unknown file type".to_string()),
            };
            let _ = tx.send((ok, log));
        });

        let status_rc    = status_c.clone();
        let progress_rc  = progress_c.clone();
        let log_frame_rc = log_frame_c.clone();
        let log_view_rc  = log_view_c.clone();
        let install_rc   = install_c.clone();

        glib::timeout_add_local(Duration::from_millis(100), move || {
            match rx.try_recv() {
                Ok((ok, log)) => {
                    progress_rc.set_visible(false);

                    status_rc.set_visible(true);
                    if ok {
                        status_rc.set_label("Done!");
                        status_rc.add_css_class("success");
                        status_rc.remove_css_class("error");
                        install_rc.set_label("Done");
                    } else {
                        status_rc.set_label("Installation failed.");
                        status_rc.add_css_class("error");
                        status_rc.remove_css_class("success");
                        install_rc.set_label("Retry");
                        install_rc.set_sensitive(true);
                    }

                    if !log.is_empty() {
                        log_view_rc.buffer().set_text(&log);
                        log_frame_rc.set_visible(true);
                    }
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => {
                    progress_rc.pulse();
                    glib::ControlFlow::Continue
                }
                Err(_) => {
                    progress_rc.set_visible(false);
                    status_rc.set_label("Error starting install process.");
                    status_rc.set_visible(true);
                    glib::ControlFlow::Break
                }
            }
        });
    });
}
