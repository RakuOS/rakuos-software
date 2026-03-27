// pages/detail.rs — App detail page pushed onto NavigationView

use gtk4::prelude::*;
use gtk4::{glib, Align, Box as GBox, Button, Label, Orientation, ScrolledWindow, Widget};
use libadwaita::prelude::*;
use libadwaita::{Carousel, NavigationPage, NavigationView};
use std::sync::Arc;

use rakuos_packages::NativeApp;
use rakuos_reviews::Review;

use super::icon_helper::load_app_icon;

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
    let page_w = build(app_id, app_name, app_summary, icon_path, icon_url, source);
    let nav_page = NavigationPage::builder()
        .title(app_name)
        .child(&page_w)
        .build();
    nav.push(&nav_page);
}

pub fn build(
    app_id: &str,
    app_name: &str,
    app_summary: &str,
    icon_path: &str,
    icon_url: &str,
    source: &str,
) -> Widget {
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
    let hero_row = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(20)
        .build();

    let icon_w = load_app_icon(icon_path, icon_url, 96, app_name);
    hero_row.append(&icon_w);

    let meta_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .valign(Align::Center)
        .hexpand(true)
        .build();

    let name_lbl = Label::builder()
        .label(app_name)
        .halign(Align::Start)
        .css_classes(vec!["title-1".to_string()])
        .build();
    meta_box.append(&name_lbl);

    let summary_lbl = Label::builder()
        .label(app_summary)
        .halign(Align::Start)
        .wrap(true)
        .build();
    meta_box.append(&summary_lbl);

    let dev_lbl = Label::builder()
        .label("")
        .halign(Align::Start)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    meta_box.append(&dev_lbl);

    let rating_lbl = Label::builder()
        .label("★ Loading…")
        .halign(Align::Start)
        .css_classes(vec!["caption".to_string()])
        .build();
    meta_box.append(&rating_lbl);

    hero_row.append(&meta_box);

    // Install / Uninstall button
    let install_btn = Button::builder()
        .label("Install")
        .valign(Align::Center)
        .css_classes(vec!["suggested-action".to_string(), "pill".to_string()])
        .build();

    let app_id_for_btn = app_id.to_string();
    let source_for_btn = source.to_string();
    let install_btn_c = install_btn.clone();

    install_btn.connect_clicked(move |btn| {
        let pkg = app_id_for_btn.clone();
        let src = source_for_btn.clone();
        btn.set_sensitive(false);

        let currently_installed = btn.label().as_deref() == Some("Uninstall");
        btn.set_label(if currently_installed { "Removing…" } else { "Installing…" });

        let btn_c = btn.clone();
        std::thread::spawn(move || {
            let success = if currently_installed {
                if src == "flatpak" {
                    rakuos_flatpak::uninstall_stream(&pkg).any(|l| l.starts_with("__done__0"))
                } else {
                    rakuos_packages::remove_stream(&pkg).any(|l| l.starts_with("__done__0"))
                }
            } else if src == "flatpak" {
                rakuos_flatpak::install_stream(&pkg).any(|l| l.starts_with("__done__0"))
            } else {
                rakuos_packages::install_stream(&pkg).any(|l| l.starts_with("__done__0"))
            };

            glib::idle_add_once(move || {
                btn_c.set_sensitive(true);
                if success {
                    if currently_installed {
                        btn_c.set_label("Install");
                        btn_c.remove_css_class("destructive-action");
                        btn_c.add_css_class("suggested-action");
                    } else {
                        btn_c.set_label("Uninstall");
                        btn_c.remove_css_class("suggested-action");
                        btn_c.add_css_class("destructive-action");
                    }
                } else {
                    btn_c.set_label(if currently_installed { "Uninstall" } else { "Install" });
                }
            });
        });
    });

    hero_row.append(&install_btn);
    main_box.append(&hero_row);
    main_box.append(&gtk4::Separator::new(Orientation::Horizontal));

    // Screenshots section (hidden initially)
    let screenshots_label = Label::builder()
        .label("Screenshots")
        .halign(Align::Start)
        .css_classes(vec!["title-3".to_string()])
        .visible(false)
        .build();
    main_box.append(&screenshots_label);

    let screenshots_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .visible(false)
        .build();
    let screenshots_carousel = Carousel::builder()
        .allow_mouse_drag(true)
        .height_request(300)
        .build();
    let screenshot_dots = libadwaita::CarouselIndicatorDots::builder()
        .carousel(&screenshots_carousel)
        .build();

    let ss_overlay = gtk4::Overlay::new();
    ss_overlay.set_child(Some(&screenshots_carousel));

    let ss_prev = Button::builder()
        .icon_name("go-previous-symbolic")
        .valign(Align::Center)
        .halign(Align::Start)
        .margin_start(8)
        .css_classes(vec!["circular".to_string(), "osd".to_string()])
        .build();
    let ss_next = Button::builder()
        .icon_name("go-next-symbolic")
        .valign(Align::Center)
        .halign(Align::End)
        .margin_end(8)
        .css_classes(vec!["circular".to_string(), "osd".to_string()])
        .build();
    ss_overlay.add_overlay(&ss_prev);
    ss_overlay.add_overlay(&ss_next);

    let sc_prev = screenshots_carousel.clone();
    ss_prev.connect_clicked(move |_| {
        let pos = sc_prev.position() as usize;
        let n = sc_prev.n_pages() as usize;
        if n > 0 {
            let t = if pos == 0 { n - 1 } else { pos - 1 };
            if let Some(p) = sc_prev.nth_page(t as u32) {
                sc_prev.scroll_to(&p, true);
            }
        }
    });

    let sc_next = screenshots_carousel.clone();
    ss_next.connect_clicked(move |_| {
        let pos = sc_next.position() as usize;
        let n = sc_next.n_pages() as usize;
        if n > 0 {
            let t = (pos + 1) % n;
            if let Some(p) = sc_next.nth_page(t as u32) {
                sc_next.scroll_to(&p, true);
            }
        }
    });

    screenshots_box.append(&ss_overlay);
    screenshots_box.append(&screenshot_dots);
    main_box.append(&screenshots_box);

    // Description
    let desc_label = Label::builder()
        .label("")
        .halign(Align::Start)
        .wrap(true)
        .selectable(true)
        .build();
    main_box.append(&desc_label);

    // Reviews section
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

    // Load data in background
    let app_id_str = app_id.to_string();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let app_data = rakuos_packages::get_app_by_id(&app_id_str).ok().flatten();
        let reviews = rt.block_on(rakuos_reviews::get_reviews(&app_id_str));

        glib::idle_add_once(move || {
            if let Some(app) = app_data {
                dev_lbl.set_label(&app.developer);
                desc_label.set_label(&app.description);

                install_btn_c.set_sensitive(true);
                if app.installed {
                    install_btn_c.set_label("Uninstall");
                    install_btn_c.remove_css_class("suggested-action");
                    install_btn_c.add_css_class("destructive-action");
                } else {
                    install_btn_c.set_label("Install");
                }

                if !app.screenshots.is_empty() {
                    screenshots_label.set_visible(true);
                    screenshots_box.set_visible(true);
                    for url in app.screenshots {
                        let img = gtk4::Image::builder()
                            .icon_name("image-missing")
                            .pixel_size(240)
                            .build();

                        let img_c = img.clone();
                        std::thread::spawn(move || {
                            let bytes = std::process::Command::new("curl")
                                .args(["-sL", "--max-time", "15", &url])
                                .output()
                                .ok()
                                .filter(|o| o.status.success())
                                .map(|o| o.stdout);
                            if let Some(b) = bytes {
                                glib::idle_add_once(move || {
                                    let loader = gtk4::gdk_pixbuf::PixbufLoader::new();
                                    if loader.write(&b).is_ok() && loader.close().is_ok() {
                                        if let Some(pb) = loader.pixbuf() {
                                            img_c.set_from_pixbuf(Some(&pb));
                                        }
                                    }
                                });
                            }
                        });

                        screenshots_carousel.append(&img);
                    }
                }
            }

            let (avg, count) = rakuos_reviews::aggregate(&reviews);
            if count > 0 {
                rating_lbl.set_label(&format!("★ {:.1}  ({} reviews)", avg, count));
            } else {
                rating_lbl.set_label("No reviews yet");
            }

            for review in reviews.iter().take(10) {
                let row = build_review_row(review);
                reviews_box.append(&row);
            }
        });
    });

    scroll.upcast()
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
    let stars: String = "★".repeat(stars_n);
    let header = GBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let rating_l = Label::builder()
        .label(&stars)
        .css_classes(vec!["caption".to_string()])
        .build();

    let user = review
        .user_display
        .clone()
        .unwrap_or_else(|| "Anonymous".to_string());
    let user_l = Label::builder()
        .label(&user)
        .css_classes(vec!["caption-heading".to_string()])
        .hexpand(true)
        .halign(Align::Start)
        .build();

    header.append(&rating_l);
    header.append(&user_l);
    row_box.append(&header);

    if !review.summary.is_empty() {
        let summary_l = Label::builder()
            .label(&review.summary)
            .halign(Align::Start)
            .css_classes(vec!["heading".to_string()])
            .margin_start(12)
            .margin_end(12)
            .build();
        row_box.append(&summary_l);
    }

    if !review.description.is_empty() {
        let desc_l = Label::builder()
            .label(&review.description)
            .halign(Align::Start)
            .wrap(true)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(8)
            .build();
        row_box.append(&desc_l);
    }

    row_box.upcast()
}
