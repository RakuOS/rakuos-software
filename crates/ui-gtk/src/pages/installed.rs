// pages/installed.rs — Installed apps page with tabs per source

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, Label, Orientation, ScrolledWindow, SelectionMode, Widget,
};
use libadwaita::prelude::*;
use libadwaita::{NavigationView, ViewStack, ViewSwitcherBar};
use std::sync::Arc;

use rakuos_appimages::AppImage;
use rakuos_packages::NativeApp;
use rakuos_webapps::WebApp;

use super::icon_helper::load_app_icon;

pub fn build(nav: Arc<NavigationView>) -> Widget {
    let outer = GBox::builder()
        .orientation(Orientation::Vertical)
        .build();

    let view_stack = ViewStack::new();
    view_stack.set_vexpand(true);

    let rpm_box = build_rpm_tab(Arc::clone(&nav));
    view_stack
        .add_titled_with_icon(&rpm_box, Some("rpm"), "RPM", "package-x-generic-symbolic")
        .set_needs_attention(false);

    let fp_box = build_flatpak_tab(Arc::clone(&nav));
    view_stack
        .add_titled_with_icon(&fp_box, Some("flatpak"), "Flatpak", "package-x-generic-symbolic")
        .set_needs_attention(false);

    let ai_box = build_appimages_tab();
    view_stack
        .add_titled_with_icon(
            &ai_box,
            Some("appimages"),
            "AppImages",
            "application-x-executable-symbolic",
        )
        .set_needs_attention(false);

    let wa_box = build_webapps_tab();
    view_stack
        .add_titled_with_icon(&wa_box, Some("webapps"), "Web Apps", "web-browser-symbolic")
        .set_needs_attention(false);

    let switcher_bar = ViewSwitcherBar::builder()
        .stack(&view_stack)
        .reveal(true)
        .build();

    outer.append(&view_stack);
    outer.append(&switcher_bar);
    outer.upcast()
}

fn build_rpm_tab(nav: Arc<NavigationView>) -> Widget {
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
        .visible(false)
        .build();

    let list = gtk4::ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .build();
    scroll.set_child(Some(&list));
    wrapper.append(&scroll);

    std::thread::spawn(move || {
        let apps = rakuos_packages::get_installed().unwrap_or_default();
        glib::idle_add_once(move || {
            spinner.set_spinning(false);
            spinner.set_visible(false);
            scroll.set_visible(true);
            if apps.is_empty() {
                list.append(&build_empty_state(
                    "No Native Packages",
                    "No overlay packages are installed",
                    "package-x-generic-symbolic",
                ));
            } else {
                for app in &apps {
                    let row = build_native_app_row(app, Arc::clone(&nav));
                    list.append(&row);
                }
            }
        });
    });

    wrapper.upcast()
}

fn build_flatpak_tab(nav: Arc<NavigationView>) -> Widget {
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
        .visible(false)
        .build();

    let list = gtk4::ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .build();
    scroll.set_child(Some(&list));
    wrapper.append(&scroll);

    std::thread::spawn(move || {
        let apps = rakuos_packages::get_installed_flatpaks_enriched().unwrap_or_default();
        glib::idle_add_once(move || {
            spinner.set_spinning(false);
            spinner.set_visible(false);
            scroll.set_visible(true);
            if apps.is_empty() {
                list.append(&build_empty_state(
                    "No Flatpaks Installed",
                    "Install Flatpak apps from the Explore page",
                    "package-x-generic-symbolic",
                ));
            } else {
                for app in &apps {
                    let row = build_native_app_row(app, Arc::clone(&nav));
                    list.append(&row);
                }
            }
        });
    });

    wrapper.upcast()
}

fn build_appimages_tab() -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let list = gtk4::ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .build();
    scroll.set_child(Some(&list));

    std::thread::spawn(move || {
        let apps = rakuos_appimages::get_installed();
        glib::idle_add_once(move || {
            if apps.is_empty() {
                list.append(&build_empty_state(
                    "No AppImages Installed",
                    "AppImages you install will appear here",
                    "application-x-executable-symbolic",
                ));
            } else {
                for app in &apps {
                    let row = build_appimage_row(app);
                    list.append(&row);
                }
            }
        });
    });

    scroll.upcast()
}

fn build_webapps_tab() -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let list = gtk4::ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .build();
    scroll.set_child(Some(&list));

    std::thread::spawn(move || {
        let apps = rakuos_webapps::get_installed();
        glib::idle_add_once(move || {
            if apps.is_empty() {
                list.append(&build_empty_state(
                    "No Web Apps Installed",
                    "Install web apps from the Web Apps page",
                    "web-browser-symbolic",
                ));
            } else {
                for app in &apps {
                    let row = build_webapp_row(app);
                    list.append(&row);
                }
            }
        });
    });

    scroll.upcast()
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
                        let btn_ccc = btn_cc.clone();
                        std::thread::spawn(move || {
                            let ok = if src == "flatpak" {
                                rakuos_flatpak::uninstall_stream(&pkg)
                                    .any(|l| l.starts_with("__done__0"))
                            } else {
                                rakuos_packages::remove_stream(&pkg)
                                    .any(|l| l.starts_with("__done__0"))
                            };
                            glib::idle_add_once(move || {
                                if ok {
                                    if let Some(p) = btn_ccc.parent() {
                                        p.set_visible(false);
                                    }
                                } else {
                                    btn_ccc.set_sensitive(true);
                                    btn_ccc.set_label("Uninstall");
                                }
                            });
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

fn build_appimage_row(app: &AppImage) -> Widget {
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

    let uninstall_btn = Button::builder()
        .label("Remove")
        .valign(Align::Center)
        .css_classes(vec!["destructive-action".to_string()])
        .build();

    let app_id = app.id.clone();
    uninstall_btn.connect_clicked(move |btn| {
        let pkg = app_id.clone();
        btn.set_sensitive(false);
        let btn_c = btn.clone();
        std::thread::spawn(move || {
            let (ok, _msg) = rakuos_appimages::uninstall(&pkg);
            glib::idle_add_once(move || {
                if ok {
                    if let Some(p) = btn_c.parent() {
                        p.set_visible(false);
                    }
                } else {
                    btn_c.set_sensitive(true);
                }
            });
        });
    });

    row.append(&uninstall_btn);
    row.upcast()
}

fn build_webapp_row(app: &WebApp) -> Widget {
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
    let url_lbl = Label::builder()
        .label(&app.url)
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .build();
    text.append(&name_lbl);
    text.append(&url_lbl);
    row.append(&text);

    let uninstall_btn = Button::builder()
        .label("Uninstall")
        .valign(Align::Center)
        .css_classes(vec!["destructive-action".to_string()])
        .build();

    let app_id = app.id.clone();
    uninstall_btn.connect_clicked(move |btn| {
        let pkg = app_id.clone();
        btn.set_sensitive(false);
        let btn_c = btn.clone();
        std::thread::spawn(move || {
            let (ok, _) = rakuos_webapps::uninstall(&pkg);
            glib::idle_add_once(move || {
                if ok {
                    if let Some(p) = btn_c.parent() {
                        p.set_visible(false);
                    }
                } else {
                    btn_c.set_sensitive(true);
                }
            });
        });
    });

    row.append(&uninstall_btn);
    row.upcast()
}

fn build_empty_state(title: &str, desc: &str, icon: &str) -> Widget {
    libadwaita::StatusPage::builder()
        .title(title)
        .description(desc)
        .icon_name(icon)
        .build()
        .upcast()
}
