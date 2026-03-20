"""
ui_gtk/tray.py — System tray / AppIndicator for GTK frontend.

Tries AppIndicator3 first (GNOME with AppIndicator extension, KDE, etc.).
Falls back to a silent mode (notifications only) if unavailable.
"""
import os

_indicator = None

try:
    import gi
    gi.require_version("AyatanaAppIndicator3", "0.1")
    from gi.repository import AyatanaAppIndicator3 as AppIndicator3
    _indicator = "ayatana"
except Exception:
    try:
        import gi
        gi.require_version("AppIndicator3", "0.1")
        from gi.repository import AppIndicator3
        _indicator = "appindicator"
    except Exception:
        pass

from gi.repository import Gio, GLib
import subprocess


def _send_notification(title: str, body: str):
    try:
        subprocess.Popen(["notify-send", "-a", "RakuOS Software", title, body])
    except Exception:
        pass


class RakuOSTray:
    def __init__(self, app, main_window):
        self._app  = app
        self._win  = main_window
        self._ind  = None
        self._update_count = 0

        if _indicator == "ayatana":
            from gi.repository import AyatanaAppIndicator3 as AI
            self._ind = AI.Indicator.new(
                "rakuos-software",
                "system-software-update",
                AI.IndicatorCategory.APPLICATION_STATUS)
            self._ind.set_status(AI.IndicatorStatus.ACTIVE)
            self._ind.set_menu(self._build_menu())

        elif _indicator == "appindicator":
            from gi.repository import AppIndicator3 as AI
            self._ind = AI.Indicator.new(
                "rakuos-software",
                "system-software-update",
                AI.IndicatorCategory.APPLICATION_STATUS)
            self._ind.set_status(AI.IndicatorStatus.ACTIVE)
            self._ind.set_menu(self._build_menu())

    def _build_menu(self):
        from gi.repository import Gtk
        menu = Gtk.Menu()

        show_item = Gtk.MenuItem(label="Show Software Center")
        show_item.connect("activate", lambda *_: self._show_window())
        menu.append(show_item)

        menu.append(Gtk.SeparatorMenuItem())

        self._status_item = Gtk.MenuItem(label="No updates available")
        self._status_item.set_sensitive(False)
        menu.append(self._status_item)

        menu.append(Gtk.SeparatorMenuItem())

        check_item = Gtk.MenuItem(label="Check for Updates")
        check_item.connect("activate", lambda *_: self._check_updates())
        menu.append(check_item)

        menu.append(Gtk.SeparatorMenuItem())

        quit_item = Gtk.MenuItem(label="Exit")
        quit_item.connect("activate", lambda *_: self._app.quit())
        menu.append(quit_item)

        menu.show_all()
        return menu

    def _show_window(self):
        self._win.present()

    def _check_updates(self):
        if hasattr(self._win, "_daemon"):
            self._win._daemon.check_now()

    def set_updates(self, count: int, summary: str = ""):
        self._update_count = count
        if self._ind and hasattr(self, "_status_item"):
            if count > 0:
                self._status_item.set_label(summary or f"{count} update{'s' if count!=1 else ''} available")
                self._status_item.set_sensitive(True)
                self._ind.set_icon_full("software-update-available", "Updates available")
            else:
                self._status_item.set_label("No updates available")
                self._status_item.set_sensitive(False)
                self._ind.set_icon_full("system-software-update", "Up to date")

    def notify(self, title: str, body: str):
        _send_notification(title, body)
