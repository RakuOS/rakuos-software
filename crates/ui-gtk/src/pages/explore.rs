// pages/explore.rs — Explore page with top apps and category grid

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, FlowBox, FlowBoxChild, Label, Orientation, ScrolledWindow,
    SelectionMode, Widget,
};
use libadwaita::prelude::*;
use libadwaita::NavigationView;
use std::sync::Arc;

use rakuos_packages::NativeApp;

use super::icon_helper::load_app_icon;

const CATEGORIES: &[(&str, &str, &str)] = &[
    ("AudioVideo", "Audio & Video", "audio-headphones-symbolic"),
    ("Game", "Games", "applications-games-symbolic"),
    ("Graphics", "Graphics", "applications-graphics-symbolic"),
    ("Network", "Internet", "network-wireless-symbolic"),
    ("Office", "Office", "x-office-document-symbolic"),
    ("Science", "Science & Education", "applications-science-symbolic"),
    ("System", "System", "preferences-system-symbolic"),
    ("Development", "Development", "applications-development-symbolic"),
    ("Utility", "Accessories", "applications-utilities-symbolic"),
];

pub fn build(nav: Arc<NavigationView>) -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let main_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(20)
        .margin_top(16)
        .margin_bottom(24)
        .margin_start(16)
        .margin_end(16)
        .build();
    scroll.set_child(Some(&main_box));

    // ── Top Apps section ──────────────────────────────────────────────────
    let top_lbl = Label::builder()
        .label("Top Apps")
        .halign(Align::Start)
        .css_classes(vec!["title-2".to_string()])
        .build();
    main_box.append(&top_lbl);

    let top_apps_inner = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .build();
    let top_apps_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .child(&top_apps_inner)
        .build();
    main_box.append(&top_apps_scroll);

    // ── Category grid ─────────────────────────────────────────────────────
    let cat_lbl = Label::builder()
        .label("Browse by Category")
        .halign(Align::Start)
        .css_classes(vec!["title-2".to_string()])
        .build();
    main_box.append(&cat_lbl);

    let flow = FlowBox::builder()
        .selection_mode(SelectionMode::None)
        .min_children_per_line(2)
        .max_children_per_line(4)
        .column_spacing(12)
        .row_spacing(12)
        .homogeneous(true)
        .build();

    for (cat_id, cat_name, icon_name) in CATEGORIES {
        let card = build_category_card(cat_id, cat_name, icon_name, Arc::clone(&nav));
        let child = FlowBoxChild::new();
        child.set_child(Some(&card));
        child.set_focusable(false);
        flow.insert(&child, -1);
    }
    main_box.append(&flow);

    // ── Load top apps in background ───────────────────────────────────────
    let nav_top = Arc::clone(&nav);
    std::thread::spawn(move || {
        let mut apps: Vec<NativeApp> = Vec::new();
        for (cat_id, _, _) in CATEGORIES.iter().take(4) {
            if let Ok(mut cat_apps) = rakuos_packages::get_by_category(cat_id) {
                cat_apps.truncate(3);
                apps.extend(cat_apps);
            }
        }
        apps.truncate(16);

        glib::idle_add_once(move || {
            for app in &apps {
                let card = build_app_card_small(app, Arc::clone(&nav_top));
                top_apps_inner.append(&card);
            }
        });
    });

    scroll.upcast()
}

fn build_category_card(
    cat_id: &str,
    cat_name: &str,
    icon_name: &str,
    nav: Arc<NavigationView>,
) -> Widget {
    let card = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .css_classes(vec!["card".to_string()])
        .margin_top(4)
        .margin_bottom(4)
        .margin_start(4)
        .margin_end(4)
        .build();

    let icon = gtk4::Image::builder()
        .icon_name(icon_name)
        .pixel_size(32)
        .margin_start(12)
        .margin_top(12)
        .margin_bottom(12)
        .build();
    card.append(&icon);

    let name_lbl = Label::builder()
        .label(cat_name)
        .hexpand(true)
        .halign(Align::Start)
        .css_classes(vec!["body".to_string()])
        .build();
    card.append(&name_lbl);

    let chevron = gtk4::Image::builder()
        .icon_name("go-next-symbolic")
        .margin_end(12)
        .build();
    card.append(&chevron);

    let btn = Button::new();
    btn.set_child(Some(&card));
    btn.add_css_class("flat");

    let cat_id_str = cat_id.to_string();
    let cat_name_str = cat_name.to_string();
    let nav_c = Arc::clone(&nav);

    btn.connect_clicked(move |_| {
        let page_w = build_category_page(&cat_id_str, Arc::clone(&nav_c));
        let nav_page = libadwaita::NavigationPage::builder()
            .title(&cat_name_str)
            .child(&page_w)
            .build();
        nav_c.push(&nav_page);
    });

    btn.upcast()
}

fn build_category_page(cat_id: &str, nav: Arc<NavigationView>) -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let list_box = gtk4::ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .build();
    scroll.set_child(Some(&list_box));

    let cat = cat_id.to_string();
    let nav_c = Arc::clone(&nav);

    std::thread::spawn(move || {
        let apps = rakuos_packages::get_by_category(&cat).unwrap_or_default();
        glib::idle_add_once(move || {
            if apps.is_empty() {
                let sp = libadwaita::StatusPage::builder()
                    .title("No Apps Found")
                    .description("There are no apps in this category")
                    .icon_name("edit-find-symbolic")
                    .build();
                list_box.append(&sp);
            } else {
                for app in &apps {
                    let row = build_list_row(app, Arc::clone(&nav_c));
                    list_box.append(&row);
                }
            }
        });
    });

    scroll.upcast()
}

pub fn build_list_row(app: &NativeApp, nav: Arc<NavigationView>) -> Widget {
    let row_box = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let icon = load_app_icon(&app.icon_path, &app.icon_url, 48, &app.name);
    row_box.append(&icon);

    let text_box = GBox::builder()
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
    let summary_lbl = Label::builder()
        .label(&app.summary)
        .halign(Align::Start)
        .wrap(false)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(50)
        .css_classes(vec!["caption".to_string()])
        .build();
    text_box.append(&name_lbl);
    text_box.append(&summary_lbl);
    row_box.append(&text_box);

    if app.installed {
        let badge = Label::builder()
            .label("Installed")
            .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
            .valign(Align::Center)
            .build();
        row_box.append(&badge);
    }

    let btn = Button::new();
    btn.set_child(Some(&row_box));
    btn.add_css_class("flat");

    let app_id = app.id.clone();
    let app_name = app.name.clone();
    let app_summary = app.summary.clone();
    let icon_path = app.icon_path.clone();
    let icon_url = app.icon_url.clone();
    let source = app.source.clone();

    btn.connect_clicked(move |_| {
        super::detail::push_detail(
            &nav,
            &app_id,
            &app_name,
            &app_summary,
            &icon_path,
            &icon_url,
            &source,
        );
    });

    btn.upcast()
}

fn build_app_card_small(app: &NativeApp, nav: Arc<NavigationView>) -> Widget {
    let card = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .width_request(140)
        .css_classes(vec!["card".to_string()])
        .margin_top(4)
        .margin_bottom(4)
        .build();

    let icon = load_app_icon(&app.icon_path, &app.icon_url, 56, &app.name);
    icon.set_halign(Align::Center);
    icon.set_margin_top(10);
    card.append(&icon);

    let name_lbl = Label::builder()
        .label(&app.name)
        .halign(Align::Center)
        .wrap(false)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(16)
        .css_classes(vec!["caption-heading".to_string()])
        .margin_start(8)
        .margin_end(8)
        .margin_bottom(10)
        .build();
    card.append(&name_lbl);

    let btn = Button::new();
    btn.set_child(Some(&card));
    btn.add_css_class("flat");

    let app_id = app.id.clone();
    let app_name = app.name.clone();
    let app_summary = app.summary.clone();
    let icon_path = app.icon_path.clone();
    let icon_url = app.icon_url.clone();
    let source = app.source.clone();

    btn.connect_clicked(move |_| {
        super::detail::push_detail(
            &nav,
            &app_id,
            &app_name,
            &app_summary,
            &icon_path,
            &icon_url,
            &source,
        );
    });

    btn.upcast()
}
