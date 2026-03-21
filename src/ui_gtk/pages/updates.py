"""
ui_gtk/pages/updates.py — Updates page for GTK frontend.

GNOME Software style: grouped sections per update type, progress bars,
sequential updates, rows dissolve on success, reboot screen for OS image.
"""
import re
import subprocess
import threading
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, GLib

from backend import updates as _upd
from ..icon_loader import resolve_icon_path


class UpdatesPage(Gtk.Box):
    def __init__(self, window):
        super().__init__(orientation=Gtk.Orientation.VERTICAL)
        self._win   = window
        self._data  = {}
        self._image_card  = None
        self._update_rows = []
        self._busy        = False   # True while an update sequence is running
        self.set_vexpand(True)

        # Stack: "loading" | "up_to_date" | "has_updates" | "reboot_required"
        self._stack = Gtk.Stack()
        self._stack.set_vexpand(True)
        self._stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self.append(self._stack)

        self._build_loading_page()
        self._build_up_to_date_page()
        self._build_updates_page()
        self._build_reboot_page()

        self._stack.set_visible_child_name("loading")

    # ── Pages ─────────────────────────────────────────────────────────────────

    def _build_loading_page(self):
        status = Adw.StatusPage()
        status.set_icon_name("content-loading-symbolic")
        status.set_title("Checking for Updates…")
        self._stack.add_named(status, "loading")

    def _build_up_to_date_page(self):
        status = Adw.StatusPage()
        status.set_icon_name("emblem-default-symbolic")
        status.set_title("Up to Date")
        status.set_description("Your system and all apps are up to date.")

        check_btn = Gtk.Button(label="Check Again")
        check_btn.add_css_class("pill")
        check_btn.set_halign(Gtk.Align.CENTER)
        check_btn.connect("clicked", lambda *_: self._do_check())
        status.set_child(check_btn)

        self._stack.add_named(status, "up_to_date")

    def _build_updates_page(self):
        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        outer.set_vexpand(True)

        bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        bar.set_margin_start(12)
        bar.set_margin_end(12)
        bar.set_margin_top(8)
        bar.set_margin_bottom(4)

        refresh_btn = Gtk.Button()
        refresh_btn.set_icon_name("view-refresh-symbolic")
        refresh_btn.add_css_class("flat")
        refresh_btn.connect("clicked", lambda *_: self._do_check())
        bar.append(refresh_btn)

        bar.append(Gtk.Box(hexpand=True))  # spacer

        self._update_all_btn = Gtk.Button(label="Update All")
        self._update_all_btn.add_css_class("suggested-action")
        self._update_all_btn.connect("clicked", lambda *_: self._do_update_all())
        bar.append(self._update_all_btn)

        outer.append(bar)

        scroll = Gtk.ScrolledWindow()
        scroll.set_vexpand(True)
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self._updates_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        self._updates_box.set_margin_start(12)
        self._updates_box.set_margin_end(12)
        self._updates_box.set_margin_top(4)
        self._updates_box.set_margin_bottom(12)
        scroll.set_child(self._updates_box)
        outer.append(scroll)

        self._stack.add_named(outer, "has_updates")

    def _build_reboot_page(self):
        status = Adw.StatusPage()
        status.set_icon_name("system-reboot-symbolic")
        status.set_title("Restart Required")
        status.set_description(
            "The new system image has been downloaded and staged.\n"
            "Restart to boot into the updated system.")

        btn_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8,
                          halign=Gtk.Align.CENTER)

        reboot_btn = Gtk.Button(label="Restart Now")
        reboot_btn.add_css_class("suggested-action")
        reboot_btn.add_css_class("pill")
        reboot_btn.connect("clicked", lambda *_: subprocess.Popen(["systemctl", "reboot"]))
        btn_box.append(reboot_btn)

        later_btn = Gtk.Button(label="Restart Later")
        later_btn.add_css_class("flat")
        later_btn.connect("clicked",
            lambda *_: self._stack.set_visible_child_name("up_to_date"))
        btn_box.append(later_btn)

        status.set_child(btn_box)
        self._stack.add_named(status, "reboot_required")

    # ── Public API ────────────────────────────────────────────────────────────

    def set_updates(self, data: dict):
        """Called by daemon or after manual check. Ignored while updating."""
        if self._busy:
            return   # don't tear down rows while a sequence is running
        self._data = data
        if data.get("total", 0) == 0:
            self._stack.set_visible_child_name("up_to_date")
        else:
            self._stack.set_visible_child_name("has_updates")
            self._populate_updates(data)

    def _do_check(self):
        """Manual refresh — show loading then fetch in background."""
        if self._busy:
            return
        self._stack.set_visible_child_name("loading")
        threading.Thread(target=self._fetch_and_render, daemon=True).start()

    def _fetch_and_render(self):
        import json

        def _check(cmd):
            try:
                r = subprocess.run(["/usr/libexec/rakuos/rakuos-update", cmd],
                    capture_output=True, text=True, timeout=120)
                data = json.loads(r.stdout)
                return r.returncode == 0, data.get("updates", [])
            except Exception:
                return False, []

        def _check_image():
            try:
                r = subprocess.run(["/usr/libexec/rakuos/rakuos-update", "check-image"],
                    capture_output=True, text=True, timeout=30)
                data = json.loads(r.stdout)
                return r.returncode == 0, data
            except Exception:
                return False, {}

        _, pkgs             = _check("check")
        _, fps              = _check("check-flatpak")
        img_avail, img_info = _check_image()

        result = {
            "packages":        pkgs,
            "flatpak":         fps,
            "image_available": img_avail,
            "image_info":      img_info,
            "total": len(pkgs) + len(fps) + (1 if img_avail else 0),
        }
        GLib.idle_add(self._apply_check_result, result)

    def _apply_check_result(self, result: dict):
        """Apply a page-initiated check result — update page AND notify window for tray."""
        self.set_updates(result)
        self._win._on_page_check_done(result)
        return False

    # ── Populate update rows ───────────────────────────────────────────────────

    def _populate_updates(self, data: dict):
        while True:
            child = self._updates_box.get_first_child()
            if child is None:
                break
            self._updates_box.remove(child)

        self._update_rows = []
        self._image_card  = None

        pkgs = data.get("packages", [])
        fps  = data.get("flatpak", [])
        ais  = data.get("appimages", [])

        gui_pkgs  = [p for p in pkgs if p.get("gui")]
        fp_apps   = [dict(p, is_flatpak=True) for p in fps if not p.get("runtime")]
        ai_apps   = [dict(a, is_appimage=True) for a in ais]
        app_group = gui_pkgs + fp_apps + ai_apps
        if app_group:
            self._updates_box.append(self._make_section("Applications", app_group))

        fp_runtimes = [dict(p, is_flatpak=True) for p in fps if p.get("runtime")]
        if fp_runtimes:
            self._updates_box.append(self._make_section("Add-ons", fp_runtimes))

        sys_pkgs = [p for p in pkgs if not p.get("gui")]
        if sys_pkgs:
            self._updates_box.append(self._make_section("System Dependencies", sys_pkgs))

        if data.get("image_available"):
            self._image_card = ImageUpdateCard(data.get("image_info", {}), self)
            self._updates_box.append(self._image_card)

    def _make_section(self, title: str, items: list) -> Gtk.Widget:
        group = Adw.PreferencesGroup()
        group.set_title(title)
        for item in items:
            row = UpdateRow(item)
            group.add(row)
            self._update_rows.append(row)
        return group

    # ── Update all ────────────────────────────────────────────────────────────

    def _do_update_all(self):
        if self._busy:
            return
        self._busy = True
        self._update_all_btn.set_sensitive(False)
        self._update_all_btn.set_label("Updating…")

        rpm_rows = [r for r in self._update_rows
                    if not r._item.get("is_flatpak") and not r._item.get("is_appimage")]
        fp_rows  = [r for r in self._update_rows if r._item.get("is_flatpak")]
        ai_rows  = [r for r in self._update_rows if r._item.get("is_appimage")]

        steps = []
        if rpm_rows:
            steps.append(lambda cb, _rows=rpm_rows: self._run_rpm_batch(_rows, cb))
        for row in fp_rows:
            steps.append(lambda cb, r=row: self._run_flatpak(r, cb))
        for row in ai_rows:
            steps.append(lambda cb, r=row: self._run_appimage(r, cb))

        def on_apps_done():
            if self._image_card and self._data.get("image_available"):
                self._run_image_update(self._on_image_done)
            else:
                GLib.idle_add(self._finish_updates)

        self._run_step_sequence(steps, on_apps_done)

    def _run_step_sequence(self, steps: list, on_complete):
        if not steps:
            try:
                on_complete()
            except Exception as e:
                print(f"[updates] on_complete error: {e}")
            return

        step, remaining = steps[0], steps[1:]

        def after(success):
            GLib.timeout_add(300,
                lambda: (self._run_step_sequence(remaining, on_complete), False)[1])

        try:
            step(after)
        except Exception as e:
            print(f"[updates] step error: {e}")
            GLib.timeout_add(300,
                lambda: (self._run_step_sequence(remaining, on_complete), False)[1])

    def _finish_updates(self):
        self._busy = False
        self._data = {}
        self._update_all_btn.set_label("Update All")
        self._update_all_btn.set_sensitive(True)
        if self._stack.get_visible_child_name() != "reboot_required":
            self._stack.set_visible_child_name("up_to_date")
        # Re-check so tray badge clears immediately
        GLib.timeout_add(600, lambda: (self._do_check(), False)[1])
        return False

    def _on_image_done(self, ok: bool):
        self._busy = False
        self._data = {}
        if ok:
            GLib.idle_add(
                lambda: self._stack.set_visible_child_name("reboot_required") or False)
        else:
            if self._image_card:
                GLib.idle_add(self._image_card.set_error)
        self._update_all_btn.set_label("Update All")
        self._update_all_btn.set_sensitive(True)

    # ── Cache helpers ─────────────────────────────────────────────────────────

    def _remove_from_data(self, pkgs: list[dict]):
        """Remove completed packages from data cache and notify window for tray update."""
        if not self._data:
            return
        ids = {p.get("app_id") or p.get("id", "") for p in pkgs}
        for key in ("packages", "flatpak", "appimages"):
            self._data[key] = [
                p for p in self._data.get(key, [])
                if (p.get("app_id") or p.get("id", "")) not in ids
            ]
        self._data["total"] = (
            len(self._data.get("packages", []))
            + len(self._data.get("flatpak", []))
            + (1 if self._data.get("image_available") else 0)
        )
        self._win._on_page_check_done(self._data)

    # ── Per-type update runners ────────────────────────────────────────────────

    def _run_rpm_batch(self, rows: list, done_cb):
        for r in rows:
            r.set_updating()

        def _worker():
            ok = True
            try:
                for line in _upd.upgrade_packages_stream():
                    if line.startswith("__done__"):
                        ok = line == "__done__0"
                        break
                    p = _parse_dnf_progress(line)
                    if p is not None:
                        for r in rows:
                            GLib.idle_add(r.set_progress, p)
            except Exception as e:
                print(f"[updates] rpm batch error: {e}")
                ok = False
            for r in rows:
                GLib.idle_add(r.set_done, ok)
            if ok:
                GLib.idle_add(self._remove_from_data, [r._item for r in rows])
            GLib.idle_add(done_cb, ok)

        threading.Thread(target=_worker, daemon=True).start()

    def _run_flatpak(self, row: "UpdateRow", done_cb):
        item   = row._item
        app_id = item.get("app_id") or item.get("id") or item.get("application", "")
        row.set_updating()

        def _worker():
            ok = True
            try:
                from backend import flatpak as _fp
                for line in _fp.update_flatpak_stream(app_id):
                    if line.startswith("__done__"):
                        ok = line == "__done__0"
                        break
                    p = _parse_flatpak_progress(line)
                    if p is not None:
                        GLib.idle_add(row.set_progress, p)
            except Exception as e:
                print(f"[updates] flatpak error for {app_id}: {e}")
                ok = False
            GLib.idle_add(row.set_done, ok)
            if ok:
                GLib.idle_add(self._remove_from_data, [row._item])
            GLib.idle_add(done_cb, ok)

        threading.Thread(target=_worker, daemon=True).start()

    def _run_appimage(self, row: "UpdateRow", done_cb):
        item   = row._item
        app_id = item.get("id", "")
        dl_url = item.get("download_url", "")

        if not app_id or not dl_url:
            row.set_done(False)
            GLib.idle_add(done_cb, False)
            return

        row.set_updating()

        def _worker():
            ok = True
            try:
                from backend import appimages as _aim
                for line in _aim.update_appimage_stream(app_id, dl_url):
                    if line.startswith("__done__"):
                        ok = line == "__done__0"
                        break
                    if line.startswith("DOWNLOAD:"):
                        try:
                            GLib.idle_add(row.set_progress,
                                          int(line.split(":")[1]) / 100)
                        except (ValueError, IndexError):
                            pass
            except Exception as e:
                print(f"[updates] appimage error for {app_id}: {e}")
                ok = False
            GLib.idle_add(row.set_done, ok)
            if ok:
                GLib.idle_add(self._remove_from_data, [row._item])
            GLib.idle_add(done_cb, ok)

        threading.Thread(target=_worker, daemon=True).start()

    def _run_image_update(self, done_cb):
        if self._image_card:
            GLib.idle_add(self._image_card.set_updating)

        info        = self._data.get("image_info", {})
        update_type = info.get("type", "switch")
        repo_url    = info.get("repo", "")
        new_tag     = info.get("available", "")

        def _worker():
            ok = True
            try:
                for line in _upd.upgrade_image_stream(update_type, repo_url, new_tag):
                    if line.startswith("__done__"):
                        ok = line == "__done__0"
                        break
                    p = _parse_bootc_progress(line)
                    if p is not None and self._image_card:
                        GLib.idle_add(self._image_card.set_progress, p)
            except Exception as e:
                print(f"[updates] image update error: {e}")
                ok = False
            GLib.idle_add(done_cb, ok)

        threading.Thread(target=_worker, daemon=True).start()


# ── Update row widget ──────────────────────────────────────────────────────────

class UpdateRow(Adw.ActionRow):
    def __init__(self, item: dict):
        super().__init__()
        self._item    = item
        self._alive   = True   # False after dissolve

        import os
        self.set_title(item.get("name") or item.get("id", ""))
        ver_from = item.get("version", item.get("current_version", ""))
        ver_to   = item.get("new_version", item.get("available_version", ""))
        if ver_from and ver_to:
            self.set_subtitle(f"{ver_from} → {ver_to}")
        elif ver_to:
            self.set_subtitle(ver_to)

        icon_img = Gtk.Image()
        icon_img.set_pixel_size(36)
        icon_path = resolve_icon_path(item)
        if icon_path and os.path.exists(icon_path):
            icon_img.set_from_file(icon_path)
        else:
            icon_img.set_from_icon_name(
                item.get("icon", "application-x-executable-symbolic"))
        self.add_prefix(icon_img)

        self._progress = Gtk.ProgressBar()
        self._progress.set_pulse_step(0.1)
        self._progress.set_visible(False)
        self._progress.set_valign(Gtk.Align.CENTER)
        self._progress.set_size_request(120, -1)
        self.add_suffix(self._progress)

    def set_updating(self):
        if not self._alive:
            return
        self._progress.set_visible(True)
        self._progress.set_fraction(0.0)

        def _pulse():
            if not self._alive:
                return False
            if self._progress.get_fraction() == 0.0 and self._progress.get_visible():
                self._progress.pulse()
                return True
            return False

        GLib.timeout_add(150, _pulse)

    def set_progress(self, fraction: float):
        if not self._alive:
            return
        self._progress.set_visible(True)
        self._progress.set_fraction(min(1.0, max(0.0, fraction)))

    def set_done(self, success: bool):
        if not self._alive:
            return
        self._progress.set_fraction(1.0 if success else 0.0)
        if success:
            GLib.timeout_add(500, self._dissolve)

    def _dissolve(self):
        self._alive = False
        # Just hide — don't try to remove from AdwPreferencesGroup's
        # internal container (causes GTK criticals).
        self.set_visible(False)
        return False


# ── OS Image update card ───────────────────────────────────────────────────────

class ImageUpdateCard(Gtk.Frame):
    def __init__(self, info: dict, page: "UpdatesPage"):
        super().__init__()
        self.add_css_class("card")
        self._page = page
        self._info = info

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.set_margin_top(12)
        box.set_margin_bottom(12)
        box.set_margin_start(16)
        box.set_margin_end(16)

        hdr = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        icon = Gtk.Image.new_from_icon_name("computer-symbolic")
        icon.set_pixel_size(36)
        hdr.append(icon)

        title_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2,
                            valign=Gtk.Align.CENTER, hexpand=True)
        title_lbl = Gtk.Label(label="RakuOS")
        title_lbl.add_css_class("title-4")
        title_lbl.set_halign(Gtk.Align.START)
        title_box.append(title_lbl)

        booted    = info.get("booted", "")
        available = info.get("available", "")
        if booted and available:
            sub_lbl = Gtk.Label(label=f"{booted}  →  {available}")
        elif available:
            sub_lbl = Gtk.Label(label=f"New image: {available}")
        else:
            sub_lbl = Gtk.Label(label="A new system image is available.")
        sub_lbl.add_css_class("dim-label")
        sub_lbl.set_halign(Gtk.Align.START)
        title_box.append(sub_lbl)

        hdr.append(title_box)

        self._update_btn = Gtk.Button(label="Update System Image")
        self._update_btn.add_css_class("suggested-action")
        self._update_btn.set_valign(Gtk.Align.CENTER)
        self._update_btn.connect("clicked",
            lambda *_: page._run_image_update(page._on_image_done))
        hdr.append(self._update_btn)

        box.append(hdr)

        self._progress = Gtk.ProgressBar()
        self._progress.set_pulse_step(0.03)
        self._progress.set_visible(False)
        box.append(self._progress)

        self.set_child(box)

    def set_updating(self):
        self._update_btn.set_sensitive(False)
        self._update_btn.set_label("Updating…")
        self._progress.set_visible(True)
        self._progress.set_fraction(0.0)

        def _pulse():
            if self._progress.get_fraction() == 0.0 and self._progress.get_visible():
                self._progress.pulse()
                return True
            return False

        GLib.timeout_add(200, _pulse)

    def set_progress(self, fraction: float):
        self._progress.set_visible(True)
        self._progress.set_fraction(min(1.0, max(0.0, fraction)))

    def set_error(self):
        self._update_btn.set_sensitive(True)
        self._update_btn.set_label("Retry")
        self._progress.set_visible(False)


# ── Progress parsers ──────────────────────────────────────────────────────────

def _parse_bootc_progress(line: str) -> float | None:
    m = re.search(r"layers\[(\d+)/(\d+)\]", line, re.IGNORECASE)
    if m:
        n, total = int(m.group(1)), int(m.group(2))
        return n / total if total else None
    return None


def _parse_flatpak_progress(line: str) -> float | None:
    m = re.search(r"\[(\d+)/(\d+)\]", line)
    if m:
        n, total = int(m.group(1)), int(m.group(2))
        return n / total if total else None
    m = re.search(r"(\d+)%", line)
    if m:
        return int(m.group(1)) / 100
    return None


def _parse_dnf_progress(line: str) -> float | None:
    m = re.search(r"\[(\d+)/(\d+)\]", line)
    if m:
        n, total = int(m.group(1)), int(m.group(2))
        return n / total if total else None
    return None
