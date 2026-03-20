"""
ui_gtk/pages/installed.py — Installed apps page for GTK frontend.
GNOME Software style: flat list with icon, name, size, Uninstall button.
"""
import os
import threading
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, GLib, Pango

import subprocess
from backend import packages as _pkg, flatpak as _fp, appimages as _ai, webapps as _wa
from ..icon_loader import resolve_icon_path


class InstalledPage(Gtk.Box):
    def __init__(self, window):
        super().__init__(orientation=Gtk.Orientation.VERTICAL)
        self._win = window
        self.set_vexpand(True)

        # Header toolbar
        bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        bar.set_margin_start(12)
        bar.set_margin_end(12)
        bar.set_margin_top(8)
        bar.set_margin_bottom(4)

        refresh_btn = Gtk.Button()
        refresh_btn.set_icon_name("view-refresh-symbolic")
        refresh_btn.add_css_class("flat")
        refresh_btn.connect("clicked", lambda *_: self.reload())
        bar.append(refresh_btn)
        self.append(bar)

        # Filter tabs: Apps | Flatpaks | Overlays | AppImages | Webapps
        self._filter_bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        self._filter_bar.set_margin_start(12)
        self._filter_bar.set_margin_end(12)
        self._filter_bar.set_margin_bottom(8)
        self._current_filter = "all"

        for label, key in [("All", "all"), ("Apps", "native"),
                            ("Flatpaks", "flatpak"), ("AppImages", "appimage"),
                            ("Web Apps", "webapp")]:
            btn = Gtk.ToggleButton(label=label)
            if key == "all":
                btn.set_active(True)
            btn.connect("toggled", self._on_filter, key)
            self._filter_bar.append(btn)
        self.append(self._filter_bar)

        # Scrollable list
        self._scroll = Gtk.ScrolledWindow()
        self._scroll.set_vexpand(True)
        self._scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.append(self._scroll)

        self._list = Gtk.ListBox()
        self._list.set_selection_mode(Gtk.SelectionMode.NONE)
        self._list.add_css_class("boxed-list")
        self._list.set_margin_start(12)
        self._list.set_margin_end(12)
        self._list.set_margin_top(4)
        self._list.set_margin_bottom(12)
        self._scroll.set_child(self._list)

        self._all_apps: list = []
        GLib.idle_add(self.reload)

    def reload(self):
        # Show spinner
        while True:
            row = self._list.get_first_child()
            if row is None:
                break
            self._list.remove(row)

        spinner = Gtk.Spinner()
        spinner.start()
        spinner.set_margin_top(32)
        spinner.set_margin_bottom(32)
        spinner.set_halign(Gtk.Align.CENTER)
        placeholder = Gtk.ListBoxRow()
        placeholder.set_child(spinner)
        self._list.append(placeholder)

        threading.Thread(target=self._load_worker, daemon=True).start()
        return False

    def _load_worker(self):
        apps = []
        try:
            # Native overlay packages
            native = _pkg.get_installed_with_metadata()
            for a in native:
                a.setdefault("source", "native")
            apps.extend(native)
        except Exception:
            pass

        try:
            # Flatpak apps
            for a in _fp.get_installed_flatpaks():
                a.setdefault("source", "flatpak")
                apps.append(a)
        except Exception:
            pass

        try:
            # Flatpak runtimes
            r = subprocess.run(
                ["flatpak", "list", "--runtime", "--columns=name,application,version,size"],
                capture_output=True, text=True, timeout=30)
            for line in r.stdout.splitlines():
                parts = line.split("\t")
                if len(parts) >= 2:
                    apps.append({
                        "name":    parts[0].strip(),
                        "id":      parts[1].strip(),
                        "version": parts[2].strip() if len(parts) > 2 else "",
                        "size":    parts[3].strip() if len(parts) > 3 else "",
                        "source":  "flatpak",
                        "is_runtime": True,
                    })
        except Exception:
            pass

        try:
            for a in _ai.get_installed():
                a.setdefault("source", "appimage")
                apps.append(a)
        except Exception:
            pass

        try:
            for a in _wa.get_installed():
                a.setdefault("source", "webapp")
                apps.append(a)
        except Exception:
            pass

        apps.sort(key=lambda a: a.get("name", a.get("id", "")).lower())
        GLib.idle_add(self._populate, apps)

    def _populate(self, apps: list):
        self._all_apps = apps
        self._render_list(apps)
        return False

    def _render_list(self, apps: list):
        while True:
            row = self._list.get_first_child()
            if row is None:
                break
            self._list.remove(row)

        if not apps:
            row = Adw.ActionRow()
            row.set_title("No apps installed")
            row.set_subtitle("Apps you install will appear here")
            self._list.append(row)
            return

        for app in apps:
            row = self._make_row(app)
            self._list.append(row)

    def _make_row(self, app: dict) -> Gtk.Widget:
        source = app.get("source", "native")
        row = Adw.ActionRow()
        row.set_title(app.get("name", app.get("id", "")))

        size = app.get("size") or app.get("installed_size")
        if size:
            subtitle = _fmt_size(size)
        elif source == "webapp":
            subtitle = app.get("url", app.get("website", "Web App"))
        elif source == "appimage":
            subtitle = f"AppImage  {app.get('version', '')}".strip()
        elif app.get("is_runtime"):
            subtitle = f"Runtime  {app.get('version', '')}".strip()
        else:
            subtitle = app.get("version", source.capitalize())
        row.set_subtitle(subtitle)
        row.set_activatable(True)

        def _open_detail(*_):
            if source == "appimage":
                self._win.navigate_to("appimage_detail", app=app)
            else:
                self._win.navigate_to("detail", app=app)

        row.connect("activated", _open_detail)

        # Icon
        icon_img = Gtk.Image()
        icon_img.set_pixel_size(40)
        icon_path = resolve_icon_path(app)
        if icon_path and os.path.exists(icon_path):
            icon_img.set_from_file(icon_path)
        else:
            icon_img.set_from_icon_name(app.get("icon", "application-x-executable-symbolic"))
        row.add_prefix(icon_img)

        # Uninstall button
        btn = Gtk.Button(label="Uninstall…")
        btn.add_css_class("flat")
        btn.set_valign(Gtk.Align.CENTER)
        btn.connect("clicked", lambda *_: self._confirm_uninstall(app))
        row.add_suffix(btn)

        return row

    def _on_filter(self, btn, key: str):
        if not btn.get_active():
            return
        # Deactivate others
        child = self._filter_bar.get_first_child()
        while child:
            if child != btn and isinstance(child, Gtk.ToggleButton):
                child.set_active(False)
            child = child.get_next_sibling()

        self._current_filter = key
        if key == "all":
            filtered = self._all_apps
        else:
            filtered = [a for a in self._all_apps if a.get("source", "native") == key]
        self._render_list(sorted(filtered, key=lambda a: a.get("name", a.get("id", "")).lower()))

    def _confirm_uninstall(self, app: dict):
        dlg = Adw.MessageDialog(
            heading=f"Remove {app.get('name', 'this app')}?",
            body="This will uninstall the application.")
        dlg.add_response("cancel", "Cancel")
        dlg.add_response("remove", "Remove")
        dlg.set_response_appearance("remove", Adw.ResponseAppearance.DESTRUCTIVE)
        dlg.connect("response", lambda d, r: self._do_uninstall(app) if r == "remove" else None)
        dlg.set_transient_for(self._win)
        dlg.present()

    def _do_uninstall(self, app: dict):
        source = app.get("source", "native")

        def _worker():
            ok = True
            try:
                if source == "flatpak":
                    is_runtime = app.get("is_runtime", False)
                    for _ in _fp.remove_flatpak_stream(app.get("id", ""), force=is_runtime):
                        pass
                elif source == "appimage":
                    ok, _ = _ai.uninstall_appimage(app.get("id", app.get("name", "")))
                elif source == "webapp":
                    ok, _ = _wa.uninstall(app.get("id", ""))
                else:
                    for _ in _pkg.remove_package_stream(app.get("pkg_name", app.get("name", ""))):
                        pass
            except Exception:
                ok = False
            if ok:
                GLib.idle_add(self.reload)
            else:
                GLib.idle_add(self._show_error, "Uninstall failed")

        threading.Thread(target=_worker, daemon=True).start()

    def _show_error(self, msg: str):
        dlg = Adw.MessageDialog(heading="Remove Failed", body=msg or "")
        dlg.add_response("ok", "OK")
        dlg.set_transient_for(self._win)
        dlg.present()
        return False


def _fmt_size(size) -> str:
    try:
        n = int(size)
        if n >= 1_048_576:
            return f"{n/1_048_576:.1f} MB"
        if n >= 1024:
            return f"{n/1024:.1f} KB"
        return f"{n} B"
    except Exception:
        return str(size)
