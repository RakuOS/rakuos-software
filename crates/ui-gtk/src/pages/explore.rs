// pages/explore.rs — Explore page: top-level category grid → sub-cat tiles + app grid

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, FlowBox, FlowBoxChild, Label, Orientation, ScrolledWindow,
    SelectionMode, Widget,
};
use libadwaita::prelude::*;
use libadwaita::{HeaderBar, NavigationPage, NavigationView, ToolbarView};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use rakuos_packages::NativeApp;
use super::icon_helper::load_app_icon;

// ── Category tree (mirrors Qt main.qml) ──────────────────────────────────────

struct TopCat {
    label: &'static str,
    cat: &'static str,
    icon: &'static str,
    subs: &'static [(&'static str, &'static str)],
}

const CATEGORIES: &[TopCat] = &[
    TopCat { label: "Games",        cat: "Game",        icon: "applications-games-symbolic", subs: &[
        ("Action",       "ActionGame"),
        ("Adventure",    "AdventureGame"),
        ("Arcade",       "ArcadeGame"),
        ("Board Games",  "BoardGame"),
        ("Card Games",   "CardGame"),
        ("Emulators",    "Emulator"),
        ("Kids",         "KidsGame"),
        ("Logic",        "LogicGame"),
        ("Role Playing", "RolePlaying"),
        ("Shooter",      "Shooter"),
        ("Simulation",   "Simulation"),
        ("Sports",       "SportsGame"),
        ("Strategy",     "StrategyGame"),
    ]},
    TopCat { label: "Internet",     cat: "Network",     icon: "network-wireless-symbolic", subs: &[
        ("Web Browsers",  "WebBrowser"),
        ("Email",         "Email"),
        ("Chat & IM",     "InstantMessaging"),
        ("File Sharing",  "FileTransfer"),
        ("News & RSS",    "News"),
        ("Remote Access", "RemoteAccess"),
    ]},
    TopCat { label: "Audio & Video", cat: "AudioVideo", icon: "audio-headphones-symbolic", subs: &[
        ("Music",   "Music"),
        ("Video",   "Video"),
        ("Editors", "AudioVideoEditing"),
        ("MIDI",    "Midi"),
        ("TV",      "TV"),
        ("Recorders","Recorder"),
        ("Mixers",  "Mixer"),
    ]},
    TopCat { label: "Graphics",     cat: "Graphics",    icon: "applications-graphics-symbolic", subs: &[
        ("2D Editors",  "2DGraphics"),
        ("3D Editors",  "3DGraphics"),
        ("Photography", "Photography"),
        ("Viewers",     "Viewer"),
        ("Publishing",  "Publishing"),
        ("Scanning",    "Scanning"),
    ]},
    TopCat { label: "Productivity", cat: "Office",      icon: "x-office-document-symbolic", subs: &[
        ("Office Suite", "WordProcessor"),
        ("Spreadsheets", "Spreadsheet"),
        ("Presentation", "Presentation"),
        ("Calendar",     "Calendar"),
        ("Finance",      "Finance"),
        ("Contacts",     "ContactManagement"),
        ("Project Mgmt", "ProjectManagement"),
    ]},
    TopCat { label: "Development",  cat: "Development", icon: "applications-development-symbolic", subs: &[
        ("IDEs",            "IDE"),
        ("Version Control", "RevisionControl"),
        ("Debuggers",       "Debugger"),
        ("Web Dev",         "WebDevelopment"),
        ("Database",        "Database"),
        ("Data Viz",        "DataVisualization"),
    ]},
    TopCat { label: "System",       cat: "System",      icon: "preferences-system-symbolic", subs: &[
        ("File Managers", "FileManager"),
        ("Terminal",      "TerminalEmulator"),
        ("Monitors",      "Monitor"),
        ("Security",      "Security"),
        ("Settings",      "Settings"),
        ("Package Mgmt",  "PackageManager"),
    ]},
    TopCat { label: "Utilities",    cat: "Utility",     icon: "applications-utilities-symbolic", subs: &[
        ("Archiving",   "Archiving"),
        ("Calculators", "Calculator"),
        ("Clocks",      "Clock"),
        ("Text Editors","TextEditor"),
        ("File Tools",  "FileTools"),
        ("Dictionary",  "Dictionary"),
    ]},
    TopCat { label: "Science",      cat: "Science",     icon: "applications-science-symbolic", subs: &[
        ("Astronomy",   "Astronomy"),
        ("Chemistry",   "Chemistry"),
        ("Electronics", "Electronics"),
        ("Engineering", "Engineering"),
        ("Geography",   "Geography"),
        ("Physics",     "Physics"),
    ]},
    TopCat { label: "Education",    cat: "Education",   icon: "applications-education-symbolic", subs: &[
        ("Languages",  "Languages"),
        ("Mathematics","Math"),
        ("Science",    "Science"),
        ("Literature", "Literature"),
        ("Geography",  "Geography"),
        ("Kids",       "KidsGame"),
    ]},
];

// ── Main explore page — only top-level category tiles ─────────────────────────

pub fn build(nav: Arc<NavigationView>) -> Widget {
    let scroll = ScrolledWindow::builder()
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
    scroll.set_child(Some(&main_box));

    let lbl = Label::builder()
        .label("Browse by Category")
        .halign(Align::Start)
        .css_classes(vec!["title-2".to_string()])
        .build();
    main_box.append(&lbl);

    let flow = FlowBox::builder()
        .selection_mode(SelectionMode::None)
        .min_children_per_line(2)
        .max_children_per_line(4)
        .column_spacing(12)
        .row_spacing(12)
        .homogeneous(true)
        .build();

    for top in CATEGORIES {
        let card = build_top_cat_card(top.label, top.cat, top.icon, top.subs, Arc::clone(&nav));
        let child = FlowBoxChild::new();
        child.set_child(Some(&card));
        child.set_focusable(false);
        flow.insert(&child, -1);
    }
    main_box.append(&flow);

    scroll.upcast()
}

// ── Top-level category card ───────────────────────────────────────────────────

fn build_top_cat_card(
    cat_label: &str,
    cat_id: &str,
    icon_name: &str,
    subs: &'static [(&'static str, &'static str)],
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
        .label(cat_label)
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

    let label_str = cat_label.to_string();
    let id_str    = cat_id.to_string();
    let nav_c = Arc::clone(&nav);

    btn.connect_clicked(move |_| {
        let page_w = build_category_page(&id_str, &label_str, subs, Arc::clone(&nav_c));
        let nav_page = NavigationPage::builder()
            .title(&label_str)
            .child(&page_w)
            .build();
        nav_c.push(&nav_page);
    });

    btn.upcast()
}

// ── Category page: sub-cat grid + top apps flow ───────────────────────────────

fn build_category_page(
    cat_id: &str,
    cat_label: &str,
    subs: &'static [(&'static str, &'static str)],
    nav: Arc<NavigationView>,
) -> Widget {
    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&HeaderBar::new());

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let main_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(12)
        .margin_bottom(24)
        .margin_start(16)
        .margin_end(16)
        .build();
    scroll.set_child(Some(&main_box));

    // ── Sub-category tile grid ────────────────────────────────────────────────
    if !subs.is_empty() {
        let sub_lbl = Label::builder()
            .label("Sub-categories")
            .halign(Align::Start)
            .css_classes(vec!["title-3".to_string()])
            .build();
        main_box.append(&sub_lbl);

        let sub_flow = FlowBox::builder()
            .selection_mode(SelectionMode::None)
            .min_children_per_line(2)
            .max_children_per_line(3)
            .column_spacing(10)
            .row_spacing(10)
            .homogeneous(true)
            .build();

        for (sub_label, sub_cat) in subs {
            let tile = build_sub_cat_tile(sub_label, sub_cat, Arc::clone(&nav));
            let child = FlowBoxChild::new();
            child.set_child(Some(&tile));
            child.set_focusable(false);
            sub_flow.insert(&child, -1);
        }
        main_box.append(&sub_flow);
    }

    // ── Top apps header row ───────────────────────────────────────────────────
    let top_header = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .build();

    let top_lbl = Label::builder()
        .label(&format!("Top {} Apps", cat_label))
        .halign(Align::Start)
        .hexpand(true)
        .css_classes(vec!["title-3".to_string()])
        .build();
    top_header.append(&top_lbl);

    let see_all_btn = Button::builder()
        .label("See all →")
        .css_classes(vec!["flat".to_string()])
        .visible(false)
        .build();
    top_header.append(&see_all_btn);
    main_box.append(&top_header);

    // ── App flow grid ─────────────────────────────────────────────────────────
    let app_flow = FlowBox::builder()
        .selection_mode(SelectionMode::None)
        .min_children_per_line(3)
        .max_children_per_line(8)
        .column_spacing(12)
        .row_spacing(12)
        .homogeneous(true)
        .build();
    main_box.append(&app_flow);

    // ── Load apps in background ───────────────────────────────────────────────
    let cat = cat_id.to_string();
    let (tx, rx) = mpsc::channel::<Vec<NativeApp>>();
    std::thread::spawn(move || {
        let apps = rakuos_packages::get_by_category(&cat).unwrap_or_default();
        let _ = tx.send(apps);
    });

    // Store all apps for "see all" toggle
    let stored_apps: Rc<RefCell<Vec<NativeApp>>> = Rc::new(RefCell::new(Vec::new()));
    let stored_c = Rc::clone(&stored_apps);
    let nav_c = Arc::clone(&nav);
    let flow_c = app_flow.clone();
    let btn_c = see_all_btn.clone();

    glib::timeout_add_local(Duration::from_millis(80), move || {
        match rx.try_recv() {
            Ok(apps) => {
                *stored_c.borrow_mut() = apps.clone();
                populate_app_flow(&flow_c, apps.iter().take(12), Arc::clone(&nav_c));
                if apps.len() > 12 {
                    btn_c.set_label(&format!("See all {} →", apps.len()));
                    btn_c.set_visible(true);
                }
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });

    // "See all" reveals remaining apps
    let nav_sa = Arc::clone(&nav);
    see_all_btn.connect_clicked(move |btn| {
        let apps = stored_apps.borrow();
        populate_app_flow(&app_flow, apps.iter(), Arc::clone(&nav_sa));
        btn.set_visible(false);
    });

    toolbar.set_content(Some(&scroll));
    toolbar.upcast()
}

// ── Sub-category tile ─────────────────────────────────────────────────────────

fn build_sub_cat_tile(
    sub_label: &str,
    sub_cat: &str,
    nav: Arc<NavigationView>,
) -> Widget {
    let row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .css_classes(vec!["card".to_string()])
        .margin_top(2)
        .margin_bottom(2)
        .margin_start(2)
        .margin_end(2)
        .build();

    let lbl = Label::builder()
        .label(sub_label)
        .hexpand(true)
        .halign(Align::Start)
        .margin_start(12)
        .margin_top(14)
        .margin_bottom(14)
        .css_classes(vec!["body".to_string()])
        .build();
    row.append(&lbl);

    let chevron = gtk4::Image::builder()
        .icon_name("go-next-symbolic")
        .margin_end(10)
        .build();
    row.append(&chevron);

    let btn = Button::new();
    btn.set_child(Some(&row));
    btn.add_css_class("flat");

    let label_str = sub_label.to_string();
    let cat_str   = sub_cat.to_string();
    let nav_c = Arc::clone(&nav);

    btn.connect_clicked(move |_| {
        let page_w = build_sub_page(&cat_str, Arc::clone(&nav_c));
        let nav_page = NavigationPage::builder()
            .title(&label_str)
            .child(&page_w)
            .build();
        nav_c.push(&nav_page);
    });

    btn.upcast()
}

// ── Sub-category page: all apps in a flow grid ────────────────────────────────

fn build_sub_page(cat_id: &str, nav: Arc<NavigationView>) -> Widget {
    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&HeaderBar::new());

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let app_flow = FlowBox::builder()
        .selection_mode(SelectionMode::None)
        .min_children_per_line(3)
        .max_children_per_line(8)
        .column_spacing(12)
        .row_spacing(12)
        .homogeneous(true)
        .margin_top(12)
        .margin_bottom(24)
        .margin_start(16)
        .margin_end(16)
        .build();
    scroll.set_child(Some(&app_flow));

    let cat = cat_id.to_string();
    let (tx, rx) = mpsc::channel::<Vec<NativeApp>>();
    std::thread::spawn(move || {
        let apps = rakuos_packages::get_by_category(&cat).unwrap_or_default();
        let _ = tx.send(apps);
    });

    let nav_c = Arc::clone(&nav);
    glib::timeout_add_local(Duration::from_millis(80), move || {
        match rx.try_recv() {
            Ok(apps) => {
                if !apps.is_empty() {
                    populate_app_flow(&app_flow, apps.iter(), Arc::clone(&nav_c));
                }
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });

    toolbar.set_content(Some(&scroll));
    toolbar.upcast()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn populate_app_flow<'a>(
    flow: &FlowBox,
    apps: impl Iterator<Item = &'a NativeApp>,
    nav: Arc<NavigationView>,
) {
    while let Some(child) = flow.first_child() {
        flow.remove(&child);
    }
    for app in apps {
        let card = build_app_card(app, Arc::clone(&nav));
        let child = FlowBoxChild::new();
        child.set_child(Some(&card));
        child.set_focusable(false);
        flow.insert(&child, -1);
    }
}

fn build_app_card(app: &NativeApp, nav: Arc<NavigationView>) -> Widget {
    let card = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .width_request(120)
        .css_classes(vec!["card".to_string()])
        .margin_top(4)
        .margin_bottom(4)
        .build();

    let icon = load_app_icon(&app.icon_path, &app.icon_url, 56, &app.name);
    icon.set_halign(Align::Center);
    icon.set_margin_top(10);
    card.append(&icon);

    if app.installed {
        let badge = Label::builder()
            .label("✓")
            .css_classes(vec!["caption".to_string()])
            .halign(Align::End)
            .margin_end(8)
            .build();
        card.append(&badge);
    }

    let name_lbl = Label::builder()
        .label(&app.name)
        .halign(Align::Center)
        .wrap(false)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(14)
        .css_classes(vec!["caption-heading".to_string()])
        .margin_start(8)
        .margin_end(8)
        .margin_bottom(10)
        .build();
    card.append(&name_lbl);

    let btn = Button::new();
    btn.set_child(Some(&card));
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

// build_list_row kept for any external callers
pub fn build_list_row(app: &NativeApp, nav: Arc<NavigationView>) -> Widget {
    build_app_card(app, nav)
}
