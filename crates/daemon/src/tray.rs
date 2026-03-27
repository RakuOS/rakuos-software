// daemon/tray.rs — System tray via D-Bus StatusNotifierItem (ksni)
// Works on KDE Plasma, GNOME (with AppIndicator extension), COSMIC.

use ksni::Tray;
use std::sync::mpsc;

use crate::DaemonMsg;

// ── Icon loading ──────────────────────────────────────────────────────────────

const BREEZE_SVG: &str = "/usr/share/icons/breeze/apps/48/system-software-update.svg";

const SIZES_AND_PNGS: &[(u32, &str)] = &[
    (48, "/usr/share/icons/AdwaitaLegacy/48x48/legacy/system-software-update.png"),
    (32, "/usr/share/icons/AdwaitaLegacy/32x32/legacy/system-software-update.png"),
    (22, "/usr/share/icons/AdwaitaLegacy/22x22/legacy/system-software-update.png"),
];

/// Returns true if the current DE uses the Breeze icon theme (KDE, COSMIC).
fn is_breeze_de() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_ascii_uppercase();
    desktop.contains("KDE") || desktop.contains("COSMIC")
}

fn render_svg_at_size(data: &[u8], size: u32) -> Option<image::RgbaImage> {
    let opt  = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(data, &opt).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
    let sx = size as f32 / tree.size().width();
    let sy = size as f32 / tree.size().height();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(sx, sy), &mut pixmap.as_mut());
    image::RgbaImage::from_raw(size, size, pixmap.data().to_vec())
}

fn rgba_to_argb32(img: image::RgbaImage) -> ksni::Icon {
    let (w, h) = img.dimensions();
    let data: Vec<u8> = img
        .into_raw()
        .chunks(4)
        .flat_map(|c| [c[3], c[0], c[1], c[2]])
        .collect();
    ksni::Icon { width: w as i32, height: h as i32, data }
}

// ── Badge drawing ─────────────────────────────────────────────────────────────

#[rustfmt::skip]
const DIGIT_FONT: [[u8; 15]; 11] = [
    [1,1,1, 1,0,1, 1,0,1, 1,0,1, 1,1,1], // 0
    [0,1,0, 1,1,0, 0,1,0, 0,1,0, 1,1,1], // 1
    [1,1,1, 0,0,1, 1,1,1, 1,0,0, 1,1,1], // 2
    [1,1,1, 0,0,1, 0,1,1, 0,0,1, 1,1,1], // 3
    [1,0,1, 1,0,1, 1,1,1, 0,0,1, 0,0,1], // 4
    [1,1,1, 1,0,0, 1,1,1, 0,0,1, 1,1,1], // 5
    [1,1,1, 1,0,0, 1,1,1, 1,0,1, 1,1,1], // 6
    [1,1,1, 0,0,1, 0,1,0, 0,1,0, 0,1,0], // 7
    [1,1,1, 1,0,1, 1,1,1, 1,0,1, 1,1,1], // 8
    [1,1,1, 1,0,1, 1,1,1, 0,0,1, 1,1,1], // 9
    [0,1,0, 0,1,0, 0,1,0, 0,0,0, 0,1,0], // !
];

fn fill_circle(img: &mut image::RgbaImage, cx: i32, cy: i32, r: i32, color: [u8; 4]) {
    let (w, h) = img.dimensions();
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                let x = cx + dx;
                let y = cy + dy;
                if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                    img.put_pixel(x as u32, y as u32, image::Rgba(color));
                }
            }
        }
    }
}

fn draw_digit(img: &mut image::RgbaImage, digit_idx: usize, cx: i32, cy: i32, color: [u8; 4]) {
    let bitmap = &DIGIT_FONT[digit_idx.min(10)];
    for row in 0..5i32 {
        for col in 0..3i32 {
            if bitmap[(row * 3 + col) as usize] == 1 {
                let x = cx - 1 + col;
                let y = cy - 2 + row;
                let (w, h) = img.dimensions();
                if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                    img.put_pixel(x as u32, y as u32, image::Rgba(color));
                }
            }
        }
    }
}

fn make_badge_icons(count: usize) -> Vec<ksni::Icon> {
    let mut icons = Vec::new();
    let use_breeze = is_breeze_de();
    let svg_data = if use_breeze { std::fs::read(BREEZE_SVG).ok() } else { None };

    for (size, png_path) in SIZES_AND_PNGS {
        let img_opt = svg_data.as_deref()
            .and_then(|data| render_svg_at_size(data, *size))
            .or_else(|| {
                let data = std::fs::read(png_path).ok()?;
                image::load_from_memory_with_format(&data, image::ImageFormat::Png)
                    .ok()
                    .map(|img| img.to_rgba8())
            });

        let Some(mut img) = img_opt else { continue };
        let (w, _) = img.dimensions();
        let r  = (w as i32 / 5).max(4);
        let cx = w as i32 - r - 1;
        let cy = r + 1;
        fill_circle(&mut img, cx, cy, r, [227, 57, 53, 255]);
        draw_digit(&mut img, if count < 10 { count } else { 10 }, cx, cy, [255, 255, 255, 255]);
        icons.push(rgba_to_argb32(img));
    }
    icons
}

// ── Tray ─────────────────────────────────────────────────────────────────────

pub struct SoftwareTray {
    pub update_count: usize,
    pub tx: mpsc::SyncSender<DaemonMsg>,
}

impl Tray for SoftwareTray {
    fn id(&self) -> String { "org.rakuos.Software".into() }

    fn category(&self) -> ksni::Category { ksni::Category::ApplicationStatus }

    fn icon_name(&self) -> String {
        if self.update_count > 0 {
            String::new() // empty → DE uses icon_pixmap (badged)
        } else {
            "system-software-update".into()
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        if self.update_count > 0 {
            make_badge_icons(self.update_count)
        } else {
            Vec::new()
        }
    }

    fn title(&self) -> String {
        if self.update_count > 0 {
            format!("RakuOS Software — {} update{} available",
                self.update_count, if self.update_count != 1 { "s" } else { "" })
        } else {
            "RakuOS Software".into()
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: "system-software-update".into(),
            icon_pixmap: Vec::new(),
            title: self.title(),
            description: String::new(),
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Open RakuOS Software".into(),
                icon_name: "system-software-install".into(),
                activate: Box::new(|t: &mut Self| { let _ = t.tx.send(DaemonMsg::OpenUi); }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "Check for Updates".into(),
                icon_name: "view-refresh".into(),
                activate: Box::new(|t: &mut Self| { let _ = t.tx.send(DaemonMsg::CheckNow); }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|t: &mut Self| { let _ = t.tx.send(DaemonMsg::Quit); }),
                ..Default::default()
            }.into(),
        ]
    }
}

pub fn spawn(tx: mpsc::SyncSender<DaemonMsg>) -> anyhow::Result<ksni::Handle<SoftwareTray>> {
    let service = ksni::TrayService::new(SoftwareTray { update_count: 0, tx });
    let handle = service.handle();
    service.spawn();
    Ok(handle)
}
