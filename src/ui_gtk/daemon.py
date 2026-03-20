"""
ui_gtk/daemon.py — RakuOS Software Center background update daemon (GTK).

Uses GLib.timeout_add instead of QTimer; results posted via GLib.idle_add.
"""
import json
import os
import subprocess
import threading
from gi.repository import GLib

RAKUOS_UPDATE = "/usr/libexec/rakuos/rakuos-update"
SETTINGS_FILE = os.path.expanduser("~/.config/rakuos/software-settings.json")
DEFAULT_INTERVAL = 1440   # minutes
MIN_INTERVAL     = 60


def _load_settings() -> dict:
    try:
        with open(SETTINGS_FILE) as f:
            return json.load(f)
    except Exception:
        return {}


def _run_check(command: str) -> tuple[bool, list]:
    try:
        r = subprocess.run([RAKUOS_UPDATE, command],
                           capture_output=True, text=True, timeout=120)
        if r.returncode not in (0, 1):
            return False, []
        data = json.loads(r.stdout)
        return r.returncode == 0, data.get("updates", [])
    except Exception:
        return False, []


def _run_check_image() -> tuple[bool, dict]:
    try:
        r = subprocess.run([RAKUOS_UPDATE, "check-image"],
                           capture_output=True, text=True, timeout=120)
        data = json.loads(r.stdout)
        return r.returncode == 0, data
    except Exception:
        return False, {}


class UpdateDaemon:
    def __init__(self):
        self._timer_id = None
        self._running  = False
        self._last_result: dict = {}
        self._callbacks: list = []   # (updates_ready_cb,)

    def connect(self, updates_ready=None, notify=None):
        self._callbacks.append((updates_ready, notify))

    def start(self):
        self._reschedule()
        GLib.timeout_add_seconds(30, self._run_checks_once)

    def check_now(self):
        if not self._running:
            GLib.idle_add(self._run_checks)

    def last_result(self) -> dict:
        return self._last_result

    # ── internals ──────────────────────────────────────────────────────────────

    def _reschedule(self):
        if self._timer_id:
            GLib.source_remove(self._timer_id)
            self._timer_id = None
        settings = _load_settings()
        interval_min = settings.get("update_interval", DEFAULT_INTERVAL)
        if interval_min > 0:
            interval_min = max(interval_min, MIN_INTERVAL)
            self._timer_id = GLib.timeout_add_seconds(
                interval_min * 60, self._run_checks)

    def _run_checks_once(self):
        """Called once by the startup delay timer."""
        self._run_checks()
        return False  # don't repeat

    def _run_checks(self):
        if self._running:
            return False
        self._running = True

        def _worker():
            settings = _load_settings()
            pkg, fp, img_info = [], [], {}
            img_avail = False
            ai_updates = []

            if settings.get("auto_check_packages", True):
                _, pkg = _run_check("check")
            if settings.get("auto_check_flatpak", True):
                _, fp  = _run_check("check-flatpak")
            if settings.get("auto_check_image", True):
                img_avail, img_info = _run_check_image()
            if settings.get("auto_check_appimages", True):
                try:
                    from backend import appimages as _ai
                    ai_updates = _ai.check_updates()
                except Exception:
                    pass

            result = {
                "packages":        pkg,
                "flatpak":         fp,
                "appimages":       ai_updates,
                "image_available": img_avail,
                "image_info":      img_info,
                "total": len(pkg) + len(fp) + len(ai_updates) + (1 if img_avail else 0),
            }
            GLib.idle_add(self._on_done, result)

        threading.Thread(target=_worker, daemon=True).start()
        return False  # GLib timers: return True to repeat, but we reschedule manually

    def _on_done(self, result: dict):
        self._running      = False
        self._last_result  = result
        self._reschedule()

        for upd_cb, notify_cb in self._callbacks:
            if upd_cb:
                upd_cb(result)

        total = result["total"]
        if total and notify_cb:
            parts = []
            if result["packages"]:
                n = len(result["packages"])
                parts.append(f"{n} package update{'s' if n!=1 else ''}")
            if result["flatpak"]:
                n = len(result["flatpak"])
                parts.append(f"{n} Flatpak update{'s' if n!=1 else ''}")
            if result["image_available"]:
                parts.append("System image update available")
            for _, notify_cb in self._callbacks:
                if notify_cb:
                    notify_cb("Updates Available", "\n".join(parts))
        return False
