// pages/home.rs — Home page with hero carousel, editor's picks and popular sections

use gtk4::prelude::*;
use gtk4::{glib, Align, Box as GBox, Button, FlowBox, FlowBoxChild, Label, Orientation, ScrolledWindow, SelectionMode, Widget};
use libadwaita::prelude::*;
use libadwaita::{Carousel, NavigationView};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use rakuos_home::HomeApp;
use super::icon_helper::load_app_icon;

pub fn build(nav: Arc<NavigationView>) -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let main_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(24)
        .margin_top(12)
        .margin_bottom(24)
        .build();
    scroll.set_child(Some(&main_box));

    let hero_container = GBox::builder()
        .orientation(Orientation::Vertical)
        .height_request(260)
        .build();
    main_box.append(&hero_container);

    let (picks_outer, picks_inner) = build_section_placeholder("Editor's Choice");
    let (popular_outer, popular_inner) = build_section_placeholder("Popular");
    let (updated_outer, updated_inner) = build_section_placeholder("Recently Updated");
    let (new_outer, new_inner) = build_section_placeholder("New Apps");

    main_box.append(&picks_outer);
    main_box.append(&popular_outer);
    main_box.append(&updated_outer);
    main_box.append(&new_outer);

    // ── Background load via mpsc (GTK objects never cross thread boundary) ─
    type HomeData = (Vec<HomeApp>, Vec<HomeApp>, Vec<HomeApp>, Vec<HomeApp>);
    let (tx, rx) = mpsc::channel::<HomeData>();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let data = rt.block_on(rakuos_home::load_all());
        let _ = tx.send(data);
    });

    let nav_c = Arc::clone(&nav);
    glib::timeout_add_local(Duration::from_millis(80), move || {
        match rx.try_recv() {
            Ok((picks, popular, updated, new)) => {
                build_hero_carousel(&hero_container, &picks, Arc::clone(&nav_c));
                populate_section(&picks_inner, &picks, Arc::clone(&nav_c));
                populate_section(&popular_inner, &popular, Arc::clone(&nav_c));
                populate_section(&updated_inner, &updated, Arc::clone(&nav_c));
                populate_section(&new_inner, &new, Arc::clone(&nav_c));
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });

    scroll.upcast()
}

fn build_section_placeholder(title: &str) -> (GBox, FlowBox) {
    let outer = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .margin_start(16)
        .margin_end(16)
        .build();

    let lbl = Label::builder()
        .label(title)
        .halign(Align::Start)
        .css_classes(vec!["title-2".to_string()])
        .build();
    outer.append(&lbl);

    let inner = FlowBox::builder()
        .selection_mode(SelectionMode::None)
        .min_children_per_line(2)
        .max_children_per_line(6)
        .column_spacing(8)
        .row_spacing(8)
        .homogeneous(true)
        .build();

    outer.append(&inner);
    (outer, inner)
}

fn build_hero_carousel(container: &GBox, apps: &[HomeApp], nav: Arc<NavigationView>) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    if apps.is_empty() {
        let sp = libadwaita::StatusPage::builder()
            .title("Nothing to show")
            .icon_name("computer-symbolic")
            .build();
        container.append(&sp);
        return;
    }

    let carousel_wrap = GBox::builder().orientation(Orientation::Vertical).build();

    let carousel = Carousel::builder()
        .allow_mouse_drag(true)
        .allow_scroll_wheel(true)
        .hexpand(true)
        .height_request(220)
        .build();

    let indicator = libadwaita::CarouselIndicatorDots::builder()
        .carousel(&carousel)
        .build();

    let overlay = gtk4::Overlay::new();
    overlay.set_child(Some(&carousel));

    let prev_btn = Button::builder()
        .icon_name("go-previous-symbolic")
        .valign(Align::Center)
        .halign(Align::Start)
        .margin_start(8)
        .css_classes(vec!["circular".to_string(), "osd".to_string()])
        .build();
    let next_btn = Button::builder()
        .icon_name("go-next-symbolic")
        .valign(Align::Center)
        .halign(Align::End)
        .margin_end(8)
        .css_classes(vec!["circular".to_string(), "osd".to_string()])
        .build();

    overlay.add_overlay(&prev_btn);
    overlay.add_overlay(&next_btn);

    let c = carousel.clone();
    prev_btn.connect_clicked(move |_| {
        let pos = c.position().round() as usize;
        let n = c.n_pages() as usize;
        if n > 0 {
            let target = if pos == 0 { n - 1 } else { pos - 1 };
            let page = c.nth_page(target as u32);
            c.scroll_to(&page, true);
        }
    });

    let c = carousel.clone();
    next_btn.connect_clicked(move |_| {
        let pos = c.position().round() as usize;
        let n = c.n_pages() as usize;
        if n > 0 {
            let target = (pos + 1) % n;
            let page = c.nth_page(target as u32);
            c.scroll_to(&page, true);
        }
    });

    for app in apps.iter().take(8) {
        let slide = build_hero_slide(app, Arc::clone(&nav));
        carousel.append(&slide);
    }

    carousel_wrap.append(&overlay);
    carousel_wrap.append(&indicator);
    container.append(&carousel_wrap);

    let c = carousel.clone();
    glib::timeout_add_seconds_local(5, move || {
        let pos = c.position().round() as usize;
        let n = c.n_pages() as usize;
        if n > 0 {
            let target = (pos + 1) % n;
            let page = c.nth_page(target as u32);
            c.scroll_to(&page, true);
        }
        glib::ControlFlow::Continue
    });
}

fn build_hero_slide(app: &HomeApp, nav: Arc<NavigationView>) -> Widget {
    let card = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(20)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(24)
        .margin_end(24)
        .hexpand(true)
        .css_classes(vec!["card".to_string()])
        .build();

    let icon = load_app_icon(&app.icon_path, &app.icon_url, 80, &app.name);
    card.append(&icon);

    let text_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .valign(Align::Center)
        .hexpand(true)
        .build();
    let name_lbl = Label::builder()
        .label(&app.name)
        .halign(Align::Start)
        .css_classes(vec!["title-1".to_string()])
        .wrap(true)
        .build();
    let summary_lbl = Label::builder()
        .label(&app.summary)
        .halign(Align::Start)
        .wrap(true)
        .build();
    text_box.append(&name_lbl);
    text_box.append(&summary_lbl);
    card.append(&text_box);

    let get_btn = Button::builder()
        .label("Get")
        .valign(Align::Center)
        .css_classes(vec!["suggested-action".to_string(), "pill".to_string()])
        .build();

    let (id, name, summary, ip, iu, src) = (
        app.id.clone(), app.name.clone(), app.summary.clone(),
        app.icon_path.clone(), app.icon_url.clone(), app.source.clone(),
    );
    let nav2 = Arc::clone(&nav);
    get_btn.connect_clicked(move |_| {
        super::detail::push_detail(&nav2, &id, &name, &summary, &ip, &iu, &src);
    });
    card.append(&get_btn);

    let btn = Button::new();
    btn.set_child(Some(&card));
    btn.add_css_class("flat");
    btn.set_hexpand(true);

    let (id, name, summary, ip, iu, src) = (
        app.id.clone(), app.name.clone(), app.summary.clone(),
        app.icon_path.clone(), app.icon_url.clone(), app.source.clone(),
    );
    btn.connect_clicked(move |_| {
        super::detail::push_detail(&nav, &id, &name, &summary, &ip, &iu, &src);
    });

    btn.upcast()
}

fn populate_section(container: &FlowBox, apps: &[HomeApp], nav: Arc<NavigationView>) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    for app in apps.iter().take(12) {
        let card = build_app_card(app, Arc::clone(&nav));
        let child = FlowBoxChild::new();
        child.set_child(Some(&card));
        child.set_focusable(false);
        container.insert(&child, -1);
    }
}

fn build_app_card(app: &HomeApp, nav: Arc<NavigationView>) -> Widget {
    let card_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .width_request(160)
        .css_classes(vec!["card".to_string()])
        .margin_top(4).margin_bottom(4).margin_start(4).margin_end(4)
        .build();

    let icon = load_app_icon(&app.icon_path, &app.icon_url, 64, &app.name);
    icon.set_halign(Align::Center);
    icon.set_margin_top(12);
    card_box.append(&icon);

    let name_lbl = Label::builder()
        .label(&app.name)
        .halign(Align::Center)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(18)
        .css_classes(vec!["caption-heading".to_string()])
        .margin_start(8).margin_end(8)
        .build();
    card_box.append(&name_lbl);

    let summary_lbl = Label::builder()
        .label(&app.summary)
        .halign(Align::Center)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(20)
        .css_classes(vec!["caption".to_string()])
        .margin_start(8).margin_end(8).margin_bottom(12)
        .build();
    card_box.append(&summary_lbl);

    let btn = Button::new();
    btn.set_child(Some(&card_box));
    btn.add_css_class("flat");

    let (id, name, summary, ip, iu, src) = (
        app.id.clone(), app.name.clone(), app.summary.clone(),
        app.icon_path.clone(), app.icon_url.clone(), app.source.clone(),
    );
    btn.connect_clicked(move |_| {
        super::detail::push_detail(&nav, &id, &name, &summary, &ip, &iu, &src);
    });

    btn.upcast()
}
