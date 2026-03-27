// rakuos-software-gtk — GTK4/libadwaita software center frontend

mod pages;

use gtk4::prelude::*;
use gtk4::{glib, Align, Box as GBox, Orientation, Separator};
use libadwaita::prelude::*;
use libadwaita::{
    Application, ApplicationWindow, HeaderBar, ToastOverlay, ViewStack,
    ViewSwitcher, ViewSwitcherPolicy,
};
use std::sync::Arc;

const APP_ID: &str = "org.rakuos.Software";

fn main() -> glib::ExitCode {
    env_logger::init();
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("RakuOS Software")
        .default_width(1100)
        .default_height(720)
        .build();

    // ── Navigation view (allows pushing detail pages) ──────────────────────
    let nav_view = libadwaita::NavigationView::new();
    let nav_arc = Arc::new(nav_view.clone());

    // ── ViewStack for main pages ───────────────────────────────────────────
    let view_stack = ViewStack::new();

    // Build all pages
    let home_w = pages::home::build(Arc::clone(&nav_arc));
    let explore_w = pages::explore::build(Arc::clone(&nav_arc));
    let webapps_w = pages::webapps::build(Arc::clone(&nav_arc));
    let installed_w = pages::installed::build(Arc::clone(&nav_arc));
    let updates_w = pages::updates::build(Arc::clone(&nav_arc));

    view_stack
        .add_titled_with_icon(&home_w, Some("home"), "Home", "go-home-symbolic")
        .set_needs_attention(false);
    view_stack
        .add_titled_with_icon(&explore_w, Some("explore"), "Explore", "edit-find-symbolic")
        .set_needs_attention(false);
    view_stack
        .add_titled_with_icon(&webapps_w, Some("webapps"), "Web Apps", "web-browser-symbolic")
        .set_needs_attention(false);
    view_stack
        .add_titled_with_icon(&installed_w, Some("installed"), "Installed", "emblem-default-symbolic")
        .set_needs_attention(false);
    view_stack
        .add_titled_with_icon(&updates_w, Some("updates"), "Updates", "software-update-available-symbolic")
        .set_needs_attention(false);

    // ── ViewSwitcher (center of header) ───────────────────────────────────
    let view_switcher = ViewSwitcher::builder()
        .stack(&view_stack)
        .policy(ViewSwitcherPolicy::Wide)
        .build();

    // ── Search bar ────────────────────────────────────────────────────────
    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Search apps…")
        .hexpand(true)
        .build();
    let search_bar = gtk4::SearchBar::builder()
        .child(&search_entry)
        .show_close_button(true)
        .build();

    // ── Search results page (hidden inside view stack or shown below header)
    let search_page_w = pages::search::build(Arc::clone(&nav_arc));

    // ── Hamburger menu ────────────────────────────────────────────────────
    let menu_model = gtk4::gio::Menu::new();
    menu_model.append(Some("System"), Some("win.show-system"));
    menu_model.append(Some("Settings"), Some("win.show-settings"));
    menu_model.append(Some("About RakuOS Software"), Some("win.show-about"));

    let hamburger = gtk4::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu_model)
        .build();

    // ── Back button ───────────────────────────────────────────────────────
    let back_btn = gtk4::Button::builder()
        .icon_name("go-previous-symbolic")
        .visible(false)
        .build();

    let nav_clone = nav_arc.clone();
    back_btn.connect_clicked(move |_| {
        nav_clone.pop();
    });

    // ── Search toggle button ──────────────────────────────────────────────
    let search_btn = gtk4::ToggleButton::builder()
        .icon_name("system-search-symbolic")
        .build();

    search_bar
        .bind_property("search-mode-enabled", &search_btn, "active")
        .bidirectional()
        .sync_create()
        .build();

    // ── HeaderBar ─────────────────────────────────────────────────────────
    let header = HeaderBar::new();
    header.set_title_widget(Some(&view_switcher));
    header.pack_start(&back_btn);
    header.pack_end(&hamburger);
    header.pack_end(&search_btn);

    // ── Main content area ─────────────────────────────────────────────────
    let main_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .build();
    main_box.append(&header);
    main_box.append(&search_bar);

    // Search results overlay: show search_page when search active, else view_stack
    let content_stack = gtk4::Stack::new();
    content_stack.set_vexpand(true);
    content_stack.add_named(&view_stack, Some("main"));
    content_stack.add_named(&search_page_w, Some("search"));
    content_stack.set_visible_child_name("main");

    main_box.append(&content_stack);

    // Connect search entry to search page
    let cs_clone = content_stack.clone();
    let search_entry_c = search_entry.clone();
    let nav_search = Arc::clone(&nav_arc);

    // When search mode activates, switch to search results view
    search_bar.connect_search_mode_enabled_notify(move |bar| {
        if bar.is_search_mode() {
            cs_clone.set_visible_child_name("search");
        } else {
            cs_clone.set_visible_child_name("main");
        }
    });

    // Trigger search on entry change
    let search_entry_c2 = search_entry.clone();
    search_entry.connect_search_changed(move |entry| {
        let query = entry.text().to_string();
        pages::search::run_search(&search_page_w, query);
    });

    // ── Toast overlay wraps everything ────────────────────────────────────
    let toast_overlay = ToastOverlay::new();
    toast_overlay.set_child(Some(&main_box));

    // ── NavigationPage for home ───────────────────────────────────────────
    let home_nav_page = libadwaita::NavigationPage::builder()
        .title("RakuOS Software")
        .child(&toast_overlay)
        .build();

    nav_view.push(&home_nav_page);

    // Track nav stack depth for back button visibility
    let back_btn_c = back_btn.clone();
    nav_view.connect_visible_page_notify(move |nv| {
        // Show back button when we can navigate back (depth > 1)
        let can_pop = nv.previous_page(nv.visible_page().as_ref().unwrap()).is_some();
        back_btn_c.set_visible(can_pop);
    });

    // ── Actions for hamburger menu ────────────────────────────────────────
    let nav_sys = Arc::clone(&nav_arc);
    let action_system = gtk4::gio::SimpleAction::new("show-system", None);
    action_system.connect_activate(move |_, _| {
        let page_w = pages::system::build();
        let nav_page = libadwaita::NavigationPage::builder()
            .title("System")
            .child(&page_w)
            .build();
        nav_sys.push(&nav_page);
    });

    let nav_set = Arc::clone(&nav_arc);
    let action_settings = gtk4::gio::SimpleAction::new("show-settings", None);
    action_settings.connect_activate(move |_, _| {
        let page_w = pages::settings::build();
        let nav_page = libadwaita::NavigationPage::builder()
            .title("Settings")
            .child(&page_w)
            .build();
        nav_set.push(&nav_page);
    });

    let win_about = window.clone();
    let action_about = gtk4::gio::SimpleAction::new("show-about", None);
    action_about.connect_activate(move |_, _| {
        let dialog = libadwaita::AboutDialog::builder()
            .application_name("RakuOS Software")
            .application_icon("org.rakuos.Software")
            .version("1.0.0")
            .developer_name("RakuOS Project")
            .license_type(gtk4::License::Gpl30)
            .website("https://rakuos.org")
            .build();
        dialog.present(Some(&win_about));
    });

    window.add_action(&action_system);
    window.add_action(&action_settings);
    window.add_action(&action_about);

    window.set_content(Some(&nav_view));
    window.present();
}
