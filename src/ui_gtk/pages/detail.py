"""
ui_gtk/pages/detail.py — App detail page for GTK frontend.

GNOME Software style: hero header, screenshots carousel, metadata chips,
description, links, reviews.
"""
import os
import re
import threading
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, GLib, Pango

import gzip
import xml.etree.ElementTree as ET
from backend import packages as _pkg, flatpak as _fp, webapps as _wa
from ..icon_loader import resolve_icon_path


class AppDetailPage(Gtk.ScrolledWindow):
    def __init__(self, window):
        super().__init__()
        self._win = window
        self.set_vexpand(True)
        self.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self._app = {}
        self._selected_app = None
        self._info_rows: list = []
        self._link_rows: list = []
        self._ss_load_total = 0
        self._ss_load_done  = 0

        # Root box
        self._root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.set_child(self._root)

    def load_app(self, app: dict):
        self._app = app
        self._selected_app = None

        # Clear previous content
        while True:
            child = self._root.get_first_child()
            if child is None:
                break
            self._root.remove(child)

        self._build_hero(app)
        self._build_screenshots_placeholder()
        self._build_meta_chips(app)
        self._build_description(app)
        self._build_info_section(app)
        self._build_links_section(app)

        # Fetch extra detail in background
        threading.Thread(
            target=self._fetch_detail, args=(app,), daemon=True).start()

    # ── Hero ──────────────────────────────────────────────────────────────────

    def _build_hero(self, app: dict):
        hero = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=16)
        hero.set_margin_top(24)
        hero.set_margin_bottom(16)
        hero.set_margin_start(24)
        hero.set_margin_end(24)

        # Icon
        self._hero_icon = Gtk.Image()
        self._hero_icon.set_pixel_size(96)
        icon_path = resolve_icon_path(app)
        if icon_path and os.path.exists(icon_path):
            self._hero_icon.set_from_file(icon_path)
        else:
            source = app.get("source", "native")
            fallback = "web-browser-symbolic" if source == "webapp" \
                       else app.get("icon", "application-x-executable-symbolic")
            self._hero_icon.set_from_icon_name(fallback)
        hero.append(self._hero_icon)

        # Right side: name, developer, summary, action row
        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6,
                        valign=Gtk.Align.CENTER, hexpand=True)

        name_lbl = Gtk.Label(label=app.get("name", ""))
        name_lbl.add_css_class("title-1")
        name_lbl.set_halign(Gtk.Align.START)
        name_lbl.set_ellipsize(Pango.EllipsizeMode.END)

        dev_lbl = Gtk.Label(label=app.get("developer", app.get("author", "")))
        dev_lbl.add_css_class("dim-label")
        dev_lbl.set_halign(Gtk.Align.START)

        sum_lbl = Gtk.Label(label=app.get("summary", ""))
        sum_lbl.set_halign(Gtk.Align.START)
        sum_lbl.set_ellipsize(Pango.EllipsizeMode.END)
        sum_lbl.set_wrap(True)
        sum_lbl.set_max_width_chars(50)

        # Star rating
        self._star_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        self._star_box.set_halign(Gtk.Align.START)
        self._build_stars(app)

        # Action row: Install/Remove button + source dropdown
        self._action_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self._action_row.set_halign(Gtk.Align.START)
        self._build_action_row(app)

        # Fixed-height container for meta/link buttons (prevents layout jump)
        self._meta_container = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL,
                                       spacing=6)
        self._meta_container.set_size_request(-1, 36)

        right.append(name_lbl)
        right.append(dev_lbl)
        right.append(sum_lbl)
        right.append(self._star_box)
        right.append(self._action_row)
        right.append(self._meta_container)
        hero.append(right)

        self._root.append(hero)

        sep = Gtk.Separator(orientation=Gtk.Orientation.HORIZONTAL)
        self._root.append(sep)

    def _build_stars(self, app: dict):
        while True:
            child = self._star_box.get_first_child()
            if child is None:
                break
            self._star_box.remove(child)

        rating = app.get("rating", 0)
        count  = app.get("review_count", 0)
        if not rating:
            return

        stars_lbl = Gtk.Label(label=_star_str(rating))
        stars_lbl.add_css_class("body")
        count_lbl = Gtk.Label(label=f"({count})" if count else "")
        count_lbl.add_css_class("caption")
        count_lbl.add_css_class("dim-label")
        self._star_box.append(stars_lbl)
        self._star_box.append(count_lbl)

    def _build_action_row(self, app: dict):
        while True:
            child = self._action_row.get_first_child()
            if child is None:
                break
            self._action_row.remove(child)

        pkg_name = app.get("pkg_name", "")
        app_id   = app.get("id", "")
        source   = app.get("source", "native")

        if source == "webapp":
            installed = app.get("installed", False) or _wa.is_installed(app_id)
        else:
            installed = (bool(pkg_name) and _pkg.is_installed_native(pkg_name)) or \
                        (bool(app_id)   and _pkg.is_installed_flatpak(app_id))

        if installed:
            btn = Gtk.Button(label="Remove" if source != "webapp" else "Remove Web App")
            btn.add_css_class("destructive-action")
            btn.connect("clicked", lambda *_: self._do_uninstall())
        else:
            btn = Gtk.Button(label="Install" if source != "webapp" else "Add Web App")
            btn.add_css_class("suggested-action")
            btn.connect("clicked", lambda *_: self._do_install())
        self._install_btn = btn
        self._action_row.append(btn)

        # Source dropdown: only for native/flatpak, not webapps
        sources = [] if source == "webapp" else _build_sources(app)
        if len(sources) > 1:
            dd = Gtk.DropDown.new_from_strings([s.get("label", s.get("id", "")) for s in sources])
            dd.connect("notify::selected", lambda d, _: self._on_source_changed(d, sources))
            self._action_row.append(dd)
            self._source_dd = dd
            self._sources   = sources
        else:
            self._source_dd = None
            self._sources   = sources

    def _on_source_changed(self, dd, sources):
        idx = dd.get_selected()
        if 0 <= idx < len(sources):
            self._selected_app = sources[idx]
            self._refresh_info()

    # ── Screenshots ───────────────────────────────────────────────────────────

    def _build_screenshots_placeholder(self):
        self._screenshots_carousel = Adw.Carousel()
        self._screenshots_carousel.set_size_request(-1, 380)
        self._screenshots_carousel.set_hexpand(True)
        self._screenshots_carousel.set_visible(False)
        self._screenshots_carousel.set_allow_scroll_wheel(True)

        # Overlay arrows on top of carousel
        overlay = Gtk.Overlay()
        overlay.set_child(self._screenshots_carousel)

        self._ss_prev_btn = Gtk.Button()
        self._ss_prev_btn.set_icon_name("go-previous-symbolic")
        self._ss_prev_btn.add_css_class("circular")
        self._ss_prev_btn.add_css_class("osd")
        self._ss_prev_btn.set_halign(Gtk.Align.START)
        self._ss_prev_btn.set_valign(Gtk.Align.CENTER)
        self._ss_prev_btn.set_margin_start(8)
        self._ss_prev_btn.set_visible(False)
        self._ss_prev_btn.connect("clicked", self._on_screenshot_prev)
        overlay.add_overlay(self._ss_prev_btn)

        self._ss_next_btn = Gtk.Button()
        self._ss_next_btn.set_icon_name("go-next-symbolic")
        self._ss_next_btn.add_css_class("circular")
        self._ss_next_btn.add_css_class("osd")
        self._ss_next_btn.set_halign(Gtk.Align.END)
        self._ss_next_btn.set_valign(Gtk.Align.CENTER)
        self._ss_next_btn.set_margin_end(8)
        self._ss_next_btn.set_visible(False)
        self._ss_next_btn.connect("clicked", self._on_screenshot_next)
        overlay.add_overlay(self._ss_next_btn)

        dots = Adw.CarouselIndicatorDots()
        dots.set_carousel(self._screenshots_carousel)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box.append(overlay)
        box.append(dots)
        self._root.append(box)

    def _on_screenshot_prev(self, _btn):
        n = self._screenshots_carousel.get_n_pages()
        if n == 0:
            return
        pos = int(self._screenshots_carousel.get_position())
        target = max(0, pos - 1)
        self._screenshots_carousel.scroll_to(
            self._screenshots_carousel.get_nth_page(target), True)

    def _on_screenshot_next(self, _btn):
        n = self._screenshots_carousel.get_n_pages()
        if n == 0:
            return
        pos = int(self._screenshots_carousel.get_position())
        target = min(n - 1, pos + 1)
        self._screenshots_carousel.scroll_to(
            self._screenshots_carousel.get_nth_page(target), True)

    # ── Meta chips (download size, safety, etc.) ──────────────────────────────

    def _build_meta_chips(self, app: dict):
        self._chips_box = Gtk.FlowBox()
        self._chips_box.set_selection_mode(Gtk.SelectionMode.NONE)
        self._chips_box.set_margin_start(24)
        self._chips_box.set_margin_end(24)
        self._chips_box.set_margin_top(12)
        self._chips_box.set_margin_bottom(12)
        self._root.append(self._chips_box)
        self._populate_chips(app)

    def _populate_chips(self, app: dict):
        while True:
            child = self._chips_box.get_first_child()
            if child is None:
                break
            self._chips_box.remove(child)

        size = app.get("size") or app.get("download_size")
        if size:
            self._chips_box.append(self._make_chip("folder-download-symbolic",
                                                    _fmt_size(size), "Download Size"))
        if app.get("safe") is not False:
            self._chips_box.append(self._make_chip("security-medium-symbolic",
                                                    "Probably Safe", "Safety"))
        form = app.get("form_factor", "desktop")
        self._chips_box.append(self._make_chip("computer-symbolic",
                                                form.capitalize(), "Platform"))
        age = app.get("age_rating", "All")
        self._chips_box.append(self._make_chip("preferences-system-parental-controls-symbolic",
                                                age, "Age Rating"))

    def _make_chip(self, icon: str, label: str, subtitle: str) -> Gtk.Widget:
        frame = Gtk.Frame()
        frame.add_css_class("card")

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4,
                      halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER)
        box.set_margin_top(12)
        box.set_margin_bottom(12)
        box.set_margin_start(16)
        box.set_margin_end(16)

        img = Gtk.Image.new_from_icon_name(icon)
        img.set_pixel_size(24)
        img.add_css_class("accent")

        val_lbl = Gtk.Label(label=label)
        val_lbl.add_css_class("caption-heading")

        sub_lbl = Gtk.Label(label=subtitle)
        sub_lbl.add_css_class("caption")
        sub_lbl.add_css_class("dim-label")

        box.append(img)
        box.append(val_lbl)
        box.append(sub_lbl)
        frame.set_child(box)
        return frame

    # ── Description ───────────────────────────────────────────────────────────

    def _build_description(self, app: dict):
        desc = app.get("description", "")
        if not desc:
            return

        lbl = Gtk.Label(label=_strip_html(desc))
        lbl.set_wrap(True)
        lbl.set_xalign(0)
        lbl.set_halign(Gtk.Align.FILL)
        lbl.set_margin_start(24)
        lbl.set_margin_end(24)
        lbl.set_margin_top(8)
        lbl.set_margin_bottom(12)
        self._root.append(lbl)

    # ── Info section (version/size/license) ───────────────────────────────────

    def _build_info_section(self, app: dict):
        self._info_group = Adw.PreferencesGroup()
        self._info_group.set_title("Details")
        self._info_group.set_margin_start(12)
        self._info_group.set_margin_end(12)
        self._info_group.set_margin_top(12)
        self._root.append(self._info_group)
        self._info_rows: list = []
        self._refresh_info_from(app)

    def _refresh_info(self):
        base = self._selected_app or self._app
        self._refresh_info_from(base)

    def _refresh_info_from(self, app: dict):
        for row in self._info_rows:
            self._info_group.remove(row)
        self._info_rows.clear()

        version = app.get("version", "")
        size    = app.get("size") or app.get("download_size")
        license_ = app.get("license", "")

        if version:
            row = Adw.ActionRow()
            row.set_title("Version")
            row.set_subtitle(version)
            self._info_group.add(row)
            self._info_rows.append(row)

        if size:
            row = Adw.ActionRow()
            row.set_title("Download Size")
            row.set_subtitle(_fmt_size(size))
            self._info_group.add(row)
            self._info_rows.append(row)

        if license_:
            short_lic = license_.split(" and ")[0].split("(")[0].strip()
            row = Adw.ActionRow()
            row.set_title("License")
            row.set_subtitle(short_lic)
            self._info_group.add(row)
            self._info_rows.append(row)

    # ── Links section ─────────────────────────────────────────────────────────

    def _build_links_section(self, app: dict):
        self._links_group = Adw.PreferencesGroup()
        self._links_group.set_title("Links")
        self._links_group.set_margin_start(12)
        self._links_group.set_margin_end(12)
        self._links_group.set_margin_top(8)
        self._links_group.set_margin_bottom(24)
        self._root.append(self._links_group)
        self._link_rows: list = []

    def _populate_links(self, urls: dict):
        for row in self._link_rows:
            self._links_group.remove(row)
        self._link_rows.clear()

        link_defs = [
            ("homepage",    "Project Website",        "globe-symbolic"),
            ("donation",    "Donate",                 "emblem-favorite-symbolic"),
            ("translate",   "Contribute Translations","language-symbolic"),
            ("bugtracker",  "Report an Issue",        "dialog-warning-symbolic"),
            ("help",        "Help & Documentation",   "help-browser-symbolic"),
        ]
        for key, label, icon in link_defs:
            url = urls.get(key)
            if not url:
                continue
            row = Adw.ActionRow()
            row.set_title(label)
            row.set_subtitle(url)
            row.set_activatable(True)
            row.add_prefix(Gtk.Image.new_from_icon_name(icon))
            row.add_suffix(Gtk.Image.new_from_icon_name("arrow-pointing-at-line-out-from-bottom-right-symbolic"))
            row.connect("activated", lambda r, u=url: _open_url(u))
            self._links_group.add(row)
            self._link_rows.append(row)

        # Populate meta container buttons
        while True:
            child = self._meta_container.get_first_child()
            if child is None:
                break
            self._meta_container.remove(child)

        for key, label, icon in link_defs[:3]:
            url = urls.get(key)
            if not url:
                continue
            btn = Gtk.Button(label=label)
            btn.add_css_class("flat")
            btn.set_icon_name(icon)
            btn.connect("clicked", lambda b, u=url: _open_url(u))
            self._meta_container.append(btn)

    # ── Async detail fetch ────────────────────────────────────────────────────

    def _fetch_detail(self, app: dict):
        source = app.get("source", "native")

        if source == "webapp":
            # Webapps carry all their data in the app dict already
            url     = app.get("url") or app.get("website", "")
            urls    = {"homepage": url} if url else {}
            screenshots = app.get("screenshots", [])
            GLib.idle_add(self._on_detail, None, None, urls, screenshots)
            return

        try:
            appstream  = _pkg._load_appstream()
            name_lower = app.get("name", "").lower()
            app_id     = app.get("id", "")
            pkg_name   = app.get("pkg_name", "")
            native = fp = None

            for a in appstream.values():
                if a.get("is_addon"):
                    continue
                if a["source"] == "native" and (a["id"] == app_id or a.get("pkg_name") == pkg_name):
                    native = _pkg._enrich_installed(a)
                if a["source"] == "flatpak" and a["id"] == app_id:
                    fp = _pkg._enrich_installed(a)

            if fp and not native:
                native = _pkg.find_native_counterpart(fp)

            if native:
                native = _pkg._enrich_detail(native)
            if fp:
                fp = _pkg._enrich_detail(fp)

            urls = _fetch_urls(app_id, name_lower)
            screenshots = app.get("screenshots", [])

        except Exception:
            native, fp, urls, screenshots = None, None, {}, []

        GLib.idle_add(self._on_detail, native, fp, urls, screenshots)

    def _on_detail(self, native, fp, urls, screenshots):
        if native or fp:
            merged = {**self._app, **(native or {}), **(fp or {})}
            self._refresh_info_from(merged)
            self._populate_chips(merged)

        if urls:
            self._populate_links(urls)

        if screenshots:
            self._load_screenshots(screenshots)

        return False

    def _load_screenshots(self, screenshots: list):
        if not screenshots:
            return

        self._screenshots_carousel.set_visible(True)

        while self._screenshots_carousel.get_n_pages() > 0:
            self._screenshots_carousel.remove(self._screenshots_carousel.get_nth_page(0))

        urls = screenshots[:6]
        self._ss_load_total   = len(urls)
        self._ss_load_done    = 0

        for url in urls:
            # Placeholder with spinner while downloading
            box = Gtk.Box(halign=Gtk.Align.FILL, valign=Gtk.Align.FILL)
            box.set_hexpand(True)
            box.set_size_request(700, 360)
            spinner = Gtk.Spinner()
            spinner.start()
            spinner.set_halign(Gtk.Align.CENTER)
            spinner.set_valign(Gtk.Align.CENTER)
            spinner.set_hexpand(True)
            box.append(spinner)
            self._screenshots_carousel.append(box)
            threading.Thread(
                target=self._load_screenshot_img,
                args=(box, url), daemon=True).start()

    def _load_screenshot_img(self, container: Gtk.Box, url: str):
        try:
            import urllib.request, tempfile
            with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
                urllib.request.urlretrieve(url, f.name)
                path = f.name

            def _set(path=path, container=container):
                child = container.get_first_child()
                if child:
                    container.remove(child)
                pic = Gtk.Picture.new_for_filename(path)
                pic.set_content_fit(Gtk.ContentFit.CONTAIN)
                pic.set_hexpand(True)
                pic.set_vexpand(True)
                pic.set_size_request(700, 360)
                container.append(pic)
                # Track completion and reveal arrows when all loaded
                self._ss_load_done += 1
                if self._ss_load_done >= self._ss_load_total:
                    show = self._ss_load_total > 1
                    self._ss_prev_btn.set_visible(show)
                    self._ss_next_btn.set_visible(show)
                return False

            GLib.idle_add(_set)
        except Exception:
            pass

    # ── Install / Remove ──────────────────────────────────────────────────────

    def _do_install(self):
        app = self._selected_app or self._app
        source = app.get("source", "native")
        self._install_btn.set_label("Adding…" if source == "webapp" else "Installing…")
        self._install_btn.set_sensitive(False)

        def _worker():
            ok = True
            try:
                if source == "webapp":
                    ok, _ = _wa.install(app.get("id", ""))
                elif source == "flatpak":
                    app_id = app.get("id", "")
                    origin = app.get("origin", "flathub")
                    for _ in _fp.install_flatpak_stream(app_id, origin):
                        pass
                else:
                    pkg_name = app.get("pkg_name", app.get("name", ""))
                    for _ in _pkg.install_package_stream(pkg_name):
                        pass
            except Exception:
                ok = False
            GLib.idle_add(self._on_install_done, ok, "")

        threading.Thread(target=_worker, daemon=True).start()

    def _on_install_done(self, ok: bool, msg: str):
        app = self._selected_app or self._app
        source = app.get("source", "native")
        if ok:
            label = "Remove Web App" if source == "webapp" else "Remove"
            self._install_btn.set_label(label)
            self._install_btn.set_css_classes(["destructive-action"])
            self._install_btn.connect("clicked", lambda *_: self._do_uninstall())
        else:
            label = "Add Web App" if source == "webapp" else "Install"
            self._install_btn.set_label(label)
            dlg = Adw.MessageDialog(heading="Install Failed",
                                    body=msg or "An error occurred.")
            dlg.add_response("ok", "OK")
            dlg.set_transient_for(self._win)
            dlg.present()
        self._install_btn.set_sensitive(True)
        return False

    def _do_uninstall(self):
        app = self._selected_app or self._app
        dlg = Adw.MessageDialog(
            heading=f"Remove {app.get('name', 'this app')}?",
            body="This will uninstall the application.")
        dlg.add_response("cancel", "Cancel")
        dlg.add_response("remove", "Remove")
        dlg.set_response_appearance("remove", Adw.ResponseAppearance.DESTRUCTIVE)
        dlg.connect("response", self._on_uninstall_confirm, app)
        dlg.set_transient_for(self._win)
        dlg.present()

    def _on_uninstall_confirm(self, dlg, response, app):
        if response != "remove":
            return
        self._install_btn.set_label("Removing…")
        self._install_btn.set_sensitive(False)
        source = app.get("source", "native")

        def _worker():
            ok = True
            try:
                if source == "webapp":
                    ok, _ = _wa.uninstall(app.get("id", ""))
                elif source == "flatpak":
                    app_id = app.get("id", "")
                    for _ in _fp.remove_flatpak_stream(app_id):
                        pass
                else:
                    pkg_name = app.get("pkg_name", app.get("name", ""))
                    for _ in _pkg.remove_package_stream(pkg_name):
                        pass
            except Exception:
                ok = False
            GLib.idle_add(self._on_uninstall_done, ok, "")

        threading.Thread(target=_worker, daemon=True).start()

    def _on_uninstall_done(self, ok: bool, msg: str):
        app = self._selected_app or self._app
        source = app.get("source", "native")
        if ok:
            label = "Add Web App" if source == "webapp" else "Install"
            self._install_btn.set_label(label)
            self._install_btn.set_css_classes(["suggested-action"])
            self._install_btn.connect("clicked", lambda *_: self._do_install())
        else:
            label = "Remove Web App" if source == "webapp" else "Remove"
            self._install_btn.set_label(label)
            dlg = Adw.MessageDialog(heading="Remove Failed", body=msg or "")
            dlg.add_response("ok", "OK")
            dlg.set_transient_for(self._win)
            dlg.present()
        self._install_btn.set_sensitive(True)
        return False


# ── Helpers ───────────────────────────────────────────────────────────────────

def _star_str(rating: float) -> str:
    full  = int(rating)
    half  = 1 if (rating - full) >= 0.5 else 0
    empty = 5 - full - half
    return "★" * full + "⯨" * half + "☆" * empty


def _fmt_size(size) -> str:
    try:
        n = int(size)
        if n >= 1_073_741_824:
            return f"{n/1_073_741_824:.1f} GB"
        if n >= 1_048_576:
            return f"{n/1_048_576:.1f} MB"
        if n >= 1024:
            return f"{n/1024:.1f} KB"
        return f"{n} B"
    except Exception:
        return str(size)


def _strip_html(text: str) -> str:
    return re.sub(r"<[^>]+>", "", text).strip()


def _open_url(url: str):
    import subprocess
    subprocess.Popen(["xdg-open", url])


def _fetch_urls(app_id: str, name_lower: str) -> dict:
    """Parse AppStream XML to extract URL entries for an app."""
    urls = {k: "" for k in ("homepage", "donation", "bugtracker", "help", "translate")}
    for appstream_dir, _ in _pkg.APPSTREAM_DIRS:
        if not os.path.isdir(appstream_dir):
            continue
        for fname in os.listdir(appstream_dir):
            fpath = os.path.join(appstream_dir, fname)
            try:
                if fname.endswith(".gz"):
                    fh = gzip.open(fpath, "rt", encoding="utf-8", errors="ignore")
                elif fname.endswith(".xml"):
                    fh = open(fpath, "rt", encoding="utf-8", errors="ignore")
                else:
                    continue
                root = ET.parse(fh).getroot()
                fh.close()
                comps = [root] if root.tag == "component" else root.findall("component")
                found = False
                for comp in comps:
                    cid = (comp.findtext("id") or "").strip()
                    if cid == app_id or (comp.findtext("name") or "").lower() == name_lower:
                        for u in comp.findall("url"):
                            ut = u.get("type", "")
                            if ut in urls and u.text:
                                urls[ut] = u.text.strip()
                        found = True
                        break
                if found:
                    break
            except Exception:
                continue
    return urls


def _build_sources(app: dict) -> list:
    """Build a list of install source dicts for the source dropdown."""
    sources = []
    appstream = _pkg._load_appstream()
    app_id    = app.get("id", "")
    pkg_name  = app.get("pkg_name", "")
    name_lower = app.get("name", "").lower()

    native_entry = None
    fp_entry     = None

    for a in appstream.values():
        if a.get("is_addon"):
            continue
        if a["source"] == "native" and (a["id"] == app_id or a.get("pkg_name") == pkg_name
                                         or a["name"].lower() == name_lower):
            if not native_entry:
                native_entry = dict(a)
                native_entry["label"] = "RakuOS Linux"
        if a["source"] == "flatpak" and (a["id"] == app_id or a["name"].lower() == name_lower):
            if not fp_entry:
                fp_entry = dict(a)
                fp_entry["label"] = a.get("origin", "Flathub").replace("-", " ").title()

    if native_entry:
        sources.append(native_entry)
    if fp_entry:
        sources.append(fp_entry)
    if not sources:
        sources.append(dict(app))

    return sources
