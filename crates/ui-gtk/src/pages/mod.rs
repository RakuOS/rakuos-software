pub mod home;
pub mod explore;
pub mod search;
pub mod installed;
pub mod updates;
pub mod detail;
pub mod settings;
pub mod system;

use gtk4::prelude::*;
use gtk4::Widget;
use libadwaita::prelude::*;
use libadwaita::{NavigationView, NavigationPage};

pub fn build_main_view() -> Widget {
    let nav = NavigationView::new();

    // Sidebar nav items mapped to pages
    let pages: &[(&str, &str, fn() -> Widget)] = &[
        ("home",      "Home",      || home::build()),
        ("explore",   "Explore",   || explore::build()),
        ("search",    "Search",    || search::build()),
        ("installed", "Installed", || installed::build()),
        ("updates",   "Updates",   || updates::build()),
        ("system",    "System",    || system::build()),
        ("settings",  "Settings",  || settings::build()),
    ];

    // Build NavigationPages and push home
    let home_page = NavigationPage::builder()
        .title("Home")
        .child(&home::build())
        .build();
    nav.push(&home_page);

    nav.upcast()
}
