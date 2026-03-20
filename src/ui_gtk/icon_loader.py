"""
ui_gtk/icon_loader.py — Resolves app icon to a file path for the GTK frontend.

Returns a file path string if found, or None (caller falls back to icon name).
"""
import os
from backend import packages as _pkg

FLATPAK_ICON_DIRS = [
    "/var/lib/flatpak/appstream/flathub/x86_64/active/icons/128x128",
    "/var/lib/flatpak/appstream/flathub/x86_64/active/icons/64x64",
    "/var/lib/flatpak/appstream/fedora-flatpaks/x86_64/active/icons/64x64",
]


def resolve_icon_path(app: dict) -> str | None:
    """Return a file path for the app icon, or None if not found."""
    icon_id = app.get("id", "") or app.get("appstream_id", "")
    icon    = app.get("icon", "")
    stem    = os.path.splitext(os.path.basename(icon))[0] if icon else ""
    short   = icon_id.split(".")[-1].lower() if icon_id else ""

    # 1. icon_path already resolved (set by catalog loader)
    pre = app.get("icon_path", "")
    if pre and os.path.exists(pre):
        return pre

    # 2. Flathub cached icon
    if icon_id:
        cached = _pkg.get_cached_icon_path(icon_id)
        if cached and os.path.exists(cached):
            return cached

    # 3. AppStream swcatalog icons
    for candidate in ([stem] if stem else []) + ([short] if short else []):
        for suffix in ("", ".png", ".svg"):
            path = _pkg.find_icon(candidate + suffix)
            if path and os.path.exists(path):
                return path

    # 4. Flatpak local icon cache
    for candidate in ([icon_id] if icon_id else []) + ([stem] if stem else []):
        for d in FLATPAK_ICON_DIRS:
            for ext in (".png", ".svg"):
                p = os.path.join(d, candidate + ext)
                if os.path.exists(p):
                    return p

    return None


def set_image_from_app(image_widget, app: dict, fallback_icon: str = "application-x-executable-symbolic"):
    """Set a Gtk.Image from an app dict. Uses file path if found, else icon name."""
    path = resolve_icon_path(app)
    if path:
        image_widget.set_from_file(path)
    else:
        icon = app.get("icon", fallback_icon) or fallback_icon
        image_widget.set_from_icon_name(icon)
