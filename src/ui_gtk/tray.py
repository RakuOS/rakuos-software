"""
ui_gtk/tray.py — System tray via StatusNotifierItem + com.canonical.dbusmenu.

Works with gnome-shell-extension-appindicator, KDE Plasma, and any
StatusNotifierWatcher-compatible compositor.

Provides the same right-click context menu as the Qt tray:
  • Show Software Center
  • ─────────────────
  • <update summary>   (disabled label)
  • ─────────────────
  • Check for Updates
  • ─────────────────
  • Exit
"""
import os
import subprocess
from gi.repository import GLib

_dbus_ok = False
try:
    import dbus
    import dbus.service
    from dbus.mainloop.glib import DBusGMainLoop
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
_MENU_IFACE = "com.canonical.dbusmenu"
_MENU_PATH  = "/MenuBar"

# Menu item IDs
_ID_SHOW   = 1
_ID_SEP1   = 2
_ID_UPDATES= 3
_ID_SEP2   = 4
_ID_CHECK  = 5
_ID_SEP3   = 6
_ID_EXIT   = 7


class _DbusMenu(dbus.service.Object):
    """Minimal com.canonical.dbusmenu server for the tray context menu."""

    def __init__(self, bus, on_show, on_check, on_quit):
        super().__init__(bus, _MENU_PATH)
        self._revision     = 1
        self._on_show      = on_show
        self._on_check     = on_check
        self._on_quit      = on_quit
        self._update_label = "No updates available"
        self._update_enabled = False

    def _item(self, id: int, label: str = "", enabled: bool = True,
              separator: bool = False) -> tuple:
        props = {
            "label":   dbus.String(label),
            "enabled": dbus.Boolean(enabled),
            "visible": dbus.Boolean(True),
            "type":    dbus.String("separator" if separator else "standard"),
        }
        return (dbus.Int32(id),
                dbus.Dictionary(props, signature="sv"),
                dbus.Array([], signature="v"))

    def _layout_root(self):
        children = [
            self._item(_ID_SHOW,    "Show Software Center"),
            self._item(_ID_SEP1,    separator=True),
            self._item(_ID_UPDATES, self._update_label, enabled=self._update_enabled),
            self._item(_ID_SEP2,    separator=True),
            self._item(_ID_CHECK,   "Check for Updates"),
            self._item(_ID_SEP3,    separator=True),
            self._item(_ID_EXIT,    "Exit"),
        ]
        # Each child must be wrapped in a dbus.Struct for the "av" container
        wrapped = dbus.Array(
            [dbus.Struct(c, signature="(ia{sv}av)") for c in children],
            signature="v")
        root_props = dbus.Dictionary(
            {"label": dbus.String(""),
             "children-display": dbus.String("submenu")},
            signature="sv")
        return (dbus.Int32(0), root_props, wrapped)

    @dbus.service.method(_MENU_IFACE,
                         in_signature="iias", out_signature="u(ia{sv}av)")
    def GetLayout(self, parentId, recursionDepth, propertyNames):
        return (dbus.UInt32(self._revision), self._layout_root())

    @dbus.service.method(_MENU_IFACE,
                         in_signature="aias", out_signature="a(ia{sv})")
    def GetGroupProperties(self, ids, propertyNames):
        return dbus.Array([], signature="(ia{sv})")

    @dbus.service.method(_MENU_IFACE, in_signature="i", out_signature="b")
    def AboutToShow(self, id):
        return dbus.Boolean(False)

    @dbus.service.method(_MENU_IFACE, in_signature="ai", out_signature="ab")
    def AboutToShowGroup(self, ids):
        return dbus.Array([dbus.Boolean(False)] * len(ids), signature="b")

    @dbus.service.method(_MENU_IFACE, in_signature="isvu")
    def Event(self, id, eventId, data, timestamp):
        if eventId != "clicked":
            return
        if id == _ID_SHOW:
            GLib.idle_add(self._on_show)
        elif id == _ID_CHECK:
            GLib.idle_add(self._on_check)
        elif id == _ID_EXIT:
            GLib.idle_add(self._on_quit)

    @dbus.service.method(_MENU_IFACE, in_signature="a(isvu)")
    def EventGroup(self, events):
        for (id, eventId, data, timestamp) in events:
            self.Event(id, eventId, data, timestamp)

    @dbus.service.signal(_MENU_IFACE, signature="a(ia{sv})a(ias)")
    def ItemsPropertiesUpdated(self, updatedProps, removedProps):
        pass

    @dbus.service.signal(_MENU_IFACE, signature="ui")
    def LayoutUpdated(self, revision, parent):
        pass

    def set_updates(self, count: int, summary: str):
        if count > 0:
            self._update_label   = summary or f"{count} update{'s' if count != 1 else ''} available"
            self._update_enabled = True
        else:
            self._update_label   = "No updates available"
            self._update_enabled = False
        self._revision += 1
        try:
            self.ItemsPropertiesUpdated(
                dbus.Array(
                    [(dbus.Int32(_ID_UPDATES),
                      dbus.Dictionary(
                          {"label":   dbus.String(self._update_label),
                           "enabled": dbus.Boolean(self._update_enabled)},
                          signature="sv"))],
                    signature="(ia{sv})"),
                dbus.Array([], signature="(ias)"))
            self.LayoutUpdated(dbus.UInt32(self._revision), dbus.Int32(0))
        except Exception:
            pass


class _StatusNotifierItem(dbus.service.Object):
    """StatusNotifierItem DBus object with context-menu support."""

    def __init__(self, bus, service_name, menu_path: str, on_activate):
        self._activate_cb = on_activate
        self._icon        = "system-software-update"
        self._tooltip     = "RakuOS Software"
        self._menu_path   = menu_path
        bn = dbus.service.BusName(service_name, bus, allow_replacement=True)
        super().__init__(bn, "/StatusNotifierItem")

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
            "ItemIsMenu":      dbus.Boolean(True),
            "Menu":            dbus.ObjectPath(self._menu_path),
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

    @dbus.service.method(_SNI_IFACE, in_signature="ii")
    def Activate(self, x, y):
        if self._activate_cb:
            GLib.idle_add(self._activate_cb)

    @dbus.service.method(_SNI_IFACE, in_signature="ii")
    def SecondaryActivate(self, x, y):
        if self._activate_cb:
            GLib.idle_add(self._activate_cb)

    @dbus.service.method(_SNI_IFACE, in_signature="ii")
    def ContextMenu(self, x, y):
        # gnome-shell-extension-appindicator handles this via DBusMenu;
        # some other bars may call this directly — show window as fallback.
        if self._activate_cb:
            GLib.idle_add(self._activate_cb)

    @dbus.service.method(_SNI_IFACE, in_signature="is")
    def Scroll(self, delta, orientation):
        pass

    @dbus.service.signal(_SNI_IFACE)
    def NewIcon(self):
        pass

    @dbus.service.signal(_SNI_IFACE)
    def NewStatus(self, status: str):
        pass

    def set_icon(self, icon_name: str):
        self._icon = icon_name
        try:
            self.PropertiesChanged(
                _SNI_IFACE, {"IconName": dbus.String(icon_name)}, [])
            self.NewIcon()
        except Exception:
            pass

    def set_tooltip(self, text: str):
        self._tooltip = text
        try:
            self.PropertiesChanged(
                _SNI_IFACE, {"ToolTipTitle": dbus.String(text)}, [])
        except Exception:
            pass


class RakuOSTray:
    def __init__(self, app, main_window):
        self._app         = app
        self._win         = main_window
        self._sni         = None
        self._menu        = None
        self._has_support = False

        if not _dbus_ok:
            return

        try:
            bus          = dbus.SessionBus()
            service_name = f"org.kde.StatusNotifierItem-{os.getpid()}-1"

            # Build context menu first so its path is known
            self._menu = _DbusMenu(
                bus,
                on_show  = self._show_window,
                on_check = self._check_updates,
                on_quit  = self._quit,
            )

            self._sni = _StatusNotifierItem(
                bus, service_name, _MENU_PATH, self._show_window)

            # Register with StatusNotifierWatcher
            try:
                watcher = bus.get_object(_SNW_NAME, _SNW_PATH)
                watcher.RegisterStatusNotifierItem(
                    service_name, dbus_interface=_SNW_NAME)
                self._has_support = True
            except Exception:
                # Watcher absent; extension may still discover us later
                self._has_support = True   # optimistic — watcher may appear

        except Exception:
            self._sni  = None
            self._menu = None

    @property
    def has_support(self) -> bool:
        """True if SNI registered (tray icon should be visible)."""
        return self._has_support

    def _show_window(self):
        self._win.present()

    def _check_updates(self):
        daemon = getattr(self._win, "_daemon", None)
        if daemon:
            daemon.check_now()

    def _quit(self):
        self._app.quit()

    def set_updates(self, count: int, summary: str = ""):
        if self._sni:
            icon = "software-update-available" if count > 0 else "system-software-update"
            self._sni.set_icon(icon)
            tip  = f"RakuOS Software — {count} update{'s' if count != 1 else ''} available" \
                   if count > 0 else "RakuOS Software — Up to date"
            self._sni.set_tooltip(tip)
        if self._menu:
            self._menu.set_updates(count, summary)

    def notify(self, title: str, body: str):
        try:
            subprocess.Popen(
                ["notify-send", "-a", "RakuOS Software",
                 "--icon=system-software-update", title, body])
        except Exception:
            pass
