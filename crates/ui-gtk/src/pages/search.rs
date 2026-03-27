// pages/search.rs — Search results page

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Label, Orientation, ScrolledWindow, Widget,
};
use libadwaita::prelude::*;
use libadwaita::NavigationView;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use rakuos_packages::NativeApp;
use rakuos_webapps::WebApp;

use super::icon_helper::load_app_icon;

/// Build the search results page container.
pub fn build(_nav: Arc<NavigationView>) -> Widget {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let outer = GBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .build();
    scroll.set_child(Some(&outer));

    let empty_state = libadwaita::StatusPage::builder()
        .title("Search for Apps")
        .description("Type a name to search the app catalog")
        .icon_name("edit-find-symbolic")
        .build();
    outer.append(&empty_state);

    let list_box = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .build();
    list_box.set_visible(false);
    outer.append(&list_box);

    scroll.upcast::<Widget>()
}

/// Update the search results list with new results for `query`.
pub fn run_search(search_widget: &Widget, query: String) {
    let Some(scroll) = search_widget.downcast_ref::<ScrolledWindow>() else {
        return;
    };
    let Some(outer) = scroll.child().and_then(|c| c.downcast::<GBox>().ok()) else {
        return;
    };

    let empty_state_w = outer.first_child();
    let list_box_w = empty_state_w
        .as_ref()
        .and_then(|w| w.next_sibling());

    let Some(list_box) = list_box_w.and_then(|w| w.downcast::<gtk4::ListBox>().ok()) else {
        return;
    };
    let empty = empty_state_w.and_then(|w| w.downcast::<libadwaita::StatusPage>().ok());

    if query.trim().is_empty() {
        list_box.set_visible(false);
        if let Some(e) = &empty {
            e.set_title("Search for Apps");
            e.set_description(Some("Type a name to search the app catalog"));
            e.set_visible(true);
        }
        return;
    }

    // Clear previous results
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }
    list_box.set_visible(true);
    if let Some(e) = &empty {
        e.set_visible(false);
    }

    let q = query.clone();
    let (tx, rx) = mpsc::channel::<(Vec<NativeApp>, Vec<WebApp>)>();

    std::thread::spawn(move || {
        let packages = rakuos_packages::search(&q).unwrap_or_default();
        let webapps = rakuos_webapps::search(&q);
        let _ = tx.send((packages, webapps));
    });

    let list_c = list_box.clone();
    let empty_c = empty.clone();
    glib::timeout_add_local(Duration::from_millis(50), move || {
        match rx.try_recv() {
            Ok((packages, webapps)) => {
                while let Some(child) = list_c.first_child() {
                    list_c.remove(&child);
                }
                let total = packages.len() + webapps.len();
                if total == 0 {
                    list_c.set_visible(false);
                    if let Some(e) = &empty_c {
                        e.set_title("No Results");
                        e.set_description(Some(&format!("No apps found for \u{201c}{}\u{201d}", query)));
                        e.set_visible(true);
                    }
                } else {
                    for app in packages.iter().take(30) {
                        let row = build_result_row_native(app);
                        list_c.append(&row);
                    }
                    for app in webapps.iter().take(10) {
                        let row = build_result_row_webapp(app);
                        list_c.append(&row);
                    }
                }
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });
}

fn build_result_row_native(app: &NativeApp) -> Widget {
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
    let summary_lbl = Label::builder()
        .label(&app.summary)
        .halign(Align::Start)
        .wrap(false)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(50)
        .css_classes(vec!["caption".to_string()])
        .build();
    text.append(&name_lbl);
    text.append(&summary_lbl);
    row.append(&text);

    let source_text = match app.source.as_str() {
        "flatpak" => "Flatpak",
        "terra" => "Terra",
        _ => "RPM",
    };
    let badge = Label::builder()
        .label(source_text)
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .valign(Align::Center)
        .build();
    row.append(&badge);

    row.upcast()
}

fn build_result_row_webapp(app: &WebApp) -> Widget {
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
    let summary_lbl = Label::builder()
        .label(&app.summary)
        .halign(Align::Start)
        .wrap(false)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(50)
        .css_classes(vec!["caption".to_string()])
        .build();
    text.append(&name_lbl);
    text.append(&summary_lbl);
    row.append(&text);

    let badge = Label::builder()
        .label("Web App")
        .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
        .valign(Align::Center)
        .build();
    row.append(&badge);

    row.upcast()
}
