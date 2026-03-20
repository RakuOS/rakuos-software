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

RAKUOS_UPDATE = "/usr/libexec/rakuos/rakuos-update"


class UpdatesPage(Gtk.Box):
    def __init__(self, window):
        super().__init__(orientation=Gtk.Orientation.VERTICAL)
        self._win   = window
        self._data  = {}
        self.set_vexpand(True)

        # Stack: "up_to_date" | "has_updates" | "reboot_required"
        self._stack = Gtk.Stack()
        self._stack.set_vexpand(True)
        self._stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self.append(self._stack)

        self._build_up_to_date_page()
        self._build_updates_page()
        self._build_reboot_page()

        self._stack.set_visible_child_name("up_to_date")

    # ── Up-to-date status page ────────────────────────────────────────────────

    def _build_up_to_date_page(self):
        status = Adw.StatusPage()
        status.set_icon_name("emblem-ok-symbolic")
        status.set_title("Up to Date")
        status.set_description("Last checked: just now")
        self._up_to_date_page = status
        self._stack.add_named(status, "up_to_date")

    # ── Updates list page ─────────────────────────────────────────────────────

    def _build_updates_page(self):
        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        outer.set_vexpand(True)

        # Top bar with "Update All" button
        bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        bar.set_margin_start(12)
        bar.set_margin_end(12)
        bar.set_margin_top(8)
        bar.set_margin_bottom(4)

        refresh_btn = Gtk.Button()
        refresh_btn.set_icon_name("view-refresh-symbolic")
        refresh_btn.add_css_class("flat")
        refresh_btn.connect("clicked", lambda *_: self._win._daemon.check_now())
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

        self._updates_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL,
                                    spacing=8)
        self._updates_box.set_margin_start(12)
        self._updates_box.set_margin_end(12)
        self._updates_box.set_margin_top(4)
        self._updates_box.set_margin_bottom(12)
        scroll.set_child(self._updates_box)
        outer.append(scroll)

        self._stack.add_named(outer, "has_updates")

    # ── Reboot required page ──────────────────────────────────────────────────

    def _build_reboot_page(self):
        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL,
                        halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER,
                        spacing=16)
        outer.set_vexpand(True)

        status = Adw.StatusPage()
        status.set_icon_name("system-restart-symbolic")
        status.set_title("Restart Required")
        status.set_description("The system image has been updated. Restart to apply.")

        reboot_btn = Gtk.Button(label="Restart Now")
        reboot_btn.add_css_class("suggested-action")
        reboot_btn.set_halign(Gtk.Align.CENTER)
        reboot_btn.connect("clicked", lambda *_: subprocess.Popen(["systemctl", "reboot"]))

        later_btn = Gtk.Button(label="Restart Later")
        later_btn.add_css_class("flat")
        later_btn.set_halign(Gtk.Align.CENTER)
        later_btn.connect("clicked", lambda *_: self._stack.set_visible_child_name("up_to_date"))

        status.set_child(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8))
        outer.append(status)
        outer.append(reboot_btn)
        outer.append(later_btn)

        self._stack.add_named(outer, "reboot_required")

    # ── Public: called by daemon ──────────────────────────────────────────────

    def set_updates(self, data: dict):
        self._data = data
        total = data.get("total", 0)

        if total == 0:
            self._stack.set_visible_child_name("up_to_date")
            return

        self._stack.set_visible_child_name("has_updates")
        self._populate_updates(data)

    # ── Populate update rows ───────────────────────────────────────────────────

    def _populate_updates(self, data: dict):
        while True:
            child = self._updates_box.get_first_child()
            if child is None:
                break
            self._updates_box.remove(child)

        self._update_rows = []

        # RPM packages
        pkgs = data.get("packages", [])
        if pkgs:
            section = self._make_section("System Packages", pkgs, "rpm")
            self._updates_box.append(section)

        # Flatpaks
        fps = data.get("flatpak", [])
        if fps:
            section = self._make_section("Flatpak Apps", fps, "flatpak")
            self._updates_box.append(section)

        # AppImages
        ais = data.get("appimages", [])
        if ais:
            section = self._make_section("AppImages", ais, "appimage")
            self._updates_box.append(section)

        # OS image
        if data.get("image_available"):
            card = self._make_image_card(data.get("image_info", {}))
            self._updates_box.append(card)

    def _make_section(self, title: str, items: list, kind: str) -> Gtk.Widget:
        group = Adw.PreferencesGroup()
        group.set_title(title)

        rows = []
        for item in items:
            row = UpdateRow(item, kind, self._win)
            group.add(row)
            rows.append(row)
            self._update_rows.append(row)

        return group

    def _make_image_card(self, info: dict) -> Gtk.Widget:
        self._image_card = ImageUpdateCard(info, self)
        return self._image_card

    # ── Update all ────────────────────────────────────────────────────────────

    def _do_update_all(self):
        self._update_all_btn.set_sensitive(False)
        self._update_all_btn.set_label("Updating…")

        # Build sequential queue: rpms → flatpaks (one by one) → appimages → image
        queue = []

        pkgs = self._data.get("packages", [])
        if pkgs:
            queue.append(("rpm_batch", pkgs))

        for fp in self._data.get("flatpak", []):
            queue.append(("flatpak", fp))

        for ai in self._data.get("appimages", []):
            queue.append(("appimage", ai))

        if self._data.get("image_available"):
            queue.append(("image", self._data.get("image_info", {})))

        self._run_queue(queue)

    def _run_queue(self, queue: list):
        if not queue:
            # All done
            GLib.idle_add(self._on_all_done)
            return

        kind, item = queue[0]
        rest = queue[1:]

        if kind == "rpm_batch":
            self._run_rpm_batch(item, lambda ok: self._run_queue(rest))
        elif kind == "flatpak":
            self._run_flatpak(item, lambda ok: self._run_queue(rest))
        elif kind == "appimage":
            self._run_appimage(item, lambda ok: self._run_queue(rest))
        elif kind == "image":
            self._run_image_update(lambda ok: self._on_image_done(ok))

    def _on_all_done(self):
        self._update_all_btn.set_label("Update All")
        self._update_all_btn.set_sensitive(True)
        # Check if any rows remain
        GLib.timeout_add(600, self._check_all_complete)
        return False

    def _check_all_complete(self):
        self._stack.set_visible_child_name("up_to_date")
        return False

    def _on_image_done(self, ok: bool):
        if ok:
            GLib.idle_add(lambda: self._stack.set_visible_child_name("reboot_required") or False)
        self._run_queue([])  # continue (image is last)

    # ── Per-type update runners ───────────────────────────────────────────────

    def _find_row(self, item, kind: str):
        for row in self._update_rows:
            if row._kind == kind and row._item.get("name") == item.get("name"):
                return row
        return None

    def _run_rpm_batch(self, pkgs: list, done_cb):
        row = self._find_row(pkgs[0], "rpm") if pkgs else None

        def _worker():
            proc = subprocess.Popen(
                [RAKUOS_UPDATE, "upgrade"],
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            for line in proc.stdout:
                p = _parse_dnf_progress(line.strip())
                if p is not None and row:
                    GLib.idle_add(row.set_progress, p)
            proc.wait()
            ok = proc.returncode == 0
            for r in self._update_rows:
                if r._kind == "rpm":
                    GLib.idle_add(r.set_done, ok)
            GLib.idle_add(done_cb, ok)

        threading.Thread(target=_worker, daemon=True).start()

    def _run_flatpak(self, item: dict, done_cb):
        row = self._find_row(item, "flatpak")
        app_id = item.get("application", item.get("id", ""))

        def _worker():
            cmd = ["flatpak", "update", "-y", "--noninteractive"]
            if app_id:
                cmd.append(app_id)
            proc = subprocess.Popen(
                cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            for line in proc.stdout:
                p = _parse_flatpak_progress(line.strip())
                if p is not None and row:
                    GLib.idle_add(row.set_progress, p)
            proc.wait()
            ok = proc.returncode == 0
            if row:
                GLib.idle_add(row.set_done, ok)
            GLib.idle_add(done_cb, ok)

        threading.Thread(target=_worker, daemon=True).start()

    def _run_appimage(self, item: dict, done_cb):
        row = self._find_row(item, "appimage")

        def _worker():
            try:
                from backend import appimages as _ai
                ok = _ai.update(item)
            except Exception:
                ok = False
            if row:
                GLib.idle_add(row.set_done, ok)
            GLib.idle_add(done_cb, ok)

        threading.Thread(target=_worker, daemon=True).start()

    def _run_image_update(self, done_cb):
        if hasattr(self, "_image_card"):
            GLib.idle_add(self._image_card.set_updating)

        def _worker():
            proc = subprocess.Popen(
                [RAKUOS_UPDATE, "upgrade-image"],
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            for line in proc.stdout:
                p = _parse_bootc_progress(line.strip())
                if p is not None and hasattr(self, "_image_card"):
                    GLib.idle_add(self._image_card.set_progress, p)
            proc.wait()
            ok = proc.returncode == 0
            GLib.idle_add(done_cb, ok)

        threading.Thread(target=_worker, daemon=True).start()


class UpdateRow(Adw.ActionRow):
    def __init__(self, item: dict, kind: str, window):
        super().__init__()
        self._item = item
        self._kind = kind
        self._win  = window

        import os
        from backend import packages as _pkg

        self.set_title(item.get("name", item.get("id", "")))
        ver_from = item.get("version", item.get("current_version", ""))
        ver_to   = item.get("new_version", item.get("available_version", ""))
        if ver_from and ver_to:
            self.set_subtitle(f"{ver_from} → {ver_to}")
        elif ver_to:
            self.set_subtitle(ver_to)

        # Icon
        icon_img = Gtk.Image()
        icon_img.set_pixel_size(36)
        icon_path = resolve_icon_path(item)
        if icon_path and os.path.exists(icon_path):
            icon_img.set_from_file(icon_path)
        else:
            icon_img.set_from_icon_name(item.get("icon", "application-x-executable-symbolic"))
        self.add_prefix(icon_img)

        # Progress bar (hidden until update starts)
        self._progress = Gtk.ProgressBar()
        self._progress.set_pulse_step(0.1)
        self._progress.set_visible(False)
        self._progress.set_valign(Gtk.Align.CENTER)
        self._progress.set_size_request(120, -1)
        self.add_suffix(self._progress)

    def set_progress(self, fraction: float):
        self._progress.set_visible(True)
        if fraction < 0:
            self._progress.pulse()
        else:
            self._progress.set_fraction(min(1.0, fraction))

    def set_done(self, success: bool):
        self._progress.set_fraction(1.0 if success else 0.0)
        self._progress.set_visible(True)
        if success:
            # Fade out row after short delay
            GLib.timeout_add(400, self._dissolve)

    def _dissolve(self):
        parent = self.get_parent()
        if parent:
            parent.remove(self)
        return False


class ImageUpdateCard(Gtk.Frame):
    def __init__(self, info: dict, page: UpdatesPage):
        super().__init__()
        self.add_css_class("card")
        self._page = page

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.set_margin_top(12)
        box.set_margin_bottom(12)
        box.set_margin_start(16)
        box.set_margin_end(16)

        title = Gtk.Label(label="Operating System Image")
        title.add_css_class("title-4")
        title.set_halign(Gtk.Align.START)

        desc = Gtk.Label(label=info.get("description", "A new system image is available."))
        desc.add_css_class("body")
        desc.set_halign(Gtk.Align.START)
        desc.set_wrap(True)

        self._progress = Gtk.ProgressBar()
        self._progress.set_pulse_step(0.05)
        self._progress.set_visible(False)

        btn_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self._update_btn = Gtk.Button(label="Update System Image")
        self._update_btn.add_css_class("suggested-action")
        self._update_btn.connect("clicked", lambda *_: page._run_image_update(
            lambda ok: page._on_image_done(ok)))
        btn_row.append(self._update_btn)
        btn_row.append(Gtk.Box(hexpand=True))

        box.append(title)
        box.append(desc)
        box.append(self._progress)
        box.append(btn_row)
        self.set_child(box)

    def set_updating(self):
        self._update_btn.set_sensitive(False)
        self._update_btn.set_label("Updating…")
        self._progress.set_visible(True)
        self._progress.pulse()

        # Keep pulsing until set_progress is called
        def _pulse():
            if self._progress.get_fraction() == 0:
                self._progress.pulse()
                return True
            return False
        GLib.timeout_add(200, _pulse)

    def set_progress(self, fraction: float):
        self._progress.set_visible(True)
        if fraction < 0:
            self._progress.pulse()
        else:
            self._progress.set_fraction(min(1.0, fraction))


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
        return int(m.group(1)) / int(m.group(2))
    m = re.search(r"(\d+)%", line)
    if m:
        return int(m.group(1)) / 100
    return None


def _parse_dnf_progress(line: str) -> float | None:
    m = re.search(r"\[(\d+)/(\d+)\]", line)
    if m:
        return int(m.group(1)) / int(m.group(2))
    return None
