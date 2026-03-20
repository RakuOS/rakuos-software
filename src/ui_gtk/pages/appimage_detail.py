"""
ui_gtk/pages/appimage_detail.py — AppImage detail page for GTK frontend.
"""
import os
import threading
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, GLib, Pango

from backend import appimages as _ai, packages as _pkg
from ..icon_loader import resolve_icon_path


class AppImageDetailPage(Gtk.ScrolledWindow):
    def __init__(self, window):
        super().__init__()
        self._win = window
        self.set_vexpand(True)
        self.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self._root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.set_child(self._root)

    def load_app(self, app: dict):
        self._app = app

        while True:
            c = self._root.get_first_child()
            if c is None:
                break
            self._root.remove(c)

        # Hero
        hero = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=16)
        hero.set_margin_top(24)
        hero.set_margin_bottom(16)
        hero.set_margin_start(24)
        hero.set_margin_end(24)

        icon_img = Gtk.Image()
        icon_img.set_pixel_size(96)
        icon_path = resolve_icon_path(app)
        if icon_path and os.path.exists(icon_path):
            icon_img.set_from_file(icon_path)
        else:
            icon_img.set_from_icon_name(app.get("icon", "application-x-executable-symbolic"))
        hero.append(icon_img)

        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6,
                        valign=Gtk.Align.CENTER, hexpand=True)

        name_lbl = Gtk.Label(label=app.get("name", ""))
        name_lbl.add_css_class("title-1")
        name_lbl.set_halign(Gtk.Align.START)

        ver_lbl = Gtk.Label(label=f"Version {app.get('version', '—')}")
        ver_lbl.add_css_class("dim-label")
        ver_lbl.set_halign(Gtk.Align.START)

        self._action_btn = Gtk.Button(label="Update")
        self._action_btn.add_css_class("suggested-action")
        self._action_btn.connect("clicked", lambda *_: self._do_update())

        self._progress = Gtk.ProgressBar()
        self._progress.set_pulse_step(0.1)
        self._progress.set_visible(False)

        right.append(name_lbl)
        right.append(ver_lbl)
        right.append(self._action_btn)
        right.append(self._progress)
        hero.append(right)
        self._root.append(hero)

        # Info group
        info_group = Adw.PreferencesGroup()
        info_group.set_title("Details")
        info_group.set_margin_start(12)
        info_group.set_margin_end(12)
        info_group.set_margin_top(12)

        for title, key in [("Version", "version"), ("Path", "path"),
                            ("Update URL", "update_url")]:
            val = app.get(key, "")
            if val:
                row = Adw.ActionRow()
                row.set_title(title)
                row.set_subtitle(str(val))
                info_group.add(row)

        self._root.append(info_group)

    def _do_update(self):
        self._action_btn.set_sensitive(False)
        self._action_btn.set_label("Updating…")
        self._progress.set_visible(True)

        def _pulse():
            if self._progress.get_visible() and self._progress.get_fraction() == 0:
                self._progress.pulse()
                return True
            return False
        GLib.timeout_add(200, _pulse)

        def _worker():
            ok = _ai.update(self._app)
            GLib.idle_add(self._on_done, ok)

        threading.Thread(target=_worker, daemon=True).start()

    def _on_done(self, ok: bool):
        self._progress.set_fraction(1.0 if ok else 0.0)
        GLib.timeout_add(600, lambda: self._progress.set_visible(False) or False)
        self._action_btn.set_label("Update" if not ok else "Up to Date")
        self._action_btn.set_sensitive(not ok)
        return False
