// ui-gtk/pages/home.rs

use gtk4::prelude::*;
use gtk4::Widget;
use libadwaita::prelude::*;

pub fn build() -> Widget {
    let label = gtk4::Label::new(Some("Home page — TODO"));
    label.upcast()
}
