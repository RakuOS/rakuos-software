"""
ui_gtk/window.py — RakuOS Software Center main window (GTK4/libadwaita).

Layout:
  AdwApplicationWindow
    AdwToolbarView
      AdwHeaderBar          (top)
      Gtk.Box (content)
        Adw.ViewStack       (pages: explore / installed / updates)
      AdwViewSwitcherBar    (bottom tab bar, visible on mobile widths)
    + Adw.NavigationView wraps each tab for push/pop (detail pages)
"""
import os
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, GLib, Gio

from .daemon import UpdateDaemon
from .tray   import RakuOSTray


class MainWindow(Adw.ApplicationWindow):
    def __init__(self, app, rpm_file=None, flatpak_file=None,
                 flatpakref=None, appimage_file=None, **kwargs):
        super().__init__(application=app, **kwargs)
        self.set_title("RakuOS Software")
        self.set_icon_name("system-software-install")
        self.set_default_size(1100, 750)
        self.set_size_request(600, 480)

        # Import pages here to avoid circular imports
        from .pages.home      import HomePage
        from .pages.explore   import ExplorePage
        from .pages.installed import InstalledPage
        from .pages.updates   import UpdatesPage
        from .pages.search    import SearchPage
        from .pages.system    import SystemPage
        from .pages.settings  import SettingsPage
        from .pages.webapps   import WebAppsPage

        # ── Daemon ────────────────────────────────────────────────────────────
        self._daemon = UpdateDaemon()
        self._daemon.connect(
            updates_ready=self._on_updates_ready,
            notify=self._on_notify)

        # ── Tray ──────────────────────────────────────────────────────────────
        self._tray = RakuOSTray(app, self)

        # ── Navigation stack (for detail pages) ──────────────────────────────
        self._nav = Adw.NavigationView()
        self._nav.set_vexpand(True)
        self._nav.set_hexpand(True)

        # ── Root content (tab pages) ──────────────────────────────────────────
        self._stack = Adw.ViewStack()
        self._stack.set_vexpand(True)

        self._home_page     = HomePage(self)
        self._explore_page  = ExplorePage(self)
        self._installed_page= InstalledPage(self)
        self._updates_page  = UpdatesPage(self)
        self._search_page   = SearchPage(self)
        self._system_page   = SystemPage(self)
        self._settings_page = SettingsPage(self)
        self._webapps_page  = WebAppsPage(self)

        # Add pages to stack — icons from freedesktop icon names
        self._stack.add_titled_with_icon(
            self._home_page, "home", "Home", "go-home-symbolic")
        self._stack.add_titled_with_icon(
            self._explore_page, "explore", "Explore", "system-search-symbolic")
        self._stack.add_titled_with_icon(
            self._installed_page, "installed", "Installed", "emblem-system-symbolic")
        self._stack.add_titled_with_icon(
            self._updates_page, "updates", "Updates", "software-update-available-symbolic")

        # Wrap stack in a navigation page (root of nav view)
        root_nav_page = Adw.NavigationPage(title="RakuOS Software", tag="root")
        root_nav_page.set_child(self._build_root_content())
        self._nav.add(root_nav_page)

        # ── Header bar ────────────────────────────────────────────────────────
        header = Adw.HeaderBar()

        # Back button (hidden at root level)
        self._back_btn = Gtk.Button()
        self._back_btn.set_icon_name("go-previous-symbolic")
        self._back_btn.add_css_class("flat")
        self._back_btn.set_visible(False)
        self._back_btn.connect("clicked", lambda *_: self.go_back())
        header.pack_start(self._back_btn)

        # Search button
        self._search_btn = Gtk.ToggleButton()
        self._search_btn.set_icon_name("system-search-symbolic")
        self._search_btn.connect("toggled", self._on_search_toggled)
        header.pack_start(self._search_btn)

        # Menu button (hamburger)
        menu_btn = Gtk.MenuButton()
        menu_btn.set_icon_name("open-menu-symbolic")
        menu_btn.set_menu_model(self._build_menu())
        header.pack_end(menu_btn)

        # View switcher in header
        self._switcher = Adw.ViewSwitcher()
        self._switcher.set_stack(self._stack)
        self._switcher.set_policy(Adw.ViewSwitcherPolicy.WIDE)
        header.set_title_widget(self._switcher)

        # ── Search bar ────────────────────────────────────────────────────────
        self._search_bar  = Gtk.SearchBar()
        self._search_entry= Gtk.SearchEntry()
        self._search_entry.set_hexpand(True)
        self._search_entry.connect("search-changed", self._on_search_changed)
        self._search_entry.connect("activate", self._on_search_activate)
        self._search_bar.set_child(self._search_entry)
        self._search_bar.connect_entry(self._search_entry)

        # ── Toolbar view ──────────────────────────────────────────────────────
        toolbar_view = Adw.ToolbarView()
        toolbar_view.add_top_bar(header)
        toolbar_view.add_top_bar(self._search_bar)
        toolbar_view.set_content(self._nav)
        self.set_content(toolbar_view)

        # Update back button visibility when nav stack changes
        self._nav.connect("notify::visible-page", self._on_nav_changed)

        # Re-check for updates when switching to the updates tab after data was cleared
        self._stack.connect("notify::visible-child", self._on_tab_changed)

        # ── Handle file args ─────────────────────────────────────────────────
        if rpm_file:
            GLib.idle_add(lambda: self._open_local_file("rpm", rpm_file) or False)
        elif flatpak_file:
            GLib.idle_add(lambda: self._open_local_file("flatpak", flatpak_file) or False)
        elif flatpakref:
            GLib.idle_add(lambda: self._open_local_file("flatpakref", flatpakref) or False)
        elif appimage_file:
            GLib.idle_add(lambda: self._open_local_file("appimage", appimage_file) or False)

        self._setup_actions()

        # Start daemon after window shown
        GLib.timeout_add_seconds(1, self._start_daemon)

        # Go to home on close/reopen
        self.connect("show", self._on_show)

    # ── Root content builder ─────────────────────────────────────────────────

    def _build_root_content(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box.append(self._stack)
        return box

    # ── Menu ──────────────────────────────────────────────────────────────────

    def _build_menu(self) -> Gio.Menu:
        menu = Gio.Menu()
        menu.append("Web Apps",    "win.webapps")
        menu.append("System",      "win.system")
        menu.append("Settings",    "win.settings")
        menu.append("About",       "win.about")
        return menu

    def _setup_actions(self):
        actions = {
            "webapps":  lambda *_: self.navigate_to("webapps"),
            "system":   lambda *_: self.navigate_to("system"),
            "settings": lambda *_: self.navigate_to("settings"),
            "about":    lambda *_: self._show_about(),
        }
        for name, cb in actions.items():
            action = Gio.SimpleAction.new(name, None)
            action.connect("activate", cb)
            self.add_action(action)

    # ── Navigation ────────────────────────────────────────────────────────────

    def navigate_to(self, page_name: str, **kwargs):
        """Push a named page onto the navigation stack."""
        if page_name == "webapps":
            self._push_page(self._webapps_page, "Web Apps")
        elif page_name == "system":
            self._push_page(self._system_page, "System")
        elif page_name == "settings":
            self._push_page(self._settings_page, "Settings")
        elif page_name == "detail":
            from .pages.detail import AppDetailPage
            page = AppDetailPage(self)
            page.load_app(kwargs.get("app", {}))
            self._push_page(page, kwargs.get("app", {}).get("name", "App"))
        elif page_name == "appimage_detail":
            from .pages.appimage_detail import AppImageDetailPage
            page = AppImageDetailPage(self)
            page.load_app(kwargs.get("app", {}))
            self._push_page(page, kwargs.get("app", {}).get("name", "App"))

    def _push_page(self, widget, title: str):
        nav_page = Adw.NavigationPage(title=title)
        nav_page.set_child(widget)
        self._nav.push(nav_page)

    def go_back(self):
        self._nav.pop()

    def _on_tab_changed(self, stack, _param):
        page = stack.get_visible_child()
        if page is self._updates_page and not self._updates_page._data:
            self._updates_page._do_check()

    def _on_nav_changed(self, nav, _param):
        depth = len(nav.get_navigation_stack())
        at_root = depth <= 1
        self._back_btn.set_visible(not at_root)
        # Hide tab switcher when on a subpage so title bar has room
        self._switcher.set_visible(at_root)

    # ── Search ────────────────────────────────────────────────────────────────

    def _on_search_toggled(self, btn):
        active = btn.get_active()
        self._search_bar.set_search_mode(active)
        if active:
            self._search_entry.grab_focus()
        else:
            self._search_entry.set_text("")

    def _on_search_changed(self, entry):
        query = entry.get_text().strip()
        if query:
            self._search_page.search(query)
            # Switch stack to search overlay if not already
            if not hasattr(self, "_search_nav_pushed") or not self._search_nav_pushed:
                self._push_page(self._search_page, "Search")
                self._search_nav_pushed = True
        else:
            if getattr(self, "_search_nav_pushed", False):
                self._nav.pop()
                self._search_nav_pushed = False

    def _on_search_activate(self, entry):
        self._on_search_changed(entry)

    # ── Daemon / tray ────────────────────────────────────────────────────────

    def _start_daemon(self):
        self._daemon.start()
        return False

    def _on_updates_ready(self, result: dict):
        total     = result.get("total", 0)
        pkg_count = len(result.get("packages", []))
        fp_count  = len(result.get("flatpak", []))
        img       = result.get("image_available", False)

        parts = []
        if pkg_count:
            parts.append(f"{pkg_count} package{'s' if pkg_count != 1 else ''}")
        if fp_count:
            parts.append(f"{fp_count} Flatpak{'s' if fp_count != 1 else ''}")
        if img:
            parts.append("system image")
        summary = ", ".join(parts) + " available" if parts else ""

        self._updates_page.set_updates(result)
        self._tray.set_updates(total, summary)

        # Update badge on Updates tab
        page = self._stack.get_page(self._updates_page)
        if page:
            page.set_needs_attention(total > 0)
            page.set_badge_number(total)

    def _on_notify(self, title: str, body: str):
        self._tray.notify(title, body)

    # ── Misc ──────────────────────────────────────────────────────────────────

    def _on_show(self, *_):
        """Reset to home tab whenever window is shown."""
        self._stack.set_visible_child_name("home")
        # Pop all pushed nav pages back to root
        while len(self._nav.get_navigation_stack()) > 1:
            self._nav.pop()

    def _open_local_file(self, kind: str, path: str):
        self.navigate_to("detail", app={"_local_file": path, "_local_kind": kind})

    def _show_about(self):
        dialog = Adw.AboutDialog()
        dialog.set_application_name("RakuOS Software")
        dialog.set_developer_name("RakuOS Project")
        dialog.set_version("1.0")
        dialog.set_website("https://rakuos.org")
        dialog.set_issue_url("https://github.com/rakuos/rakuos-software/issues")
        dialog.set_license_type(Gtk.License.GPL_3_0)
        dialog.present()
