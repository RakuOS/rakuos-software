"""
ui_gtk/pages/search.py — Search results page for GTK frontend.
"""
import os
import threading
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, GLib, Pango

from backend import packages as _pkg, flatpak as _fp
from backend import webapps as _wa
from ..icon_loader import resolve_icon_path


class SearchPage(Gtk.Box):
    def __init__(self, window):
        super().__init__(orientation=Gtk.Orientation.VERTICAL)
        self._win   = window
        self._query = ""
        self.set_vexpand(True)

        self._status = Adw.StatusPage()
        self._status.set_icon_name("system-search-symbolic")
        self._status.set_title("Search")
        self._status.set_description("Type to search for apps")
        self._status.set_vexpand(True)

        scroll = Gtk.ScrolledWindow()
        scroll.set_vexpand(True)
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self._list = Gtk.ListBox()
        self._list.set_selection_mode(Gtk.SelectionMode.NONE)
        self._list.add_css_class("boxed-list")
        self._list.set_margin_start(12)
        self._list.set_margin_end(12)
        self._list.set_margin_top(8)
        self._list.set_margin_bottom(12)
        scroll.set_child(self._list)

        self._stack = Gtk.Stack()
        self._stack.set_vexpand(True)
        self._stack.add_named(self._status, "placeholder")
        self._stack.add_named(scroll, "results")
        self.append(self._stack)

        self._stack.set_visible_child_name("placeholder")

    def search(self, query: str):
        self._query = query
        self._stack.set_visible_child_name("results")

        while True:
            row = self._list.get_first_child()
            if row is None:
                break
            self._list.remove(row)

        spinner_row = Gtk.ListBoxRow()
        sp = Gtk.Spinner()
        sp.start()
        sp.set_margin_top(24)
        sp.set_margin_bottom(24)
        sp.set_halign(Gtk.Align.CENTER)
        spinner_row.set_child(sp)
        self._list.append(spinner_row)

        threading.Thread(
            target=self._search_worker, args=(query,), daemon=True).start()

    def _search_worker(self, query: str):
        try:
            results = _pkg.search_packages(query) + _fp.search_flatpaks(query)
            results += _wa.search_webapps(query)
        except Exception:
            results = []
        GLib.idle_add(self._show_results, query, results)

    def _show_results(self, query: str, results: list):
        if query != self._query:
            return False  # Stale

        while True:
            row = self._list.get_first_child()
            if row is None:
                break
            self._list.remove(row)

        if not results:
            row = Adw.ActionRow()
            row.set_title(f'No results for \u201c{query}\u201d')
            self._list.append(row)
            return False

        for app in results:
            row = self._make_row(app)
            self._list.append(row)

        return False

    def _make_row(self, app: dict) -> Gtk.Widget:
        row = Adw.ActionRow()
        row.set_title(app.get("name", app.get("id", "")))
        row.set_subtitle(app.get("summary", ""))
        row.set_activatable(True)
        row.connect("activated", lambda *_: self._win.navigate_to("detail", app=app))

        icon_img = Gtk.Image()
        icon_img.set_pixel_size(40)
        icon_path = resolve_icon_path(app)
        if icon_path and os.path.exists(icon_path):
            icon_img.set_from_file(icon_path)
        else:
            icon_img.set_from_icon_name(app.get("icon", "application-x-executable-symbolic"))
        row.add_prefix(icon_img)
        row.add_suffix(Gtk.Image.new_from_icon_name("go-next-symbolic"))

        return row
