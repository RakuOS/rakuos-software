"""
ui_gtk/main.py — GTK4/libadwaita frontend entry point for RakuOS Software Center.
"""
import sys
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gio, GLib

from .window import MainWindow


class RakuOSSoftwareApp(Adw.Application):
    def __init__(self, start_hidden: bool = False, **file_args):
        super().__init__(
            application_id="org.rakuos.Software",
            flags=Gio.ApplicationFlags.DEFAULT_FLAGS)
        self._start_hidden = start_hidden
        self._file_args    = file_args
        self.connect("activate", self._on_activate)

    def _on_activate(self, app):
        existing = self.get_windows()
        if existing:
            existing[0].present()
            return

        win = MainWindow(app, **self._file_args)
        win.connect("close-request", self._on_close_request)

        # --tray: start hidden only if the tray icon registered successfully.
        # If there's no tray support (no SNI watcher), show the window anyway
        # so the user isn't left with no way to access the app.
        if self._start_hidden and win._tray.has_support:
            pass   # stay hidden; tray icon is the entry point
        else:
            win.present()

    def _on_close_request(self, win):
        """Hide to tray. Reset to Home so the next open is always fresh."""
        # Pop any pushed nav pages back to root
        while len(win._nav.get_navigation_stack()) > 1:
            win._nav.pop()
        # Switch to home tab
        win._stack.set_visible_child_name("home")
        win.hide()
        return True  # prevent destroy


def run(rpm_file=None, flatpak_file=None, flatpakref=None, appimage_file=None):
    start_hidden = "--tray" in sys.argv
    app = RakuOSSoftwareApp(
        start_hidden   = start_hidden,
        rpm_file       = rpm_file,
        flatpak_file   = flatpak_file,
        flatpakref     = flatpakref,
        appimage_file  = appimage_file)
    sys.exit(app.run(sys.argv[:1]))
