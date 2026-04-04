// pages/installed.rs — Installed apps page with tabs per source

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, Entry, Label, Orientation, ScrolledWindow, SelectionMode, Widget,
};
use std::cell::RefCell;
use std::rc::Rc;
use libadwaita::prelude::*;
use libadwaita::{HeaderBar, NavigationPage, NavigationView, ToolbarView, ViewStack, ViewSwitcherBar};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use rakuos_appimages::AppImage;
use rakuos_packages::{FlatpakRuntime, NativeApp};
use rakuos_webapps::WebApp;

use super::icon_helper::load_app_icon;

pub fn build(nav: Arc<NavigationView>) -> Widget {
    let outer = GBox::builder()
        .orientation(Orientation::Vertical)
        .build();

    let view_stack = ViewStack::new();
    view_stack.set_vexpand(true);

    // ── Build tab shells (spinner + list, no data yet) ────────────────────────
    let (rpm_widget, rpm_spinner, rpm_list) = make_tab_shell();
    let rpm_page = view_stack.add_titled_with_icon(
        &rpm_widget, Some("rpm"), "RPM", "package-x-generic-symbolic");

    let (fp_widget, fp_spinner, fp_list) = make_tab_shell();
    let fp_page = view_stack.add_titled_with_icon(
        &fp_widget, Some("flatpak"), "Flatpak", "package-x-generic-symbolic");

    let (ai_widget, ai_spinner, ai_list) = make_tab_shell();
    let ai_page = view_stack.add_titled_with_icon(
        &ai_widget, Some("appimages"), "AppImages", "application-x-executable-symbolic");

    let (wa_widget, wa_spinner, wa_list) = make_tab_shell();
    let wa_page = view_stack.add_titled_with_icon(
        &wa_widget, Some("webapps"), "Web Apps", "web-browser-symbolic");

    let switcher_bar = ViewSwitcherBar::builder()
        .stack(&view_stack)
        .reveal(true)
        .build();

    outer.append(&view_stack);
    outer.append(&switcher_bar);

    // ── Single combined load (mirrors Qt InstalledPage) ───────────────────────
    type InstalledData = (Vec<NativeApp>, Vec<NativeApp>, Vec<FlatpakRuntime>, Vec<AppImage>, Vec<WebApp>);
    let (tx, rx) = mpsc::channel::<InstalledData>();

    std::thread::spawn(move || {
        let native   = rakuos_packages::get_installed().unwrap_or_default();
        let flatpak  = rakuos_packages::get_installed_flatpaks_enriched().unwrap_or_default();
        let runtimes = rakuos_packages::get_installed_flatpak_runtimes();
        let images   = rakuos_appimages::get_installed();
        let webapps  = rakuos_webapps::get_installed();
        let _ = tx.send((native, flatpak, runtimes, images, webapps));
    });

    glib::timeout_add_local(Duration::from_millis(80), move || {
        match rx.try_recv() {
            Ok((native, flatpak, runtimes, appimages, webapps)) => {
                // Stop spinners
                for sp in [&rpm_spinner, &fp_spinner, &ai_spinner, &wa_spinner] {
                    sp.set_spinning(false);
                    sp.set_visible(false);
                }

                // Update tab titles with counts
                rpm_page.set_title(Some(&format!("RPM ({})", native.len())));
                fp_page.set_title(Some(&format!("Flatpak ({}) + {} runtimes", flatpak.len(), runtimes.len())));
                ai_page.set_title(Some(&format!("AppImages ({})", appimages.len())));
                wa_page.set_title(Some(&format!("Web Apps ({})", webapps.len())));

                // Populate RPM
                if native.is_empty() {
                    rpm_list.append(&build_empty_state("No Native Packages",
                        "No overlay packages installed", "package-x-generic-symbolic"));
                } else {
                    let mut sorted = native.clone();
                    sorted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    for app in &sorted {
                        rpm_list.append(&build_native_app_row(app, Arc::clone(&nav)));
                    }
                }

                // Populate Flatpak apps
                if flatpak.is_empty() && runtimes.is_empty() {
                    fp_list.append(&build_empty_state("No Flatpaks Installed",
                        "Install Flatpak apps from the Explore page", "package-x-generic-symbolic"));
                } else {
                    if !flatpak.is_empty() {
                        fp_list.append(&build_section_header("Applications"));
                        let mut sorted = flatpak.clone();
                        sorted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                        for app in &sorted {
                            fp_list.append(&build_native_app_row(app, Arc::clone(&nav)));
                        }
                    }
                    if !runtimes.is_empty() {
                        fp_list.append(&build_section_header("Runtimes / Add-ons"));
                        let mut sorted = runtimes.clone();
                        sorted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                        for rt in &sorted {
                            fp_list.append(&build_runtime_row(rt));
                        }
                    }
                }

                // Populate AppImages
                if appimages.is_empty() {
                    ai_list.append(&build_empty_state("No AppImages Installed",
                        "AppImages you install will appear here",
                        "application-x-executable-symbolic"));
                } else {
                    for app in &appimages {
                        ai_list.append(&build_appimage_row(app, Arc::clone(&nav)));
                    }
                }

                // Populate Web Apps
                if webapps.is_empty() {
                    wa_list.append(&build_empty_state("No Web Apps Installed",
                        "Install web apps from the Web Apps page", "web-browser-symbolic"));
                } else {
                    for app in &webapps {
                        wa_list.append(&build_webapp_row(app, Arc::clone(&nav)));
                    }
                }

                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });

    outer.upcast()
}

/// Returns (wrapper_widget, spinner, list_box)
fn make_tab_shell() -> (Widget, gtk4::Spinner, gtk4::ListBox) {
    let wrapper = GBox::builder()
        .orientation(Orientation::Vertical)
        .build();

    let spinner = gtk4::Spinner::builder()
        .spinning(true)
        .halign(Align::Center)
        .margin_top(48)
        .build();
    wrapper.append(&spinner);

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let list = gtk4::ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .margin_top(12).margin_bottom(12)
        .margin_start(16).margin_end(16)
        .build();
    scroll.set_child(Some(&list));
    wrapper.append(&scroll);

    (wrapper.upcast(), spinner, list)
}

fn build_section_header(title: &str) -> Widget {
    let lbl = Label::builder()
        .label(title)
        .halign(Align::Start)
        .margin_top(12)
        .margin_bottom(4)
        .margin_start(16)
        .css_classes(vec!["heading".to_string(), "dim-label".to_string()])
        .build();
    lbl.upcast()
}

fn build_runtime_row(rt: &FlatpakRuntime) -> Widget {
    let row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let icon = gtk4::Image::builder()
        .icon_name("application-x-addon-symbolic")
        .pixel_size(32)
        .build();
    row.append(&icon);

    let info = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .valign(Align::Center)
        .hexpand(true)
        .build();
    info.append(&Label::builder()
        .label(&rt.name)
        .halign(Align::Start)
        .build());
    let sub = if rt.version.is_empty() {
        format!("{} · {}", rt.app_id, rt.origin)
    } else {
        format!("{} · {} · {}", rt.app_id, rt.version, rt.origin)
    };
    info.append(&Label::builder()
        .label(&sub)
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .wrap(true)
        .build());
    row.append(&info);

    let list_row = gtk4::ListBoxRow::new();
    list_row.set_child(Some(&row));
    list_row.set_activatable(false);
    list_row.upcast()
}



fn build_native_app_row(app: &NativeApp, nav: Arc<NavigationView>) -> Widget {
    let row_outer = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let icon = load_app_icon(&app.icon_path, &app.icon_url, 48, &app.name);
    row_outer.append(&icon);

    let text = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .valign(Align::Center)
        .hexpand(true)
        .build();

    let name_lbl = Label::builder()
        .label(&app.name)
        .halign(Align::Start)
        .css_classes(vec!["heading".to_string()])
        .build();
    let version_lbl = Label::builder()
        .label(&app.version)
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .build();
    text.append(&name_lbl);
    text.append(&version_lbl);
    row_outer.append(&text);

    let uninstall_btn = Button::builder()
        .label("Uninstall")
        .valign(Align::Center)
        .css_classes(vec!["destructive-action".to_string()])
        .build();

    let pkg_name = app.package_name.clone();
    let app_name_str = app.name.clone();
    let source = app.source.clone();

    uninstall_btn.connect_clicked(move |btn| {
        let pkg_c = pkg_name.clone();
        let app_name_c = app_name_str.clone();
        let source_c = source.clone();
        let btn_c = btn.clone();

        if let Some(root) = btn.root() {
            if let Ok(win) = root.downcast::<gtk4::Window>() {
                let dialog = libadwaita::MessageDialog::builder()
                    .transient_for(&win)
                    .heading(&format!("Uninstall {}?", app_name_c))
                    .body("This app will be removed from your system.")
                    .build();
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("uninstall", "Uninstall");
                dialog.set_response_appearance(
                    "uninstall",
                    libadwaita::ResponseAppearance::Destructive,
                );
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");

                let btn_cc = btn_c.clone();
                dialog.connect_response(None, move |_d, response| {
                    if response == "uninstall" {
                        let pkg = pkg_c.clone();
                        let src = source_c.clone();
                        btn_cc.set_sensitive(false);
                        btn_cc.set_label("Removing…");
                        let (tx, rx) = mpsc::channel::<bool>();
                        std::thread::spawn(move || {
                            let ok = if src == "flatpak" {
                                rakuos_flatpak::uninstall_stream(&pkg)
                                    .any(|l| l.starts_with("__done__0"))
                            } else {
                                rakuos_packages::remove_stream(&pkg)
                                    .any(|l| l.starts_with("__done__0"))
                            };
                            let _ = tx.send(ok);
                        });
                        let btn_ccc = btn_cc.clone();
                        glib::timeout_add_local(Duration::from_millis(50), move || {
                            match rx.try_recv() {
                                Ok(ok) => {
                                    if ok {
                                        if let Some(p) = btn_ccc.parent() {
                                            p.set_visible(false);
                                        }
                                    } else {
                                        btn_ccc.set_sensitive(true);
                                        btn_ccc.set_label("Uninstall");
                                    }
                                    glib::ControlFlow::Break
                                }
                                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                                Err(_) => glib::ControlFlow::Break,
                            }
                        });
                    }
                });
                dialog.present();
            }
        }
    });

    row_outer.append(&uninstall_btn);

    let btn_wrap = Button::new();
    btn_wrap.set_child(Some(&row_outer));
    btn_wrap.add_css_class("flat");

    let app_id2 = app.id.clone();
    let app_name2 = app.name.clone();
    let app_summary2 = app.summary.clone();
    let icon_path2 = app.icon_path.clone();
    let icon_url2 = app.icon_url.clone();
    let source2 = app.source.clone();

    btn_wrap.connect_clicked(move |_| {
        super::detail::push_detail(
            &nav,
            &app_id2,
            &app_name2,
            &app_summary2,
            &icon_path2,
            &icon_url2,
            &source2,
        );
    });

    btn_wrap.upcast()
}

fn build_appimage_row(app: &AppImage, nav: Arc<NavigationView>) -> Widget {
    let row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let icon = load_app_icon(&app.icon_path, "", 48, &app.name);
    row.append(&icon);

    let text = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .valign(Align::Center)
        .hexpand(true)
        .build();

    let name_lbl = Label::builder()
        .label(&app.name)
        .halign(Align::Start)
        .css_classes(vec!["heading".to_string()])
        .build();
    let ver_lbl = Label::builder()
        .label(&app.version)
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .build();
    text.append(&name_lbl);
    text.append(&ver_lbl);
    row.append(&text);

    // Show update source badge if present
    if !app.update_source.is_empty() && app.update_source != "none" {
        let badge = Label::builder()
            .label(&app.update_source)
            .css_classes(vec!["caption".to_string()])
            .halign(Align::Center)
            .valign(Align::Center)
            .build();
        row.append(&badge);
    }

    let btn_wrap = Button::new();
    btn_wrap.set_child(Some(&row));
    btn_wrap.add_css_class("flat");

    let app_clone = app.clone();
    btn_wrap.connect_clicked(move |_| {
        let detail_w = build_appimage_detail(&app_clone);
        let nav_page = NavigationPage::builder()
            .title(&app_clone.name)
            .child(&detail_w)
            .build();
        nav.push(&nav_page);
    });

    btn_wrap.upcast()
}

fn build_appimage_detail(app: &AppImage) -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let main_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(24).margin_bottom(32)
        .margin_start(24).margin_end(24)
        .build();
    scroll.set_child(Some(&main_box));

    // Hero row
    let hero = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(20)
        .build();

    let icon = load_app_icon(&app.icon_path, "", 96, &app.name);
    hero.append(&icon);

    let meta = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .valign(Align::Center)
        .hexpand(true)
        .build();

    meta.append(&Label::builder()
        .label(&app.name)
        .halign(Align::Start)
        .css_classes(vec!["title-1".to_string()])
        .build());
    if !app.version.is_empty() {
        meta.append(&Label::builder()
            .label(&format!("v{}", app.version))
            .halign(Align::Start)
            .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
            .build());
    }
    if !app.summary.is_empty() {
        meta.append(&Label::builder()
            .label(&app.summary)
            .halign(Align::Start)
            .wrap(true)
            .build());
    }
    hero.append(&meta);

    // Uninstall button
    let uninstall_btn = Button::builder()
        .label("Remove")
        .valign(Align::Center)
        .css_classes(vec!["destructive-action".to_string(), "pill".to_string()])
        .build();
    let app_id_u = app.id.clone();
    uninstall_btn.connect_clicked(move |btn| {
        let pkg = app_id_u.clone();
        btn.set_sensitive(false);
        btn.set_label("Removing…");
        let (tx, rx) = mpsc::channel::<bool>();
        std::thread::spawn(move || {
            let (ok, _) = rakuos_appimages::uninstall(&pkg);
            let _ = tx.send(ok);
        });
        let btn_c = btn.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            match rx.try_recv() {
                Ok(ok) => {
                    if ok {
                        btn_c.set_label("Removed");
                    } else {
                        btn_c.set_sensitive(true);
                        btn_c.set_label("Remove");
                    }
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });
    hero.append(&uninstall_btn);
    main_box.append(&hero);
    main_box.append(&gtk4::Separator::new(Orientation::Horizontal));

    if !app.description.is_empty() {
        main_box.append(&Label::builder()
            .label(&app.description)
            .halign(Align::Start)
            .wrap(true)
            .selectable(true)
            .build());
        main_box.append(&gtk4::Separator::new(Orientation::Horizontal));
    }

    // Installed path / dates info
    let info_list = gtk4::ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .build();
    if !app.installed_path.is_empty() {
        info_list.append(&libadwaita::ActionRow::builder()
            .title("Installed Path").subtitle(&app.installed_path).build());
    }
    if !app.installed_at.is_empty() {
        info_list.append(&libadwaita::ActionRow::builder()
            .title("Installed").subtitle(&app.installed_at).build());
    }
    if !app.last_checked.is_empty() {
        info_list.append(&libadwaita::ActionRow::builder()
            .title("Last Checked").subtitle(&app.last_checked).build());
    }
    if info_list.first_child().is_some() {
        main_box.append(&info_list);
        main_box.append(&gtk4::Separator::new(Orientation::Horizontal));
    }

    // ── Update Settings (editable) ────────────────────────────────────────
    main_box.append(&Label::builder()
        .label("Update Settings")
        .halign(Align::Start)
        .css_classes(vec!["title-4".to_string()])
        .build());

    let settings_list = gtk4::ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .build();

    // Source dropdown row
    let source_items = gtk4::StringList::new(&["none", "github", "gitlab", "url"]);
    let source_combo = libadwaita::ComboRow::builder()
        .title("Source")
        .model(&source_items)
        .build();
    let src_idx = match app.update_source.as_str() {
        "github" => 1u32,
        "gitlab" => 2,
        "url"    => 3,
        _        => 0,
    };
    source_combo.set_selected(src_idx);
    settings_list.append(&source_combo);

    // URL field row — label changes with source
    let url_label = Rc::new(RefCell::new(Label::builder()
        .label(url_field_label(src_idx))
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string()])
        .width_request(80)
        .build()));

    let url_entry = Entry::builder()
        .text(&app.update_url)
        .placeholder_text(url_field_placeholder(src_idx))
        .hexpand(true)
        .build();

    let url_row_box = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(10).margin_bottom(10)
        .margin_start(12).margin_end(12)
        .build();
    url_row_box.append(&*url_label.borrow());
    url_row_box.append(&url_entry);

    let url_action_row = libadwaita::ActionRow::new();
    url_action_row.set_child(Some(&url_row_box));
    url_action_row.set_visible(src_idx > 0);
    settings_list.append(&url_action_row);

    // Pattern field row
    let pattern_entry = Entry::builder()
        .text(&app.update_pattern)
        .placeholder_text("e.g. *.AppImage")
        .hexpand(true)
        .build();

    let pattern_row_box = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(10).margin_bottom(10)
        .margin_start(12).margin_end(12)
        .build();
    pattern_row_box.append(&Label::builder()
        .label("Pattern")
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string()])
        .width_request(80)
        .build());
    pattern_row_box.append(&pattern_entry);

    let pattern_action_row = libadwaita::ActionRow::new();
    pattern_action_row.set_child(Some(&pattern_row_box));
    pattern_action_row.set_visible(src_idx > 0);
    settings_list.append(&pattern_action_row);

    // Save row
    let save_status = Label::builder()
        .label("")
        .halign(Align::Start)
        .visible(false)
        .build();
    let save_btn = Button::builder()
        .label("Save Settings")
        .valign(Align::Center)
        .css_classes(vec!["suggested-action".to_string()])
        .build();
    let save_row_box = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(10).margin_bottom(10)
        .margin_start(12).margin_end(12)
        .build();
    save_row_box.append(&save_btn);
    save_row_box.append(&save_status);
    let save_action_row = libadwaita::ActionRow::new();
    save_action_row.set_child(Some(&save_row_box));
    settings_list.append(&save_action_row);

    main_box.append(&settings_list);

    // Source combo → update URL/pattern row visibility and label
    let url_row_c = url_action_row.clone();
    let pat_row_c = pattern_action_row.clone();
    let url_lbl_rc = Rc::clone(&url_label);
    let url_entry_c = url_entry.clone();
    source_combo.connect_notify_local(Some("selected"), move |combo, _| {
        let idx = combo.selected();
        let visible = idx > 0;
        url_row_c.set_visible(visible);
        pat_row_c.set_visible(visible);
        url_lbl_rc.borrow().set_label(url_field_label(idx));
        url_entry_c.set_placeholder_text(Some(url_field_placeholder(idx)));
    });

    // Save button
    let app_id_s = app.id.clone();
    save_btn.connect_clicked(move |_| {
        let src_text = match source_combo.selected() {
            1 => "github",
            2 => "gitlab",
            3 => "url",
            _ => "none",
        };
        let url = url_entry.text().to_string();
        let pattern = pattern_entry.text().to_string();
        let id = app_id_s.clone();
        let (tx, rx) = mpsc::channel::<(bool, String)>();
        std::thread::spawn(move || {
            let res = rakuos_appimages::save_settings(&id, src_text, &url, &pattern);
            let _ = tx.send(res);
        });
        let status_c = save_status.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            match rx.try_recv() {
                Ok((ok, msg)) => {
                    status_c.set_label(if ok { "✓ Saved" } else { &msg });
                    status_c.remove_css_class(if ok { "error" } else { "success" });
                    status_c.add_css_class(if ok { "success" } else { "error" });
                    status_c.set_visible(true);
                    let sc = status_c.clone();
                    glib::timeout_add_local_once(Duration::from_secs(3), move || {
                        sc.set_visible(false);
                    });
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });

    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&HeaderBar::new());
    toolbar.set_content(Some(&scroll));
    toolbar.upcast()
}

fn url_field_label(source_idx: u32) -> &'static str {
    match source_idx {
        1 => "owner/repo",
        2 => "namespace/repo",
        _ => "URL",
    }
}

fn url_field_placeholder(source_idx: u32) -> &'static str {
    match source_idx {
        1 => "e.g. owner/myapp",
        2 => "e.g. group/myapp",
        _ => "https://example.com/releases/latest",
    }
}

fn build_webapp_row(app: &WebApp, nav: Arc<NavigationView>) -> Widget {
    let row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let icon = load_app_icon(&app.icon_path, &app.icon_url, 48, &app.name);
    row.append(&icon);

    let text = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .valign(Align::Center)
        .hexpand(true)
        .build();

    let name_lbl = Label::builder()
        .label(&app.name)
        .halign(Align::Start)
        .css_classes(vec!["heading".to_string()])
        .build();
    let display_url = if app.website.is_empty() { &app.url } else { &app.website };
    let url_lbl = Label::builder()
        .label(display_url)
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .build();
    text.append(&name_lbl);
    text.append(&url_lbl);
    row.append(&text);

    let btn_wrap = Button::new();
    btn_wrap.set_child(Some(&row));
    btn_wrap.add_css_class("flat");

    let app_clone = app.clone();
    btn_wrap.connect_clicked(move |_| {
        super::webapps::push_webapp_detail(&nav, &app_clone);
    });

    btn_wrap.upcast()
}

fn build_empty_state(title: &str, desc: &str, icon: &str) -> Widget {
    libadwaita::StatusPage::builder()
        .title(title)
        .description(desc)
        .icon_name(icon)
        .build()
        .upcast()
}
