"""
ui_gtk/pages/settings.py — Settings page for GTK frontend.
"""
import json
import os
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk

SETTINGS_FILE = os.path.expanduser("~/.config/rakuos/software-settings.json")


def _load() -> dict:
    try:
        with open(SETTINGS_FILE) as f:
            return json.load(f)
    except Exception:
        return {}


def _save(data: dict):
    os.makedirs(os.path.dirname(SETTINGS_FILE), exist_ok=True)
    with open(SETTINGS_FILE, "w") as f:
        json.dump(data, f, indent=2)


class SettingsPage(Gtk.ScrolledWindow):
    def __init__(self, window):
        super().__init__()
        self._win = window
        self.set_vexpand(True)
        self.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self._settings = _load()

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(12)
        box.set_margin_bottom(24)
        self.set_child(box)

        # Update checks group
        updates_group = Adw.PreferencesGroup()
        updates_group.set_title("Update Checks")
        updates_group.set_margin_start(12)
        updates_group.set_margin_end(12)
        box.append(updates_group)

        self._add_switch(updates_group, "Check for Package Updates",
                         "auto_check_packages", True)
        self._add_switch(updates_group, "Check for Flatpak Updates",
                         "auto_check_flatpak", True)
        self._add_switch(updates_group, "Check for AppImage Updates",
                         "auto_check_appimages", True)
        self._add_switch(updates_group, "Check for OS Image Updates",
                         "auto_check_image", True)
        self._add_switch(updates_group, "Auto-install Package/Flatpak Updates",
                         "auto_update", False)

        # Check interval
        interval_group = Adw.PreferencesGroup()
        interval_group.set_title("Check Interval")
        interval_group.set_margin_start(12)
        interval_group.set_margin_end(12)
        box.append(interval_group)

        interval_row = Adw.ComboRow()
        interval_row.set_title("Check Frequency")
        interval_row.set_model(Gtk.StringList.new([
            "Manual Only", "Every 6 Hours", "Every 12 Hours",
            "Daily (Default)", "Weekly"
        ]))
        intervals = [0, 360, 720, 1440, 10080]
        current = self._settings.get("update_interval", 1440)
        idx = intervals.index(current) if current in intervals else 3
        interval_row.set_selected(idx)
        interval_row.connect("notify::selected",
                             lambda r, _: self._on_interval(r, intervals))
        interval_group.add(interval_row)

    def _add_switch(self, group, title: str, key: str, default: bool):
        row = Adw.SwitchRow()
        row.set_title(title)
        row.set_active(self._settings.get(key, default))
        row.connect("notify::active",
                    lambda r, _: self._on_toggle(key, r.get_active()))
        group.add(row)

    def _on_toggle(self, key: str, value: bool):
        self._settings[key] = value
        _save(self._settings)

    def _on_interval(self, row, intervals: list):
        idx = row.get_selected()
        if 0 <= idx < len(intervals):
            self._settings["update_interval"] = intervals[idx]
            _save(self._settings)
