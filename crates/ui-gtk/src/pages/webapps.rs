// pages/webapps.rs — Web Apps page

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, FlowBox, FlowBoxChild, Label, Orientation, ScrolledWindow,
    SelectionMode, Widget,
};
use libadwaita::prelude::*;
use libadwaita::{NavigationPage, NavigationView};
use std::sync::Arc;

use rakuos_webapps::WebApp;

use super::icon_helper::load_app_icon;

pub fn build(nav: Arc<NavigationView>) -> Widget {
    let outer_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let main_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(16)
        .margin_bottom(24)
        .margin_start(16)
        .margin_end(16)
        .build();
    outer_scroll.set_child(Some(&main_box));

    let header_lbl = Label::builder()
        .label("Web Apps")
        .halign(Align::Start)
        .css_classes(vec!["title-1".to_string()])
        .build();
    main_box.append(&header_lbl);

    let subtitle_lbl = Label::builder()
        .label("Install web apps as native desktop applications")
        .halign(Align::Start)
        .css_classes(vec!["body".to_string(), "dim-label".to_string()])
        .build();
    main_box.append(&subtitle_lbl);

    let flow = FlowBox::builder()
        .selection_mode(SelectionMode::None)
        .min_children_per_line(2)
        .max_children_per_line(5)
        .column_spacing(12)
        .row_spacing(12)
        .homogeneous(true)
        .build();
    main_box.append(&flow);

    let spinner = gtk4::Spinner::builder()
        .spinning(true)
        .halign(Align::Center)
        .build();
    main_box.append(&spinner);

    let nav_c = Arc::clone(&nav);
    std::thread::spawn(move || {
        let apps = rakuos_webapps::get_catalog();
        glib::idle_add_once(move || {
            spinner.set_spinning(false);
            spinner.set_visible(false);

            if apps.is_empty() {
                let sp = libadwaita::StatusPage::builder()
                    .title("No Web Apps Available")
                    .description("No web app catalog found on this system")
                    .icon_name("web-browser-symbolic")
                    .build();
                main_box.append(&sp);
            } else {
                for app in &apps {
                    let card = build_webapp_card(app, Arc::clone(&nav_c));
                    let child = FlowBoxChild::new();
                    child.set_child(Some(&card));
                    child.set_focusable(false);
                    flow.insert(&child, -1);
                }
            }
        });
    });

    outer_scroll.upcast()
}

fn build_webapp_card(app: &WebApp, nav: Arc<NavigationView>) -> Widget {
    let card = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .width_request(160)
        .css_classes(vec!["card".to_string()])
        .margin_top(4)
        .margin_bottom(4)
        .build();

    let icon = load_app_icon(&app.icon_path, &app.icon_url, 64, &app.name);
    icon.set_halign(Align::Center);
    icon.set_margin_top(12);
    card.append(&icon);

    let name_lbl = Label::builder()
        .label(&app.name)
        .halign(Align::Center)
        .wrap(false)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(18)
        .css_classes(vec!["caption-heading".to_string()])
        .margin_start(8)
        .margin_end(8)
        .build();
    card.append(&name_lbl);

    let summary_lbl = Label::builder()
        .label(&app.summary)
        .halign(Align::Center)
        .wrap(false)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(20)
        .css_classes(vec!["caption".to_string()])
        .margin_start(8)
        .margin_end(8)
        .build();
    card.append(&summary_lbl);

    let spacer = GBox::new(Orientation::Vertical, 0);
    spacer.set_height_request(8);
    card.append(&spacer);

    let btn = Button::new();
    btn.set_child(Some(&card));
    btn.add_css_class("flat");

    let app_clone = app.clone();
    let nav_c = Arc::clone(&nav);

    btn.connect_clicked(move |_| {
        let detail_w = build_webapp_detail(&app_clone);
        let nav_page = NavigationPage::builder()
            .title(&app_clone.name)
            .child(&detail_w)
            .build();
        nav_c.push(&nav_page);
    });

    btn.upcast()
}

fn build_webapp_detail(app: &WebApp) -> Widget {
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

    // Hero row
    let hero = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(20)
        .build();

    let icon = load_app_icon(&app.icon_path, &app.icon_url, 96, &app.name);
    hero.append(&icon);

    let meta = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .valign(Align::Center)
        .hexpand(true)
        .build();

    let name_lbl = Label::builder()
        .label(&app.name)
        .halign(Align::Start)
        .css_classes(vec!["title-1".to_string()])
        .build();
    let summary_lbl = Label::builder()
        .label(&app.summary)
        .halign(Align::Start)
        .wrap(true)
        .build();
    let url_lbl = Label::builder()
        .label(&app.url)
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .build();

    meta.append(&name_lbl);
    meta.append(&summary_lbl);
    meta.append(&url_lbl);
    hero.append(&meta);

    // Install / Uninstall button
    let is_installed = app.installed;
    let install_btn = Button::builder()
        .label(if is_installed { "Uninstall" } else { "Install" })
        .valign(Align::Center)
        .css_classes(if is_installed {
            vec!["destructive-action".to_string(), "pill".to_string()]
        } else {
            vec!["suggested-action".to_string(), "pill".to_string()]
        })
        .build();

    let app_id = app.id.clone();
    // Use a RefCell-wrapped bool to track install state
    let installed_cell = std::rc::Rc::new(std::cell::Cell::new(is_installed));
    let installed_cell_c = installed_cell.clone();

    install_btn.connect_clicked(move |btn| {
        let currently = installed_cell_c.get();
        let pkg = app_id.clone();
        btn.set_sensitive(false);
        btn.set_label(if currently { "Uninstalling…" } else { "Installing…" });
        let btn_c = btn.clone();
        let ic = installed_cell_c.clone();
        std::thread::spawn(move || {
            let (ok, _msg) = if currently {
                rakuos_webapps::uninstall(&pkg)
            } else {
                rakuos_webapps::install(&pkg)
            };
            glib::idle_add_once(move || {
                btn_c.set_sensitive(true);
                if ok {
                    let new_state = !currently;
                    ic.set(new_state);
                    if new_state {
                        btn_c.set_label("Uninstall");
                        btn_c.remove_css_class("suggested-action");
                        btn_c.add_css_class("destructive-action");
                    } else {
                        btn_c.set_label("Install");
                        btn_c.remove_css_class("destructive-action");
                        btn_c.add_css_class("suggested-action");
                    }
                } else {
                    btn_c.set_label(if currently { "Uninstall" } else { "Install" });
                }
            });
        });
    });

    hero.append(&install_btn);
    main_box.append(&hero);
    main_box.append(&gtk4::Separator::new(Orientation::Horizontal));

    if !app.description.is_empty() {
        let desc = Label::builder()
            .label(&app.description)
            .halign(Align::Start)
            .wrap(true)
            .selectable(true)
            .build();
        main_box.append(&desc);
    }

    scroll.upcast()
}
