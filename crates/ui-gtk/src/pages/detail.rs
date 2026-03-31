// pages/detail.rs — App detail page: source selector, add-ons popup, screenshots, reviews

use gtk4::prelude::*;
use gtk4::{glib, Align, Box as GBox, Button, Label, Orientation, ScrolledWindow, Widget};
use libadwaita::prelude::*;
use libadwaita::{Carousel, Dialog, HeaderBar, NavigationPage, NavigationView, ToolbarView};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use rakuos_packages::{NativeApp, SourceOption};
use rakuos_reviews::Review;

use super::icon_helper::load_app_icon;

// ── Active source tracking ────────────────────────────────────────────────────
// Every glib timer created by a detail page is registered here.
// cancel_current_detail() force-removes them all immediately, dropping their
// closures synchronously and releasing every widget ref they held.

thread_local! {
    static ACTIVE_SOURCES: RefCell<Vec<glib::SourceId>> = const { RefCell::new(Vec::new()) };
}

fn track(id: glib::SourceId) {
    ACTIVE_SOURCES.with(|s| s.borrow_mut().push(id));
}

/// Force-cancel all timers belonging to the current detail page.
/// Safe to call even when no detail page is open.
pub fn cancel_current_detail() {
    ACTIVE_SOURCES.with(|sources| {
        for id in sources.borrow_mut().drain(..) {
            // Returns false (no-op) if the source already completed — that is fine.
            let _ = id.remove();
        }
    });
}

/// Push a detail page for an app identified by its ID.
pub fn push_detail(
    nav: &NavigationView,
    app_id: &str,
    app_name: &str,
    app_summary: &str,
    icon_path: &str,
    icon_url: &str,
    source: &str,
) {
    cancel_current_detail(); // drop all timers from any previous detail page first
    let nav_page = NavigationPage::builder()
        .title(app_name)
        .build();
    build(app_id, app_name, app_summary, icon_path, icon_url, source, &nav_page);
    nav.push(&nav_page);
}

fn build(
    app_id: &str,
    app_name: &str,
    app_summary: &str,
    icon_path: &str,
    icon_url: &str,
    source: &str,
    nav_page: &NavigationPage,
) {
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
        .margin_top(24)
        .margin_bottom(32)
        .margin_start(24)
        .margin_end(24)
        .build();
    scroll.set_child(Some(&main_box));

    // ── Hero ─────────────────────────────────────────────────────────────────

    let hero_row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(20)
        .build();

    let icon_w = load_app_icon(icon_path, icon_url, 96, app_name);
    icon_w.set_valign(Align::Start);
    hero_row.append(&icon_w);

    let meta_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .hexpand(true)
        .build();

    let name_lbl = Label::builder()
        .label(app_name)
        .halign(Align::Start)
        .css_classes(vec!["title-1".to_string()])
        .wrap(true)
        .build();
    meta_box.append(&name_lbl);

    let rating_lbl = Label::builder()
        .label("Loading…")
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string()])
        .build();
    meta_box.append(&rating_lbl);

    let summary_lbl = Label::builder()
        .label(app_summary)
        .halign(Align::Start)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    meta_box.append(&summary_lbl);

    let dev_lbl = Label::builder()
        .label("")
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .build();
    meta_box.append(&dev_lbl);

    // Meta row: source badge, version, license
    let meta_row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(4)
        .build();

    let source_badge = Label::builder()
        .label(source_label(source))
        .css_classes(vec!["caption".to_string(), "pill".to_string()])
        .halign(Align::Start)
        .build();
    meta_row.append(&source_badge);

    let version_lbl = Label::builder()
        .label("")
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .halign(Align::Start)
        .visible(false)
        .build();
    meta_row.append(&version_lbl);

    let license_lbl = Label::builder()
        .label("")
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .halign(Align::Start)
        .visible(false)
        .build();
    meta_row.append(&license_lbl);

    meta_box.append(&meta_row);
    hero_row.append(&meta_box);

    // ── Right column: source dropdown + install + add-ons buttons ─────────────
    let btn_col = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .valign(Align::Start)
        .build();

    let dropdown_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .visible(false)
        .build();
    btn_col.append(&dropdown_box);

    let install_btn = Button::builder()
        .label("Install")
        .sensitive(false)
        .css_classes(vec!["suggested-action".to_string(), "pill".to_string()])
        .build();
    btn_col.append(&install_btn);

    let addons_btn = Button::builder()
        .label("Add-ons")
        .css_classes(vec!["pill".to_string()])
        .visible(false)
        .build();
    btn_col.append(&addons_btn);

    hero_row.append(&btn_col);
    main_box.append(&hero_row);
    main_box.append(&gtk4::Separator::new(Orientation::Horizontal));

    // ── Screenshots carousel ──────────────────────────────────────────────────
    let screenshots_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .visible(false)
        .build();

    let screenshots_carousel = Carousel::builder()
        .allow_mouse_drag(true)
        .allow_scroll_wheel(true)
        .hexpand(true)
        .height_request(300)
        .build();

    let screenshot_dots = libadwaita::CarouselIndicatorDots::builder()
        .carousel(&screenshots_carousel)
        .build();

    let ss_overlay = gtk4::Overlay::new();
    ss_overlay.set_child(Some(&screenshots_carousel));

    let ss_prev = Button::builder()
        .icon_name("go-previous-symbolic")
        .valign(Align::Center).halign(Align::Start).margin_start(8)
        .css_classes(vec!["circular".to_string(), "osd".to_string()])
        .build();
    let ss_next = Button::builder()
        .icon_name("go-next-symbolic")
        .valign(Align::Center).halign(Align::End).margin_end(8)
        .css_classes(vec!["circular".to_string(), "osd".to_string()])
        .build();
    ss_overlay.add_overlay(&ss_prev);
    ss_overlay.add_overlay(&ss_next);

    let sc = screenshots_carousel.clone();
    ss_prev.connect_clicked(move |_| {
        let pos = sc.position().round() as usize;
        let n = sc.n_pages() as usize;
        if n > 0 {
            let t = if pos == 0 { n - 1 } else { pos - 1 };
            sc.scroll_to(&sc.nth_page(t as u32), true);
        }
    });
    let sc = screenshots_carousel.clone();
    ss_next.connect_clicked(move |_| {
        let pos = sc.position().round() as usize;
        let n = sc.n_pages() as usize;
        if n > 0 {
            sc.scroll_to(&sc.nth_page(((pos + 1) % n) as u32), true);
        }
    });

    screenshots_box.append(&ss_overlay);
    screenshots_box.append(&screenshot_dots);
    main_box.append(&screenshots_box);

    // ── Description ───────────────────────────────────────────────────────────
    let desc_label = Label::builder()
        .label("")
        .halign(Align::Start)
        .wrap(true)
        .selectable(true)
        .build();
    main_box.append(&desc_label);

    // ── Reviews ───────────────────────────────────────────────────────────────
    let reviews_title = Label::builder()
        .label("Reviews")
        .halign(Align::Start)
        .css_classes(vec!["title-3".to_string()])
        .build();
    main_box.append(&reviews_title);

    let reviews_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .build();
    main_box.append(&reviews_box);

    let reviews_content = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .build();
    reviews_box.append(&reviews_content);

    // Pagination row (hidden until reviews load)
    let pagination_box = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .halign(Align::Center)
        .margin_top(4)
        .visible(false)
        .build();
    let prev_btn = Button::with_label("← Previous");
    let page_lbl = Label::new(Some(""));
    let next_btn = Button::with_label("Next →");
    pagination_box.append(&prev_btn);
    pagination_box.append(&page_lbl);
    pagination_box.append(&next_btn);
    reviews_box.append(&pagination_box);

    const REVIEWS_PER_PAGE: usize = 10;
    let all_reviews: Rc<RefCell<Vec<Review>>> = Rc::new(RefCell::new(Vec::new()));
    let current_page: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));

    // render_fn held exclusively by the data-load timer (strong ref).
    // Prev/next buttons use Weak to avoid a reference cycle:
    //   render_fn → prev_c/next_c (GObjects) and buttons → render_fn_c (Rc)
    let render_fn: Rc<RefCell<Box<dyn Fn()>>> = Rc::new(RefCell::new(Box::new(|| {})));

    // Prev button — Weak ref breaks the render_fn ↔ button cycle
    {
        let render_fn_weak = Rc::downgrade(&render_fn);
        let current_page_c = Rc::clone(&current_page);
        prev_btn.connect_clicked(move |_| {
            let mut p = current_page_c.borrow_mut();
            if *p > 0 {
                *p -= 1;
                drop(p);
                if let Some(rf) = render_fn_weak.upgrade() { rf.borrow()(); }
            }
        });
    }
    // Next button — Weak ref
    {
        let render_fn_weak = Rc::downgrade(&render_fn);
        let current_page_c = Rc::clone(&current_page);
        let all_reviews_c  = Rc::clone(&all_reviews);
        next_btn.connect_clicked(move |_| {
            let total = all_reviews_c.borrow().len();
            let max_page = total.saturating_sub(1) / REVIEWS_PER_PAGE;
            let mut p = current_page_c.borrow_mut();
            if *p < max_page {
                *p += 1;
                drop(p);
                if let Some(rf) = render_fn_weak.upgrade() { rf.borrow()(); }
            }
        });
    }

    // ── Shared state ──────────────────────────────────────────────────────────
    let install_state: Rc<RefCell<(String, String, bool)>> =
        Rc::new(RefCell::new((app_id.to_string(), source.to_string(), false)));

    let addons_store: Rc<RefCell<Vec<serde_json::Value>>> =
        Rc::new(RefCell::new(Vec::new()));

    // ── Install button ────────────────────────────────────────────────────────
    let state_c = Rc::clone(&install_state);
    install_btn.connect_clicked(move |btn| {
        let (id, src, installed) = state_c.borrow().clone();
        btn.set_sensitive(false);
        btn.set_label(if installed { "Removing…" } else { "Installing…" });

        let (tx, rx) = mpsc::channel::<bool>();
        std::thread::spawn(move || {
            let ok = if installed {
                if src == "flatpak" {
                    rakuos_flatpak::uninstall_stream(&id).any(|l| l.starts_with("__done__0"))
                } else {
                    rakuos_packages::remove_stream(&id).any(|l| l.starts_with("__done__0"))
                }
            } else if src == "flatpak" {
                rakuos_flatpak::install_stream(&id).any(|l| l.starts_with("__done__0"))
            } else {
                rakuos_packages::install_stream(&id).any(|l| l.starts_with("__done__0"))
            };
            let _ = tx.send(ok);
        });

        let btn_c = btn.clone();
        let state_cc = Rc::clone(&state_c);
        // Not tracked: this timer is short-lived per-click and self-terminates.
        glib::timeout_add_local(Duration::from_millis(50), move || {
            match rx.try_recv() {
                Ok(ok) => {
                    btn_c.set_sensitive(true);
                    if ok {
                        let mut s = state_cc.borrow_mut();
                        s.2 = !s.2;
                        let now = s.2;
                        drop(s);
                        apply_install_state(&btn_c, now);
                    } else {
                        apply_install_state(&btn_c, state_cc.borrow().2);
                    }
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    });

    // ── Add-ons button → modal Dialog popup ───────────────────────────────────
    let store_for_btn = Rc::clone(&addons_store);
    addons_btn.connect_clicked(move |btn| {
        let addons = store_for_btn.borrow().clone();
        if addons.is_empty() { return; }
        show_addons_dialog(btn.upcast_ref::<gtk4::Widget>(), &addons);
    });

    // ── Background data load ──────────────────────────────────────────────────
    let app_id_str = app_id.to_string();
    type DetailData = (Option<NativeApp>, Vec<Review>);
    let (tx, rx) = mpsc::channel::<DetailData>();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let app_data = rakuos_packages::get_app_by_id(&app_id_str).ok().flatten();
        let reviews = rt.block_on(rakuos_reviews::get_reviews(&app_id_str));
        let _ = tx.send((app_data, reviews));
    });

    let install_state_t = Rc::clone(&install_state);
    let screenshots_carousel_t = screenshots_carousel.clone();
    let screenshots_box_t = screenshots_box.clone();
    let addons_store_t = Rc::clone(&addons_store);

    // Tracked: force-removed by cancel_current_detail() when navigating away.
    track(glib::timeout_add_local(Duration::from_millis(80), move || {
        match rx.try_recv() {
            Ok((app_data, reviews)) => {
                if let Some(app) = app_data {
                    dev_lbl.set_label(&app.developer);
                    desc_label.set_label(&app.description);

                    if !app.version.is_empty() {
                        version_lbl.set_label(&format!("v{}", app.version));
                        version_lbl.set_visible(true);
                    }
                    if !app.license.is_empty() {
                        license_lbl.set_label(&app.license);
                        license_lbl.set_visible(true);
                    }

                    // ── Source dropdown ───────────────────────────────────────
                    if app.sources.len() > 1 {
                        let labels: Vec<String> = app.sources.iter()
                            .map(|s| format!("{}{}", s.label,
                                if s.installed { " ✓" } else { "" }))
                            .collect();
                        let str_list = gtk4::StringList::new(
                            &labels.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                        );
                        let dropdown = gtk4::DropDown::new(Some(str_list), gtk4::Expression::NONE);
                        dropdown.set_margin_bottom(4);

                        let best = if !app.sources[0].installed {
                            app.sources.iter().position(|s| s.installed).unwrap_or(0)
                        } else { 0 };
                        dropdown.set_selected(best as u32);
                        dropdown_box.append(&dropdown);
                        dropdown_box.set_visible(true);
                        source_badge.set_visible(false);

                        {
                            let src = &app.sources[best];
                            *install_state_t.borrow_mut() =
                                (src.id.clone(), src.source.clone(), src.installed);
                        }
                        apply_install_state(&install_btn, app.sources[best].installed);
                        install_btn.set_sensitive(true);

                        {
                            let src = &app.sources[best];
                            if let Ok(addons) = rakuos_packages::get_addons_for_app(&src.id, &src.source) {
                                *addons_store_t.borrow_mut() = addons;
                                addons_btn.set_visible(!addons_store_t.borrow().is_empty());
                            }
                        }

                        let sources_rc: Rc<RefCell<Vec<SourceOption>>> =
                            Rc::new(RefCell::new(app.sources.clone()));
                        let state_dd = Rc::clone(&install_state_t);
                        let btn_dd = install_btn.clone();
                        let desc_dd = desc_label.clone();
                        let ver_dd = version_lbl.clone();
                        let lic_dd = license_lbl.clone();
                        let sc_dd = screenshots_carousel_t.clone();
                        let sb_dd = screenshots_box_t.clone();
                        let abtn_dd = addons_btn.clone();
                        let store_dd = Rc::clone(&addons_store_t);
                        let srcs = Rc::clone(&sources_rc);

                        dropdown.connect_notify_local(Some("selected"), move |dd, _| {
                            let idx = dd.selected() as usize;
                            let srcs_b = srcs.borrow();
                            let Some(src) = srcs_b.get(idx) else { return };

                            *state_dd.borrow_mut() =
                                (src.id.clone(), src.source.clone(), src.installed);
                            apply_install_state(&btn_dd, src.installed);

                            if !src.description.is_empty() { desc_dd.set_label(&src.description); }
                            if !src.version.is_empty() {
                                ver_dd.set_label(&format!("v{}", src.version));
                                ver_dd.set_visible(true);
                            }
                            if !src.license.is_empty() {
                                lic_dd.set_label(&src.license);
                                lic_dd.set_visible(true);
                            }

                            // Reload screenshots for the new source
                            while let Some(ch) = sc_dd.first_child() { sc_dd.remove(&ch); }
                            if !src.screenshots.is_empty() {
                                sb_dd.set_visible(true);
                                load_screenshots(&sc_dd, &src.screenshots);
                            } else {
                                sb_dd.set_visible(false);
                            }

                            if let Ok(addons) = rakuos_packages::get_addons_for_app(&src.id, &src.source) {
                                *store_dd.borrow_mut() = addons;
                                abtn_dd.set_visible(!store_dd.borrow().is_empty());
                            }
                        });
                    } else {
                        let pkg_id = if app.source == "flatpak" {
                            app.id.clone()
                        } else {
                            app.package_name.clone()
                        };
                        *install_state_t.borrow_mut() =
                            (pkg_id, app.source.clone(), app.installed);
                        apply_install_state(&install_btn, app.installed);
                        install_btn.set_sensitive(true);

                        let addon_id = if !app.sources.is_empty() {
                            app.sources[0].id.clone()
                        } else { app.id.clone() };
                        let addon_src = if !app.sources.is_empty() {
                            app.sources[0].source.clone()
                        } else { app.source.clone() };
                        if let Ok(addons) = rakuos_packages::get_addons_for_app(&addon_id, &addon_src) {
                            *addons_store_t.borrow_mut() = addons;
                            addons_btn.set_visible(!addons_store_t.borrow().is_empty());
                        }
                    }

                    if !app.screenshots.is_empty() {
                        screenshots_box_t.set_visible(true);
                        load_screenshots(&screenshots_carousel_t, &app.screenshots);
                    }
                }

                let (avg, count) = rakuos_reviews::aggregate(&reviews);
                if count > 0 {
                    rating_lbl.set_label(&format!("★ {:.1}  ({} reviews)", avg, count));
                } else {
                    rating_lbl.set_label("No reviews yet");
                }

                *all_reviews.borrow_mut() = reviews;
                *current_page.borrow_mut() = 0;

                // Build the render closure — render_fn is the only strong Rc holder;
                // buttons use Weak so this drops immediately when the timer exits.
                let reviews_c    = Rc::clone(&all_reviews);
                let page_c       = Rc::clone(&current_page);
                let content_c    = reviews_content.clone();
                let page_lbl_c   = page_lbl.clone();
                let prev_c       = prev_btn.clone();
                let next_c       = next_btn.clone();
                let pagination_c = pagination_box.clone();
                *render_fn.borrow_mut() = Box::new(move || {
                    while let Some(child) = content_c.first_child() {
                        content_c.remove(&child);
                    }
                    let all   = reviews_c.borrow();
                    let total = all.len();
                    let page  = *page_c.borrow();
                    let start = page * REVIEWS_PER_PAGE;
                    let end   = (start + REVIEWS_PER_PAGE).min(total);
                    for review in &all[start..end] {
                        content_c.append(&build_review_row(review));
                    }
                    let total_pages = total.saturating_sub(1) / REVIEWS_PER_PAGE + 1;
                    if total > REVIEWS_PER_PAGE {
                        page_lbl_c.set_label(&format!("Page {} of {}", page + 1, total_pages));
                        prev_c.set_sensitive(page > 0);
                        next_c.set_sensitive(page + 1 < total_pages);
                        pagination_c.set_visible(true);
                    }
                });

                render_fn.borrow()();
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    }));

    toolbar.set_content(Some(&scroll));
    nav_page.set_child(Some(&toolbar));
}

// ── Add-ons dialog ────────────────────────────────────────────────────────────

fn show_addons_dialog(parent_widget: &impl IsA<gtk4::Widget>, addons: &[serde_json::Value]) {
    let dialog = Dialog::builder()
        .title("Add-ons")
        .content_width(480)
        .content_height(520)
        .build();

    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&HeaderBar::new());

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let list = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .margin_top(12).margin_bottom(12)
        .margin_start(12).margin_end(12)
        .build();
    scroll.set_child(Some(&list));

    for addon in addons.iter().take(30) {
        let name = addon["name"].as_str()
            .unwrap_or(addon["id"].as_str().unwrap_or("Unknown"));
        let summary = addon["summary"].as_str().unwrap_or("");
        let installed = addon["installed"].as_bool().unwrap_or(false);
        let addon_id  = addon["id"].as_str().unwrap_or("").to_string();
        let addon_src = addon["source"].as_str().unwrap_or("").to_string();

        let row = libadwaita::ActionRow::builder()
            .title(name)
            .subtitle(summary)
            .build();

        let btn = Button::builder()
            .label(if installed { "Remove" } else { "Install" })
            .valign(Align::Center)
            .css_classes(if installed {
                vec!["destructive-action".to_string()]
            } else {
                vec!["suggested-action".to_string()]
            })
            .build();

        btn.connect_clicked(move |b| {
            b.set_sensitive(false);
            let id  = addon_id.clone();
            let src = addon_src.clone();
            let removing = b.label().as_deref() == Some("Remove");
            b.set_label(if removing { "Removing…" } else { "Installing…" });

            let (tx, rx) = mpsc::channel::<bool>();
            std::thread::spawn(move || {
                let ok = if removing {
                    if src == "flatpak" {
                        rakuos_flatpak::uninstall_stream(&id).any(|l| l.starts_with("__done__0"))
                    } else {
                        rakuos_packages::remove_stream(&id).any(|l| l.starts_with("__done__0"))
                    }
                } else if src == "flatpak" {
                    rakuos_flatpak::install_stream(&id).any(|l| l.starts_with("__done__0"))
                } else {
                    rakuos_packages::install_stream(&id).any(|l| l.starts_with("__done__0"))
                };
                let _ = tx.send(ok);
            });

            let bc = b.clone();
            glib::timeout_add_local(Duration::from_millis(50), move || {
                match rx.try_recv() {
                    Ok(ok) => {
                        bc.set_sensitive(true);
                        if ok {
                            let now_removing = bc.label().as_deref() == Some("Removing…");
                            bc.set_label(if now_removing { "Install" } else { "Remove" });
                        } else {
                            let was = bc.label().as_deref() == Some("Removing…");
                            bc.set_label(if was { "Remove" } else { "Install" });
                        }
                        glib::ControlFlow::Break
                    }
                    Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                }
            });
        });

        row.add_suffix(&btn);
        list.append(&row);
    }

    toolbar.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar));
    dialog.present(Some(parent_widget));
}

// ── Screenshot loading ────────────────────────────────────────────────────────

fn load_screenshots(carousel: &Carousel, urls: &[String]) {
    for url in urls.iter().take(8) {
        let picture = gtk4::Picture::builder()
            .hexpand(true)
            .can_shrink(true)
            .height_request(280)
            .build();

        let (stx, srx) = mpsc::channel::<Option<Vec<u8>>>();
        let url_c = url.clone();
        std::thread::spawn(move || {
            let bytes = std::process::Command::new("curl")
                .args(["-sL", "--max-time", "15", &url_c])
                .output().ok()
                .filter(|o| o.status.success())
                .map(|o| o.stdout);
            let _ = stx.send(bytes);
        });

        let pic_c = picture.clone();
        // Tracked: force-removed by cancel_current_detail() on page exit.
        track(glib::timeout_add_local(Duration::from_millis(100), move || {
            match srx.try_recv() {
                Ok(Some(b)) => {
                    let loader = gtk4::gdk_pixbuf::PixbufLoader::new();
                    if loader.write(&b).is_ok() && loader.close().is_ok() {
                        if let Some(pb) = loader.pixbuf() {
                            let texture = gtk4::gdk::Texture::for_pixbuf(&pb);
                            pic_c.set_paintable(Some(&texture));
                        }
                    }
                    glib::ControlFlow::Break
                }
                Ok(None) => glib::ControlFlow::Break,
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        }));

        carousel.append(&picture);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn apply_install_state(btn: &Button, installed: bool) {
    if installed {
        btn.set_label("Uninstall");
        btn.remove_css_class("suggested-action");
        btn.add_css_class("destructive-action");
    } else {
        btn.set_label("Install");
        btn.remove_css_class("destructive-action");
        btn.add_css_class("suggested-action");
    }
}

fn source_label(source: &str) -> &'static str {
    match source {
        "flatpak"  => "Flatpak",
        "appimage" => "AppImage",
        "webapp"   => "Web App",
        _          => "RakuOS Linux",
    }
}

fn build_review_row(review: &Review) -> Widget {
    let row_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .css_classes(vec!["card".to_string()])
        .margin_top(4)
        .margin_bottom(4)
        .build();

    let stars_n = ((review.rating as f32 / 20.0).round() as usize).clamp(1, 5);
    let stars: String = "★".repeat(stars_n) + &"☆".repeat(5 - stars_n);

    let header = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    header.append(&Label::builder()
        .label(&stars)
        .css_classes(vec!["caption".to_string()])
        .build());
    header.append(&Label::builder()
        .label(review.user_display.as_deref().unwrap_or("Anonymous"))
        .css_classes(vec!["caption-heading".to_string()])
        .hexpand(true)
        .halign(Align::Start)
        .build());
    row_box.append(&header);

    if !review.summary.is_empty() {
        row_box.append(&Label::builder()
            .label(&review.summary)
            .halign(Align::Start)
            .css_classes(vec!["heading".to_string()])
            .margin_start(12).margin_end(12)
            .build());
    }
    if !review.description.is_empty() {
        row_box.append(&Label::builder()
            .label(&review.description)
            .halign(Align::Start)
            .wrap(true)
            .margin_start(12).margin_end(12).margin_bottom(8)
            .build());
    }

    row_box.upcast()
}
