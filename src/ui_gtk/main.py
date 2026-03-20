"""
ui_gtk/main.py — GTK4/libadwaita frontend entry point for RakuOS Software Center.
"""
import os
import sys
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gio, GLib

from .window import MainWindow


class RakuOSSoftwareApp(Adw.Application):
    def __init__(self, **file_args):
        super().__init__(
            application_id="org.rakuos.Software",
            flags=Gio.ApplicationFlags.DEFAULT_FLAGS)
        self._file_args = file_args
        self.connect("activate", self._on_activate)

    def _on_activate(self, app):
        existing = self.get_windows()
        if existing:
            existing[0].present()
            return

        win = MainWindow(app, **self._file_args)
        win.connect("close-request", self._on_close_request)
        win.present()

    def _on_close_request(self, win):
        """Hide to tray instead of quitting."""
        win.hide()
        return True  # prevent destroy


def run(rpm_file=None, flatpak_file=None, flatpakref=None, appimage_file=None):
    app = RakuOSSoftwareApp(
        rpm_file=rpm_file,
        flatpak_file=flatpak_file,
        flatpakref=flatpakref,
        appimage_file=appimage_file)
    sys.exit(app.run(sys.argv[:1]))  # pass only argv[0] — we've already parsed args
