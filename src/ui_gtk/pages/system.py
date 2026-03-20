"""
ui_gtk/pages/system.py — System info page for GTK frontend.
Shows OS image info, overlay packages, rollback, reset overlay, DE switcher.
"""
import re
import subprocess
import threading
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, GLib
from pathlib import Path

from backend import updates


RAKUOS_IMAGES = [
    {"label": "KDE Plasma",  "image_name": "rakuos-kde"},
    {"label": "GNOME",       "image_name": "rakuos-gnome"},
]
GHCR_ORG = "rakuos"


def _parse_image_name(image_full: str) -> tuple[str, bool]:
    try:
        path = image_full.replace("ghcr.io/", "").split(":")[0]
        name = path.split("/")[-1]
        is_nvidia = name.endswith("-nvidia")
        return name, is_nvidia
    except Exception:
        return "", False


def _base_image_name(image_name: str) -> str:
    return image_name.removesuffix("-nvidia")


def _nvidia_image_name(image_name: str) -> str:
    return _base_image_name(image_name) + "-nvidia"


def _get_latest_tag_for(image_name: str) -> str:
    repo_path = f"{GHCR_ORG}/{image_name}"
    try:
        token = updates._get_ghcr_token(repo_path)
        annotation = updates._get_latest_version_annotation(repo_path, token)
        tag = re.sub(r"^latest\.", "", annotation).strip()
        if re.match(r"^\d{8}$", tag):
            return tag
        return ""
    except Exception as e:
        print(f"[system] _get_latest_tag_for {image_name}: {e}")
        return ""


class SystemPage(Gtk.ScrolledWindow):
    def __init__(self, window):
        super().__init__()
        self._win = window
        self.set_vexpand(True)
        self.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self._box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        self._box.set_margin_top(12)
        self._box.set_margin_bottom(24)
        self.set_child(self._box)

        # Spinner while loading
        self._spinner = Gtk.Spinner()
        self._spinner.start()
        self._spinner.set_margin_top(48)
        self._spinner.set_halign(Gtk.Align.CENTER)
        self._box.append(self._spinner)

        GLib.idle_add(self._load_info)

    def _load_info(self):
        threading.Thread(target=self._load_worker, daemon=True).start()
        return False

    def _load_worker(self):
        status = updates.get_system_status()
        pkgs_file = Path("/var/lib/rakuos/packages.list")
        pkgs = []
        if pkgs_file.exists():
            pkgs = [l.strip() for l in pkgs_file.read_text().splitlines()
                    if l.strip() and not l.startswith("#")]
        GLib.idle_add(self._populate, status, pkgs)

    def _populate(self, status: dict, pkgs: list):
        # Clear spinner / old content
        while True:
            child = self._box.get_first_child()
            if child is None:
                break
            self._box.remove(child)

        image_full = status.get("image", "")
        version    = status.get("version", "")
        digest     = status.get("digest", "")
        timestamp  = status.get("timestamp", "")
        current_name, is_nvidia = _parse_image_name(image_full)

        # ── Booted Image card ─────────────────────────────────────────────────
        img_group = Adw.PreferencesGroup()
        img_group.set_title("Booted Image")
        img_group.set_margin_start(12)
        img_group.set_margin_end(12)
        self._box.append(img_group)

        rows = [
            ("Image",     image_full or "—"),
            ("Version",   version    or "—"),
            ("Digest",    (digest[:24] + "…") if digest else "—"),
            ("Timestamp", timestamp  or "—"),
            ("Nvidia",    "Yes" if is_nvidia else "No"),
        ]
        for key, val in rows:
            row = Adw.ActionRow()
            row.set_title(key)
            row.set_subtitle(str(val))
            img_group.add(row)

        # ── Rollback ──────────────────────────────────────────────────────────
        action_group = Adw.PreferencesGroup()
        action_group.set_margin_start(12)
        action_group.set_margin_end(12)
        self._box.append(action_group)

        rollback_row = Adw.ActionRow()
        rollback_row.set_title("Rollback")
        rollback_row.set_subtitle("Revert to the previous OS image")
        rollback_row.set_activatable(True)
        rollback_row.add_suffix(Gtk.Image.new_from_icon_name("go-previous-symbolic"))
        rollback_row.connect("activated", lambda *_: self._confirm_rollback())
        action_group.add(rollback_row)

        # ── DE Switcher ───────────────────────────────────────────────────────
        de_group = Adw.PreferencesGroup()
        de_group.set_title("Desktop Environment")
        de_group.set_margin_start(12)
        de_group.set_margin_end(12)
        self._box.append(de_group)

        self._de_rows = {}
        base_current = _base_image_name(current_name)

        for entry in RAKUOS_IMAGES:
            img_name = entry["image_name"]
            label    = entry["label"]
            is_current = (img_name == base_current)

            row = Adw.ActionRow()
            row.set_title(label + ("  ✓" if is_current else ""))
            row.set_subtitle("Currently running" if is_current else "")
            row.set_activatable(not is_current)

            if not is_current:
                switch_btn = Gtk.Button(label="Switch")
                switch_btn.add_css_class("suggested-action")
                switch_btn.set_valign(Gtk.Align.CENTER)
                target = _nvidia_image_name(img_name) if is_nvidia else img_name
                switch_btn.connect("clicked",
                    lambda *_, t=target, l=label: self._do_switch(t, l))
                row.add_suffix(switch_btn)

            de_group.add(row)

        # ── Overlay packages ──────────────────────────────────────────────────
        ov_group = Adw.PreferencesGroup()
        ov_group.set_title("Package Overlay")
        ov_group.set_margin_start(12)
        ov_group.set_margin_end(12)
        self._box.append(ov_group)

        if pkgs:
            for pkg in sorted(pkgs):
                row = Adw.ActionRow()
                row.set_title(pkg)
                ov_group.add(row)
        else:
            empty_row = Adw.ActionRow()
            empty_row.set_title("No overlay packages installed")
            ov_group.add(empty_row)

        # Reset overlay button
        reset_group = Adw.PreferencesGroup()
        reset_group.set_margin_start(12)
        reset_group.set_margin_end(12)
        self._box.append(reset_group)

        reset_row = Adw.ActionRow()
        reset_row.set_title("Reset Overlay")
        reset_row.set_subtitle("Remove all overlay packages and restore base image")
        reset_row.set_activatable(True)
        reset_row.add_suffix(Gtk.Image.new_from_icon_name("edit-delete-symbolic"))
        reset_row.connect("activated", lambda *_: self._confirm_reset_overlay())
        reset_group.add(reset_row)

        return False

    def _confirm_rollback(self):
        dlg = Adw.MessageDialog(
            heading="Rollback OS Image?",
            body="This will revert to the previous system image. A restart is required.")
        dlg.add_response("cancel", "Cancel")
        dlg.add_response("rollback", "Rollback")
        dlg.set_response_appearance("rollback", Adw.ResponseAppearance.DESTRUCTIVE)
        dlg.connect("response", lambda d, r: self._do_rollback() if r == "rollback" else None)
        dlg.set_transient_for(self._win)
        dlg.present()

    def _do_rollback(self):
        def _worker():
            for _ in updates.rollback_stream():
                pass
            subprocess.Popen(["systemctl", "reboot"])
        threading.Thread(target=_worker, daemon=True).start()

    def _do_switch(self, image_name: str, label: str):
        dlg = Adw.MessageDialog(
            heading=f"Switch to {label}?",
            body=f"This will stage a switch to {image_name}. A reboot is required to apply.")
        dlg.add_response("cancel", "Cancel")
        dlg.add_response("switch", "Switch")
        dlg.set_response_appearance("switch", Adw.ResponseAppearance.SUGGESTED)
        dlg.connect("response",
            lambda d, r: self._run_switch(image_name) if r == "switch" else None)
        dlg.set_transient_for(self._win)
        dlg.present()

    def _run_switch(self, image_name: str):
        def _worker():
            try:
                from backend.updates import _get_ghcr_token, _get_latest_version_annotation
                repo_path = f"{GHCR_ORG}/{image_name}"
                token = _get_ghcr_token(repo_path)
                annotation = _get_latest_version_annotation(repo_path, token)
                tag = re.sub(r"^latest\.", "", annotation).strip()
                if not tag:
                    tag = "latest"
            except Exception:
                tag = "latest"
            target = f"ghcr.io/{GHCR_ORG}/{image_name}:{tag}"
            subprocess.run(["sudo", "bootc", "switch", target])
        threading.Thread(target=_worker, daemon=True).start()

    def _confirm_reset_overlay(self):
        dlg = Adw.MessageDialog(
            heading="Reset Package Overlay?",
            body="This will remove ALL overlay packages. This cannot be undone.")
        dlg.add_response("cancel", "Cancel")
        dlg.add_response("reset", "Reset Overlay")
        dlg.set_response_appearance("reset", Adw.ResponseAppearance.DESTRUCTIVE)
        dlg.connect("response", lambda d, r: self._do_reset_overlay() if r == "reset" else None)
        dlg.set_transient_for(self._win)
        dlg.present()

    def _do_reset_overlay(self):
        def _worker():
            subprocess.run(
                ["pkexec", "rakuos", "reset-overlay", "--confirm"],
                capture_output=True)
            GLib.idle_add(self._load_info)
        threading.Thread(target=_worker, daemon=True).start()
