"""
ui_gtk/pages/webapps.py — Web Apps page for GTK frontend.
"""
import os
import threading
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, GLib, Pango

from backend import webapps as _wa


class WebAppsPage(Gtk.Box):
    def __init__(self, window):
        super().__init__(orientation=Gtk.Orientation.VERTICAL)
        self._win = window
        self.set_vexpand(True)

        # Top bar
        bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        bar.set_margin_start(12)
        bar.set_margin_end(12)
        bar.set_margin_top(8)
        bar.set_margin_bottom(4)

        title = Gtk.Label(label="Web Apps")
        title.add_css_class("title-2")
        bar.append(title)
        bar.append(Gtk.Box(hexpand=True))

        refresh_btn = Gtk.Button()
        refresh_btn.set_icon_name("view-refresh-symbolic")
        refresh_btn.add_css_class("flat")
        refresh_btn.connect("clicked", lambda *_: self.reload())
        bar.append(refresh_btn)
        self.append(bar)

        # Tabs: Browse | Installed
        self._tab_stack = Gtk.Stack()
        self._tab_stack.set_vexpand(True)
        self._tab_stack.set_transition_type(Gtk.StackTransitionType.SLIDE_LEFT_RIGHT)

        switcher = Gtk.StackSwitcher()
        switcher.set_stack(self._tab_stack)
        switcher.set_halign(Gtk.Align.CENTER)
        switcher.set_margin_bottom(4)
        self.append(switcher)
        self.append(self._tab_stack)

        self._build_catalog_tab()
        self._build_installed_tab()

        GLib.idle_add(self.reload)

    def _build_catalog_tab(self):
        scroll = Gtk.ScrolledWindow()
        scroll.set_vexpand(True)
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self._catalog_flow = Gtk.FlowBox()
        self._catalog_flow.set_homogeneous(True)
        self._catalog_flow.set_min_children_per_line(2)
        self._catalog_flow.set_max_children_per_line(3)
        self._catalog_flow.set_column_spacing(8)
        self._catalog_flow.set_row_spacing(8)
        self._catalog_flow.set_margin_start(12)
        self._catalog_flow.set_margin_end(12)
        self._catalog_flow.set_margin_top(8)
        self._catalog_flow.set_margin_bottom(12)
        self._catalog_flow.set_selection_mode(Gtk.SelectionMode.NONE)

        scroll.set_child(self._catalog_flow)
        self._tab_stack.add_titled(scroll, "catalog", "Browse")

    def _build_installed_tab(self):
        scroll = Gtk.ScrolledWindow()
        scroll.set_vexpand(True)
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self._installed_list = Gtk.ListBox()
        self._installed_list.set_selection_mode(Gtk.SelectionMode.NONE)
        self._installed_list.add_css_class("boxed-list")
        self._installed_list.set_margin_start(12)
        self._installed_list.set_margin_end(12)
        self._installed_list.set_margin_top(8)
        self._installed_list.set_margin_bottom(12)

        scroll.set_child(self._installed_list)
        self._tab_stack.add_titled(scroll, "installed", "Installed")

    def reload(self):
        threading.Thread(target=self._load_worker, daemon=True).start()
        return False

    def _load_worker(self):
        try:
            catalog   = _wa.get_catalog()
            installed = _wa.get_installed()
        except Exception:
            catalog, installed = [], []
        GLib.idle_add(self._populate, catalog, installed)

    def _populate(self, catalog: list, installed: list):
        # Catalog
        while True:
            c = self._catalog_flow.get_first_child()
            if c is None:
                break
            self._catalog_flow.remove(c)

        for app in catalog:
            card = self._make_catalog_card(app, installed)
            self._catalog_flow.append(card)

        # Installed
        while True:
            r = self._installed_list.get_first_child()
            if r is None:
                break
            self._installed_list.remove(r)

        if not installed:
            row = Adw.ActionRow()
            row.set_title("No web apps installed")
            self._installed_list.append(row)
        else:
            for app in installed:
                row = self._make_installed_row(app)
                self._installed_list.append(row)

        return False

    def _make_catalog_card(self, app: dict, installed: list) -> Gtk.Widget:
        btn = Gtk.Button()
        btn.add_css_class("card")
        btn.set_hexpand(True)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.set_margin_top(12)
        box.set_margin_bottom(12)
        box.set_margin_start(12)
        box.set_margin_end(12)

        # Icon
        icon_img = Gtk.Image()
        icon_img.set_pixel_size(56)
        icon_path = app.get("icon_path", "")
        if icon_path and os.path.exists(icon_path):
            icon_img.set_from_file(icon_path)
        else:
            icon_img.set_from_icon_name("web-browser-symbolic")

        name_lbl = Gtk.Label(label=app.get("name", ""))
        name_lbl.add_css_class("caption-heading")
        name_lbl.set_ellipsize(Pango.EllipsizeMode.END)

        is_installed = any(i.get("id") == app.get("id") for i in installed)
        status_lbl = Gtk.Label(label="Installed" if is_installed else "")
        status_lbl.add_css_class("caption")
        status_lbl.add_css_class("dim-label" if not is_installed else "success")

        box.append(icon_img)
        box.append(name_lbl)
        box.append(status_lbl)
        btn.set_child(box)
        btn.connect("clicked", lambda *_: self._win.navigate_to("detail", app=app))
        return btn

    def _make_installed_row(self, app: dict) -> Gtk.Widget:
        row = Adw.ActionRow()
        row.set_title(app.get("name", ""))
        row.set_subtitle(app.get("url", app.get("website", "")))
        row.set_activatable(True)
        row.connect("activated", lambda *_: self._win.navigate_to("detail", app=app))

        icon_img = Gtk.Image()
        icon_img.set_pixel_size(40)
        icon_path = app.get("icon_path", "")
        if icon_path and os.path.exists(icon_path):
            icon_img.set_from_file(icon_path)
        else:
            icon_img.set_from_icon_name("web-browser-symbolic")
        row.add_prefix(icon_img)

        return row

