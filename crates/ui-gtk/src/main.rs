// rakuos-software-gtk — GTK4/libadwaita software center frontend

mod pages;

use gtk4::prelude::*;
use gtk4::{gio, glib, Align, Box as GBox, Label, Orientation, Spinner, Stack};
use libadwaita::prelude::*;
use libadwaita::{
    Application, ApplicationWindow, HeaderBar, ToastOverlay, ViewStack,
    ViewSwitcher, ViewSwitcherPolicy,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const APP_ID: &str = "org.rakuos.Software";

// ── Daemon / PID helpers (mirrors rakuos-software-qt) ─────────────────────────

fn pid_file_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".cache/rakuos/software-ui.pid")
}

fn show_flag_path() -> PathBuf {
    std::env::temp_dir().join("rakuos-software-show")
}

fn quit_flag_path() -> PathBuf {
    std::env::temp_dir().join("rakuos-software-quit")
}

fn write_pid_file() {
    let path = pid_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, std::process::id().to_string());
}

fn daemon_binary() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let sibling = exe
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .join("rakuos-software-tray");
        if sibling.exists() {
            return sibling;
        }
    }
    PathBuf::from("rakuos-software-tray")
}

fn ensure_daemon_running() {
    let running = std::process::Command::new("pgrep")
        .args(["-f", "rakuos-software-tray"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !running {
        log::info!("Starting rakuos-software-tray daemon...");
        let _ = std::process::Command::new(daemon_binary()).spawn();
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> glib::ExitCode {
    env_logger::init();

    // --tray: start hidden so the daemon can raise us on demand
    let start_hidden = std::env::args().any(|a| a == "--tray");

    write_pid_file();
    ensure_daemon_running();

    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    // Normal launch (no file argument) → build UI or raise existing window.
    app.connect_activate(move |app| {
        if app.windows().is_empty() {
            build_ui(app, start_hidden);
        } else {
            // Second launch forwarded by GIO single-instance — signal the polling
            // timer so it navigates home before raising the window.
            let _ = std::fs::write(show_flag_path(), "1");
        }
    });

    // Launched with a file argument (first launch OR second instance forwarded
    // by GIO single-instance to the primary via D-Bus).
    app.connect_open(move |app, files, _| {
        // Write the flag file first so build_ui can read it immediately.
        if let Some(path) = files.first().and_then(|f| f.path()) {
            let _ = std::fs::write(open_file_flag_path(), path.to_string_lossy().as_ref());
        }
        if app.windows().is_empty() {
            // First launch with a file — build window (it reads flag at the end).
            build_ui(app, false);
        } else {
            // Window already exists — polling timer picks up the flag within 500 ms.
            if let Some(win) = app.windows().first() {
                win.present();
            }
        }
    });

    let exit = app.run();
    let _ = std::fs::remove_file(pid_file_path());
    exit
}

fn open_file_flag_path() -> PathBuf {
    std::env::temp_dir().join("rakuos-software-open-file")
}

fn build_ui(app: &Application, start_hidden: bool) {
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

    // ── Search results page ───────────────────────────────────────────────
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
        pages::detail::cancel_current_detail();
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

    let content_stack = gtk4::Stack::new();
    content_stack.set_vexpand(true);
    content_stack.add_named(&view_stack, Some("main"));
    content_stack.add_named(&search_page_w, Some("search"));
    content_stack.set_visible_child_name("main");

    main_box.append(&content_stack);

    // When search mode activates, switch to search results view
    let cs_clone = content_stack.clone();
    search_bar.connect_search_mode_enabled_notify(move |bar| {
        if bar.is_search_mode() {
            cs_clone.set_visible_child_name("search");
        } else {
            cs_clone.set_visible_child_name("main");
        }
    });

    // Trigger search on entry change
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
        let can_pop = nv.previous_page(nv.visible_page().as_ref().unwrap()).is_some();
        back_btn_c.set_visible(can_pop);
    });

    // ── Actions for hamburger menu ────────────────────────────────────────
    let nav_sys = Arc::clone(&nav_arc);
    let action_system = gtk4::gio::SimpleAction::new("show-system", None);
    action_system.connect_activate(move |_, _| {
        let page_w = pages::system::build();
        let toolbar = libadwaita::ToolbarView::new();
        toolbar.add_top_bar(&libadwaita::HeaderBar::new());
        toolbar.set_content(Some(&page_w));
        let nav_page = libadwaita::NavigationPage::builder()
            .title("System")
            .child(&toolbar)
            .build();
        nav_sys.push(&nav_page);
    });

    let nav_set = Arc::clone(&nav_arc);
    let action_settings = gtk4::gio::SimpleAction::new("show-settings", None);
    action_settings.connect_activate(move |_, _| {
        let page_w = pages::settings::build();
        let toolbar = libadwaita::ToolbarView::new();
        toolbar.add_top_bar(&libadwaita::HeaderBar::new());
        toolbar.set_content(Some(&page_w));
        let nav_page = libadwaita::NavigationPage::builder()
            .title("Settings")
            .child(&toolbar)
            .build();
        nav_set.push(&nav_page);
    });

    let win_about = window.clone();
    let action_about = gtk4::gio::SimpleAction::new("show-about", None);
    action_about.connect_activate(move |_, _| {
        let dialog = libadwaita::AboutDialog::builder()
            .application_name("RakuOS Software")
            .application_icon("rakuos-logo")
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

    // ── Root stack: loading splash → main content ─────────────────────────────
    let root_stack = Stack::new();
    root_stack.set_vexpand(true);
    root_stack.set_hexpand(true);

    // Loading splash
    let loading_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(20)
        .halign(Align::Center)
        .valign(Align::Center)
        .build();

    let logo = gtk4::Image::builder()
        .icon_name("rakuos-logo")
        .pixel_size(96)
        .build();
    loading_box.append(&logo);

    let load_spinner = Spinner::builder()
        .spinning(true)
        .halign(Align::Center)
        .width_request(36)
        .height_request(36)
        .build();
    loading_box.append(&load_spinner);

    let load_label = Label::new(Some("Loading software catalog…"));
    load_label.add_css_class("dim-label");
    loading_box.append(&load_label);

    root_stack.add_named(&loading_box, Some("loading"));
    root_stack.add_named(&nav_view, Some("main"));
    root_stack.set_visible_child_name("loading");

    window.set_content(Some(&root_stack));

    // Warm appstream cache in background; switch to main content when ready
    let cache_ready = Arc::new(AtomicBool::new(false));
    let cache_ready_bg = Arc::clone(&cache_ready);
    std::thread::spawn(move || {
        let _ = rakuos_appstream::get_appstream();
        cache_ready_bg.store(true, Ordering::Release);
    });

    let root_stack_c = root_stack.clone();
    glib::timeout_add_local(Duration::from_millis(200), move || {
        if cache_ready.load(Ordering::Acquire) {
            root_stack_c.set_visible_child_name("main");
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });

    // ── Tray show-flag + open-file + quit-flag polling ───────────────────────
    let window_raise = window.clone();
    let nav_open = Arc::clone(&nav_arc);
    let view_stack_raise = view_stack.clone();
    let content_stack_raise = content_stack.clone();
    let search_bar_raise = search_bar.clone();
    glib::timeout_add_local(Duration::from_millis(500), move || {
        // Quit flag: daemon requested full shutdown — bypass close-request handler.
        if quit_flag_path().exists() {
            let _ = std::fs::remove_file(quit_flag_path());
            std::process::exit(0);
        }
        // Tray raise (or second-launch via connect_activate)
        let flag = show_flag_path();
        if flag.exists() {
            let _ = std::fs::remove_file(&flag);
            // Dismiss search if active
            search_bar_raise.set_search_mode(false);
            content_stack_raise.set_visible_child_name("main");
            // Cancel any in-flight detail page loads, then pop to root
            pages::detail::cancel_current_detail();
            while nav_open.pop() {}
            // Switch to Home tab
            view_stack_raise.set_visible_child_name("home");
            window_raise.present();
        }
        // Open-file request from connect_open (single-instance second launch)
        let open_flag = open_file_flag_path();
        if open_flag.exists() {
            if let Ok(path) = std::fs::read_to_string(&open_flag) {
                let _ = std::fs::remove_file(&open_flag);
                let path = path.trim().to_string();
                if !path.is_empty() {
                    pages::local_install::push_local_install(&nav_open, &path);
                    window_raise.present();
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // Open startup file immediately if one was written to the flag file
    // (written by main() before app.run() so it's always present on first launch).
    let open_flag = open_file_flag_path();
    if open_flag.exists() {
        if let Ok(path) = std::fs::read_to_string(&open_flag) {
            let _ = std::fs::remove_file(&open_flag);
            let path = path.trim().to_string();
            if !path.is_empty() {
                pages::local_install::push_local_install(&nav_arc, &path);
            }
        }
    }

    // Close → hide (keep running so the tray daemon can raise us again)
    window.connect_close_request(|win| {
        win.set_visible(false);
        glib::Propagation::Stop
    });

    // Present immediately unless started via --tray (background mode)
    if !start_hidden {
        window.present();
    }
}
