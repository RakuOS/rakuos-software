// icon_helper.rs — shared helper for loading app icons

use gtk4::prelude::*;
use gtk4::{gdk_pixbuf, glib, Image, Widget};
use libadwaita::prelude::*;
use libadwaita::Avatar;
use std::path::Path;

/// Load an app icon as a GTK Widget.
/// Priority: local file → URL-based async load → Avatar with initials.
pub fn load_app_icon(icon_path: &str, icon_url: &str, size: i32, name: &str) -> Widget {
    // 1. Try local file path
    if !icon_path.is_empty() && Path::new(icon_path).exists() {
        let img = Image::builder()
            .file(icon_path)
            .pixel_size(size)
            .build();
        img.add_css_class("app-icon");
        return img.upcast();
    }

    // 2. Try loading from URL asynchronously
    if !icon_url.is_empty() {
        let img = Image::builder()
            .icon_name("application-x-executable")
            .pixel_size(size)
            .build();
        img.add_css_class("app-icon");

        let img_c = img.clone();
        let url = icon_url.to_string();
        let size_px = size;

        std::thread::spawn(move || {
            if let Some(bytes) = fetch_bytes_sync(&url) {
                glib::idle_add_once(move || {
                    let loader = gdk_pixbuf::PixbufLoader::new();
                    if loader.write(&bytes).is_ok() && loader.close().is_ok() {
                        if let Some(pb) = loader.pixbuf() {
                            if let Some(scaled) = pb.scale_simple(
                                size_px,
                                size_px,
                                gdk_pixbuf::InterpType::Bilinear,
                            ) {
                                img_c.set_from_pixbuf(Some(&scaled));
                            }
                        }
                    }
                });
            }
        });

        return img.upcast();
    }

    // 3. Fallback: Avatar with initials
    let initials: String = name
        .split_whitespace()
        .take(2)
        .filter_map(|w| w.chars().next())
        .collect();
    let display = if initials.is_empty() {
        name.to_string()
    } else {
        initials
    };
    let avatar = Avatar::builder()
        .text(&display)
        .size(size)
        .show_initials(true)
        .build();
    avatar.upcast()
}

fn fetch_bytes_sync(url: &str) -> Option<Vec<u8>> {
    let out = std::process::Command::new("curl")
        .args(["-sL", "--max-time", "10", url])
        .output()
        .ok()?;
    if out.status.success() && !out.stdout.is_empty() {
        Some(out.stdout)
    } else {
        None
    }
}

/// Map a category name to a CSS color string for UI accents.
pub fn category_color(category: &str) -> &'static str {
    match category.to_lowercase().as_str() {
        "audiovideo" | "audio" | "video" | "audio & video" => "#e5a50a",
        "game" | "games" => "#9141ac",
        "graphics" => "#1c71d8",
        "network" | "internet" => "#2ec27e",
        "office" => "#e66100",
        "science" | "education" | "science & education" => "#1a5fb4",
        "system" => "#c01c28",
        "development" | "development tools" => "#613583",
        "accessories" | "utility" => "#26a269",
        _ => "#5e5c64",
    }
}
