"""
ui_gtk/pages/appimage_detail.py — AppImage detail page for GTK frontend.
"""
import json
import os
import threading
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, GLib, Pango

from backend import appimages as _ai
from ..icon_loader import resolve_icon_path


class AppImageDetailPage(Gtk.ScrolledWindow):
    def __init__(self, window):
        super().__init__()
        self._win = window
        self.set_vexpand(True)
        self.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self._root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.set_child(self._root)

        self._app = {}

    def load_app(self, app: dict):
        self._app = app

        while True:
            c = self._root.get_first_child()
            if c is None:
                break
            self._root.remove(c)

        self._build_hero(app)
        self._build_details(app)
        self._build_update_source(app)

    # ── Hero ──────────────────────────────────────────────────────────────────

    def _build_hero(self, app: dict):
        hero = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=16)
        hero.set_margin_top(24)
        hero.set_margin_bottom(16)
        hero.set_margin_start(24)
        hero.set_margin_end(24)

        # Icon — prefer icon_data (raw bytes from AppImage) then file path
        icon_img = Gtk.Image()
        icon_img.set_pixel_size(96)
        icon_data = app.get("icon_data")
        icon_path = resolve_icon_path(app) or app.get("icon_path", "")
        if icon_data:
            import tempfile
            try:
                with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
                    f.write(icon_data)
                    icon_img.set_from_file(f.name)
            except Exception:
                icon_img.set_from_icon_name("application-x-executable-symbolic")
        elif icon_path and os.path.exists(icon_path):
            icon_img.set_from_file(icon_path)
        else:
            icon_img.set_from_icon_name("application-x-executable-symbolic")
        hero.append(icon_img)

        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6,
                        valign=Gtk.Align.CENTER, hexpand=True)

        name_lbl = Gtk.Label(label=app.get("name", ""))
        name_lbl.add_css_class("title-1")
        name_lbl.set_halign(Gtk.Align.START)
        name_lbl.set_ellipsize(Pango.EllipsizeMode.END)
        right.append(name_lbl)

        ver = app.get("version", "")
        if ver:
            ver_lbl = Gtk.Label(label=f"Version {ver}")
            ver_lbl.add_css_class("dim-label")
            ver_lbl.set_halign(Gtk.Align.START)
            right.append(ver_lbl)

        # Update source badge
        src = app.get("update_source", "none")
        if src and src != "none":
            badge_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
            badge_lbl = Gtk.Label(label=f"Updates: {src.title()}")
            badge_lbl.add_css_class("caption")
            badge_lbl.add_css_class("accent")
            badge_box.append(badge_lbl)
            badge_box.set_halign(Gtk.Align.START)
            right.append(badge_box)

        summary = app.get("summary") or app.get("description", "")
        if summary:
            sum_lbl = Gtk.Label(label=summary)
            sum_lbl.set_halign(Gtk.Align.START)
            sum_lbl.set_wrap(True)
            sum_lbl.set_max_width_chars(50)
            sum_lbl.add_css_class("dim-label")
            right.append(sum_lbl)

        # Action button: Install / Uninstall
        btn_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        btn_box.set_halign(Gtk.Align.START)
        btn_box.set_margin_top(4)

        self._action_btn = Gtk.Button()
        if app.get("installed"):
            self._action_btn.set_label("Uninstall")
            self._action_btn.add_css_class("destructive-action")
            self._action_btn.connect("clicked", lambda *_: self._do_uninstall())
        else:
            self._action_btn.set_label("Install")
            self._action_btn.add_css_class("suggested-action")
            self._action_btn.connect("clicked", lambda *_: self._do_install())
        btn_box.append(self._action_btn)

        # Update button (only for installed with an update source)
        if app.get("installed") and src != "none":
            upd_btn = Gtk.Button(label="Check for Update")
            upd_btn.add_css_class("flat")
            upd_btn.connect("clicked", lambda *_: self._do_update())
            btn_box.append(upd_btn)

        right.append(btn_box)

        # Progress bar
        self._progress = Gtk.ProgressBar()
        self._progress.set_pulse_step(0.1)
        self._progress.set_visible(False)
        self._progress.set_margin_top(4)
        right.append(self._progress)

        hero.append(right)
        self._root.append(hero)
        self._root.append(Gtk.Separator(orientation=Gtk.Orientation.HORIZONTAL))

    # ── Details ───────────────────────────────────────────────────────────────

    def _build_details(self, app: dict):
        group = Adw.PreferencesGroup()
        group.set_title("Details")
        group.set_margin_start(12)
        group.set_margin_end(12)
        group.set_margin_top(16)

        fields = [
            ("Version",        "version"),
            ("Installed Path", "installed_path"),
            ("Source Archive", "archive_path"),
            ("Path",           "src_path"),
        ]
        for title, key in fields:
            val = app.get(key, "")
            if val:
                row = Adw.ActionRow()
                row.set_title(title)
                row.set_subtitle(str(val))
                group.add(row)

        self._root.append(group)

    # ── Update source section ─────────────────────────────────────────────────

    def _build_update_source(self, app: dict):
        group = Adw.PreferencesGroup()
        group.set_title("Update Source")
        group.set_description("Configure how RakuOS checks for updates for this AppImage.")
        group.set_margin_start(12)
        group.set_margin_end(12)
        group.set_margin_top(12)
        group.set_margin_bottom(8)

        # Source type dropdown row
        src_row = Adw.ActionRow()
        src_row.set_title("Source")

        src_options = ["None", "GitHub", "GitLab", "URL"]
        src_values  = ["none", "github", "gitlab", "url"]
        self._src_dd = Gtk.DropDown.new_from_strings(src_options)
        cur = app.get("update_source", "none")
        if cur in src_values:
            self._src_dd.set_selected(src_values.index(cur))
        self._src_dd.set_valign(Gtk.Align.CENTER)
        self._src_dd.connect("notify::selected", self._on_src_changed)
        src_row.add_suffix(self._src_dd)
        group.add(src_row)

        # URL entry row
        self._url_row = Adw.EntryRow()
        self._url_row.set_title("Update URL")
        self._url_row.set_text(app.get("update_url", ""))
        self._url_row.set_show_apply_button(False)
        group.add(self._url_row)

        # Pattern entry row
        self._pattern_row = Adw.EntryRow()
        self._pattern_row.set_title("Filename Pattern")
        self._pattern_row.set_text(app.get("update_pattern", "*.AppImage"))
        self._pattern_row.set_show_apply_button(False)
        group.add(self._pattern_row)

        # Show/hide URL+pattern based on current selection
        visible = cur != "none"
        self._url_row.set_visible(visible)
        self._pattern_row.set_visible(visible)

        self._root.append(group)

        # Save button — only for installed apps
        if app.get("installed"):
            save_btn = Gtk.Button(label="Save Update Settings")
            save_btn.add_css_class("pill")
            save_btn.set_halign(Gtk.Align.START)
            save_btn.set_margin_start(12)
            save_btn.set_margin_top(4)
            save_btn.set_margin_bottom(16)
            save_btn.connect("clicked", lambda *_: self._save_update_settings())
            self._root.append(save_btn)

        # Status label for save feedback
        self._status_lbl = Gtk.Label(label="")
        self._status_lbl.set_halign(Gtk.Align.START)
        self._status_lbl.set_margin_start(12)
        self._status_lbl.set_visible(False)
        self._root.append(self._status_lbl)

    def _on_src_changed(self, dd, _param):
        src_values = ["none", "github", "gitlab", "url"]
        idx = dd.get_selected()
        val = src_values[idx] if 0 <= idx < len(src_values) else "none"
        visible = val != "none"
        self._url_row.set_visible(visible)
        self._pattern_row.set_visible(visible)

    # ── Actions ───────────────────────────────────────────────────────────────

    def _do_install(self):
        # src_path is set by get_appimage_info_for_display — for archives it
        # already points to the extracted AppImage inside the temp dir.
        src_path = self._app.get("src_path", self._app.get("path",
                                  self._app.get("installed_path", "")))
        if not src_path:
            return
        self._action_btn.set_sensitive(False)
        self._action_btn.set_label("Installing…")
        self._progress.set_visible(True)
        self._progress.set_fraction(0.0)
        self._start_pulse()

        def _worker():
            ok, msg, app_info = _ai.install_appimage(src_path)
            GLib.idle_add(self._on_install_done, ok, msg, app_info)

        threading.Thread(target=_worker, daemon=True).start()

    def _on_install_done(self, ok: bool, msg: str, app_info: dict):
        self._progress.set_fraction(1.0 if ok else 0.0)
        GLib.timeout_add(600, lambda: self._progress.set_visible(False) or False)
        if ok:
            self._app = {**self._app, **app_info}
            self._action_btn.set_label("Uninstall")
            self._action_btn.set_css_classes(["destructive-action"])
            self._action_btn.connect("clicked", lambda *_: self._do_uninstall())
        else:
            self._action_btn.set_label("Install")
        self._action_btn.set_sensitive(True)
        self._show_status("✓ " + msg if ok else "✗ " + msg, ok)
        return False

    def _do_uninstall(self):
        app_id = self._app.get("id", "")
        if not app_id:
            return
        self._action_btn.set_sensitive(False)
        self._action_btn.set_label("Removing…")
        self._progress.set_visible(True)
        self._progress.set_fraction(0.0)
        self._start_pulse()

        def _worker():
            ok, msg = _ai.uninstall_appimage(app_id)
            GLib.idle_add(self._on_uninstall_done, ok, msg)

        threading.Thread(target=_worker, daemon=True).start()

    def _on_uninstall_done(self, ok: bool, msg: str):
        self._progress.set_fraction(1.0 if ok else 0.0)
        GLib.timeout_add(600, lambda: self._progress.set_visible(False) or False)
        if ok:
            self._app["installed"] = False
            self._action_btn.set_label("Install")
            self._action_btn.set_css_classes(["suggested-action"])
            self._action_btn.connect("clicked", lambda *_: self._do_install())
        else:
            self._action_btn.set_label("Uninstall")
        self._action_btn.set_sensitive(True)
        self._show_status("✓ " + msg if ok else "✗ " + msg, ok)
        return False

    def _do_update(self):
        self._action_btn.set_sensitive(False)
        self._progress.set_visible(True)
        self._progress.set_fraction(0.0)
        self._start_pulse()

        def _worker():
            ok = _ai.update(self._app)
            GLib.idle_add(self._on_update_done, ok)

        threading.Thread(target=_worker, daemon=True).start()

    def _on_update_done(self, ok: bool):
        self._progress.set_fraction(1.0 if ok else 0.0)
        GLib.timeout_add(600, lambda: self._progress.set_visible(False) or False)
        self._action_btn.set_sensitive(True)
        self._show_status("✓ Updated successfully." if ok else "✗ Update failed.", ok)
        return False

    def _save_update_settings(self):
        app_id = self._app.get("id", "")
        if not app_id:
            return
        src_values = ["none", "github", "gitlab", "url"]
        idx = self._src_dd.get_selected()
        src = src_values[idx] if 0 <= idx < len(src_values) else "none"
        url     = self._url_row.get_text().strip()
        pattern = self._pattern_row.get_text().strip()

        sidecar_path = _ai.INSTALL_DIR / f"{app_id}.json"
        if not sidecar_path.exists():
            self._show_status("✗ AppImage not installed.", False)
            return
        try:
            sidecar = json.loads(sidecar_path.read_text())
            sidecar["update_source"]  = src
            sidecar["update_url"]     = url
            sidecar["update_pattern"] = pattern
            sidecar_path.write_text(json.dumps(sidecar, indent=2))
            self._app["update_source"]  = src
            self._app["update_url"]     = url
            self._app["update_pattern"] = pattern
            self._show_status("✓ Update settings saved.", True)
        except Exception as e:
            self._show_status(f"✗ Failed to save: {e}", False)

    # ── Helpers ───────────────────────────────────────────────────────────────

    def _start_pulse(self):
        def _pulse():
            if self._progress.get_visible() and self._progress.get_fraction() == 0.0:
                self._progress.pulse()
                return True
            return False
        GLib.timeout_add(200, _pulse)

    def _show_status(self, msg: str, ok: bool):
        self._status_lbl.set_text(msg)
        self._status_lbl.remove_css_class("success")
        self._status_lbl.remove_css_class("error")
        self._status_lbl.add_css_class("success" if ok else "error")
        self._status_lbl.set_visible(True)
