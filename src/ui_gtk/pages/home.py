"""
ui_gtk/pages/home.py — Home page for GTK frontend.

Featured carousel + category grid + Editor's Choice row.
Mirrors GNOME Software's home page layout.
"""
import os
import threading
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, GLib, GdkPixbuf, Pango

from backend import packages as _pkg, home as _home
from ..icon_loader import resolve_icon_path


# (label, appstream_cat, icon_name, css_class)
CATEGORIES = [
    ("Create",    "Graphics",    "applications-graphics-symbolic",      "cat-create"),
    ("Work",      "Office",      "applications-office-symbolic",        "cat-work"),
    ("Play",      "Game",        "applications-games-symbolic",         "cat-play"),
    ("Socialise", "Network",     "applications-internet-symbolic",      "cat-social"),
    ("Learn",     "Education",   "applications-science-symbolic",       "cat-learn"),
    ("Develop",   "Development", "applications-development-symbolic",   "cat-develop"),
]

CAT_COLORS = {
    "cat-create":  "#9c27b0",
    "cat-work":    "#1976d2",
    "cat-play":    "#7b1fa2",
    "cat-social":  "#e91e63",
    "cat-learn":   "#2e7d32",
    "cat-develop": "#424242",
}


class HomePage(Gtk.ScrolledWindow):
    def __init__(self, window):
        super().__init__()
        self._win = window
        self.set_vexpand(True)
        self.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self._box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=24)
        self._box.set_margin_bottom(24)
        self.set_child(self._box)

        self._build_featured()
        self._build_categories()
        self._build_editors_choice()

        GLib.idle_add(self._load_async)

    # ── Featured carousel ─────────────────────────────────────────────────────

    def _build_featured(self):
        self._carousel = Adw.Carousel()
        self._carousel.set_allow_scroll_wheel(True)
        self._carousel.set_hexpand(True)
        self._carousel.set_size_request(-1, 220)

        dots = Adw.CarouselIndicatorDots()
        dots.set_carousel(self._carousel)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box.append(self._carousel)
        box.append(dots)
        self._box.append(box)

        # Placeholder cards while loading
        for _ in range(3):
            card = self._make_placeholder_card()
            self._carousel.append(card)

    def _make_placeholder_card(self):
        card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL,
                       spacing=8, halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER)
        card.set_size_request(860, 200)
        spinner = Gtk.Spinner()
        spinner.start()
        card.append(spinner)
        return card

    def _make_featured_card(self, app: dict) -> Gtk.Widget:
        """Build a single featured app card for the carousel."""
        frame = Gtk.Frame()
        frame.add_css_class("card")
        frame.set_hexpand(True)

        overlay = Gtk.Overlay()
        frame.set_child(overlay)

        # Background color block
        bg = Gtk.Box()
        bg.set_size_request(-1, 200)
        bg.add_css_class("featured-bg")
        overlay.set_child(bg)

        # Content
        content = Gtk.Box(orientation=Gtk.Orientation.VERTICAL,
                          spacing=8, halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER)
        content.set_margin_top(24)
        content.set_margin_bottom(24)

        # Icon
        icon_img = Gtk.Image()
        icon_img.set_pixel_size(80)
        icon_path = resolve_icon_path(app)
        if icon_path and os.path.exists(icon_path):
            icon_img.set_from_file(icon_path)
        else:
            icon_img.set_from_icon_name(app.get("icon", "application-x-executable-symbolic"))

        name_lbl = Gtk.Label(label=app.get("name", ""))
        name_lbl.add_css_class("title-2")
        name_lbl.set_ellipsize(Pango.EllipsizeMode.END)

        summary_lbl = Gtk.Label(label=app.get("summary", ""))
        summary_lbl.add_css_class("body")
        summary_lbl.set_ellipsize(Pango.EllipsizeMode.END)

        content.append(icon_img)
        content.append(name_lbl)
        content.append(summary_lbl)
        overlay.add_overlay(content)

        # Click gesture
        gesture = Gtk.GestureClick()
        gesture.connect("released", lambda *_: self._win.navigate_to("detail", app=app))
        frame.add_controller(gesture)
        frame.set_cursor_from_name("pointer")

        return frame

    # ── Category grid ─────────────────────────────────────────────────────────

    def _build_categories(self):
        lbl = Gtk.Label(label="Browse by Category")
        lbl.add_css_class("title-2")
        lbl.set_halign(Gtk.Align.START)
        lbl.set_margin_start(12)
        self._box.append(lbl)

        grid = Gtk.FlowBox()
        grid.set_homogeneous(True)
        grid.set_min_children_per_line(2)
        grid.set_max_children_per_line(3)
        grid.set_column_spacing(12)
        grid.set_row_spacing(12)
        grid.set_margin_start(12)
        grid.set_margin_end(12)
        grid.set_selection_mode(Gtk.SelectionMode.NONE)

        for label, cat, icon, css in CATEGORIES:
            btn = self._make_cat_button(label, cat, icon, css)
            grid.append(btn)

        self._box.append(grid)

    def _make_cat_button(self, label: str, cat: str, icon: str, css: str) -> Gtk.Widget:
        btn = Gtk.Button()
        btn.add_css_class("suggested-action")
        btn.add_css_class("cat-button")
        btn.set_hexpand(True)

        color = CAT_COLORS.get(css, "#555")
        btn.set_css_classes(["cat-button"])

        inner = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        inner.set_margin_top(16)
        inner.set_margin_bottom(16)
        inner.set_margin_start(16)
        inner.set_margin_end(16)

        img = Gtk.Image.new_from_icon_name(icon)
        img.set_pixel_size(32)
        lbl = Gtk.Label(label=label)
        lbl.add_css_class("title-3")

        inner.append(img)
        inner.append(lbl)
        btn.set_child(inner)

        # Inline color via CSS provider
        provider = Gtk.CssProvider()
        provider.load_from_string(f"button.cat-button {{ background-color: {color}; color: white; border-radius: 12px; }}")
        btn.get_style_context().add_provider(provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

        def _go(*, _cat=cat, _label=label):
            self._win._stack.set_visible_child_name("explore")
            self._win._explore_page._show_apps(_cat, _label, back_to_home=True)

        btn.connect("clicked", lambda *_: _go())
        return btn

    # ── Editor's Choice ───────────────────────────────────────────────────────

    def _build_editors_choice(self):
        lbl = Gtk.Label(label="Editor's Choice")
        lbl.add_css_class("title-2")
        lbl.set_halign(Gtk.Align.START)
        lbl.set_margin_start(12)
        self._box.append(lbl)

        self._editors_box = Gtk.FlowBox()
        self._editors_box.set_homogeneous(True)
        self._editors_box.set_min_children_per_line(2)
        self._editors_box.set_max_children_per_line(4)
        self._editors_box.set_column_spacing(12)
        self._editors_box.set_row_spacing(12)
        self._editors_box.set_margin_start(12)
        self._editors_box.set_margin_end(12)
        self._editors_box.set_selection_mode(Gtk.SelectionMode.NONE)
        self._box.append(self._editors_box)

    # ── Data loading ──────────────────────────────────────────────────────────

    def _load_async(self):
        def _worker():
            try:
                appstream  = _pkg.preload_appstream() or _pkg._load_appstream()
                pop_ids    = _home.get_popular()[:5]
                picks_ids  = _home.get_picks()[:8]
                # IDs are strings — look up full dicts from appstream
                featured = [appstream[i] for i in pop_ids if i in appstream][:5]
                editors  = [appstream[i] for i in picks_ids if i in appstream][:8]
                # Fall back to top-rated if lists empty
                if not featured:
                    result = _pkg.get_by_category("", limit=5)
                    featured = result.get("items", []) if isinstance(result, dict) else result[:5]
            except Exception:
                featured, editors = [], []
            GLib.idle_add(self._populate, featured, editors)

        threading.Thread(target=_worker, daemon=True).start()
        return False

    def _populate(self, featured: list, editors: list):
        # Replace placeholder carousel pages
        while self._carousel.get_n_pages() > 0:
            self._carousel.remove(self._carousel.get_nth_page(0))

        for app in featured:
            card = self._make_featured_card(app)
            self._carousel.append(card)

        if not featured:
            card = self._make_placeholder_card()
            self._carousel.append(card)

        # Editor's choice cards
        while True:
            child = self._editors_box.get_first_child()
            if child is None:
                break
            self._editors_box.remove(child)

        for app in editors:
            card = self._make_app_card(app)
            self._editors_box.append(card)

        return False

    def _make_app_card(self, app: dict) -> Gtk.Widget:
        """Small app card for Editor's Choice grid."""
        btn = Gtk.Button()
        btn.add_css_class("card")
        btn.set_hexpand(True)

        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.set_margin_top(8)
        box.set_margin_bottom(8)
        box.set_margin_start(12)
        box.set_margin_end(12)

        icon_img = Gtk.Image()
        icon_img.set_pixel_size(48)
        icon_path = resolve_icon_path(app)
        if icon_path and os.path.exists(icon_path):
            icon_img.set_from_file(icon_path)
        else:
            icon_img.set_from_icon_name(app.get("icon", "application-x-executable-symbolic"))

        text_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2,
                           valign=Gtk.Align.CENTER)
        name_lbl = Gtk.Label(label=app.get("name", ""))
        name_lbl.set_halign(Gtk.Align.START)
        name_lbl.add_css_class("body")
        name_lbl.set_ellipsize(Pango.EllipsizeMode.END)

        sum_lbl = Gtk.Label(label=app.get("summary", ""))
        sum_lbl.set_halign(Gtk.Align.START)
        sum_lbl.add_css_class("caption")
        sum_lbl.set_ellipsize(Pango.EllipsizeMode.END)

        text_box.append(name_lbl)
        text_box.append(sum_lbl)

        box.append(icon_img)
        box.append(text_box)
        btn.set_child(box)

        btn.connect("clicked", lambda *_: self._win.navigate_to("detail", app=app))
        return btn
