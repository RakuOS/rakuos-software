"""
ui_gtk/tray.py — System tray via StatusNotifierItem DBus protocol.

Works natively with GNOME (gnome-shell-extension-appindicator),
KDE Plasma, and any StatusNotifierWatcher-compatible compositor.
Falls back to notify-send-only if DBus is unavailable.
"""
import os
import subprocess
from gi.repository import GLib

_dbus_ok = False
try:
    import dbus
    import dbus.service
    from dbus.mainloop.glib import DBusGMainLoop
    # Must be set before any SessionBus() call; safe to call once.
    try:
        DBusGMainLoop(set_as_default=True)
    except Exception:
        pass
    _dbus_ok = True
except ImportError:
    pass

_SNI_IFACE = "org.kde.StatusNotifierItem"
_SNW_NAME  = "org.kde.StatusNotifierWatcher"
_SNW_PATH  = "/StatusNotifierWatcher"


class _StatusNotifierItem(dbus.service.Object):
    """Minimal StatusNotifierItem DBus object."""

    def __init__(self, bus, service_name, on_activate):
        self._activate_cb = on_activate
        self._icon = "system-software-update"
        self._tooltip = "RakuOS Software Center"
        bn = dbus.service.BusName(service_name, bus, allow_replacement=True)
        super().__init__(bn, "/StatusNotifierItem")

    # ── Properties ────────────────────────────────────────────────────────────

    def _props(self) -> dict:
        return {
            "Id":              dbus.String("rakuos-software"),
            "Title":           dbus.String("RakuOS Software"),
            "Status":          dbus.String("Active"),
            "Category":        dbus.String("ApplicationStatus"),
            "IconName":        dbus.String(self._icon),
            "ToolTipTitle":    dbus.String(self._tooltip),
            "ToolTipBody":     dbus.String(""),
            "ToolTipIconName": dbus.String(self._icon),
            "ItemIsMenu":      dbus.Boolean(False),
        }

    @dbus.service.method(dbus.PROPERTIES_IFACE, in_signature="ss", out_signature="v")
    def Get(self, iface, prop):
        return self._props().get(prop, dbus.String(""))

    @dbus.service.method(dbus.PROPERTIES_IFACE, in_signature="s", out_signature="a{sv}")
    def GetAll(self, iface):
        return dbus.Dictionary(self._props(), signature="sv")

    @dbus.service.signal(dbus.PROPERTIES_IFACE, signature="sa{sv}as")
    def PropertiesChanged(self, iface, changed, invalidated):
        pass

    # ── Methods ───────────────────────────────────────────────────────────────

    @dbus.service.method(_SNI_IFACE, in_signature="ii")
    def Activate(self, x, y):
        if self._activate_cb:
            GLib.idle_add(self._activate_cb)

    @dbus.service.method(_SNI_IFACE, in_signature="ii")
    def SecondaryActivate(self, x, y):
        if self._activate_cb:
            GLib.idle_add(self._activate_cb)

    @dbus.service.method(_SNI_IFACE, in_signature="is")
    def Scroll(self, delta, orientation):
        pass

    # ── Signals ───────────────────────────────────────────────────────────────

    @dbus.service.signal(_SNI_IFACE)
    def NewIcon(self):
        pass

    @dbus.service.signal(_SNI_IFACE)
    def NewStatus(self, status: str):
        pass

    # ── Helpers ───────────────────────────────────────────────────────────────

    def set_icon(self, icon_name: str):
        self._icon = icon_name
        try:
            self.PropertiesChanged(
                _SNI_IFACE, {"IconName": dbus.String(icon_name)}, [])
            self.NewIcon()
        except Exception:
            pass


class RakuOSTray:
    def __init__(self, app, main_window):
        self._app  = app
        self._win  = main_window
        self._sni  = None
        self._update_count = 0

        if not _dbus_ok:
            return

        try:
            bus = dbus.SessionBus()
            service_name = f"org.kde.StatusNotifierItem-{os.getpid()}-1"
            self._sni = _StatusNotifierItem(bus, service_name, self._show_window)

            # Register with the StatusNotifierWatcher (extension listens for this)
            try:
                watcher = bus.get_object(_SNW_NAME, _SNW_PATH)
                watcher.RegisterStatusNotifierItem(
                    service_name,
                    dbus_interface=_SNW_NAME)
            except Exception:
                # Watcher may not exist yet; the extension will still pick up
                # the service name from the session bus when it starts.
                pass

        except Exception:
            self._sni = None

    def _show_window(self):
        self._win.present()

    def set_updates(self, count: int, summary: str = ""):
        self._update_count = count
        if self._sni:
            icon = "software-update-available" if count > 0 else "system-software-update"
            self._sni.set_icon(icon)

    def notify(self, title: str, body: str):
        try:
            subprocess.Popen(
                ["notify-send", "-a", "RakuOS Software", title, body])
        except Exception:
            pass
