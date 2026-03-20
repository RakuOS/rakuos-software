"""
ui_gtk/pages/explore.py — Explore/Browse page for GTK frontend.

Shows top-level category grid → subcategory list → app grid.
"""
import os
import threading
import gi
gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, GLib, Pango

from backend import packages as _pkg
from ..icon_loader import resolve_icon_path

CATEGORY_TREE = [
    ("Games",        "Game",        "applications-games-symbolic", [
        ("Action",       "ActionGame"),   ("Adventure",    "AdventureGame"),
        ("Arcade",       "ArcadeGame"),   ("Board Games",  "BoardGame"),
        ("Emulators",    "Emulator"),     ("Role Playing", "RolePlaying"),
        ("Shooter",      "Shooter"),      ("Strategy",     "StrategyGame"),
    ]),
    ("Internet",     "Network",     "applications-internet-symbolic", [
        ("Web Browsers", "WebBrowser"),   ("Email",        "Email"),
        ("Chat &amp; IM",    "InstantMessaging"), ("File Sharing","FileTransfer"),
        ("News &amp; RSS",   "News"),
    ]),
    ("Audio &amp; Video","AudioVideo",  "applications-multimedia-symbolic", [
        ("Music",        "Music"),        ("Video",        "Video"),
        ("Editors",      "AudioVideoEditing"), ("Podcasts","Podcasting"),
    ]),
    ("Graphics",     "Graphics",    "applications-graphics-symbolic", [
        ("2D Editors",   "2DGraphics"),   ("3D Editors",   "3DGraphics"),
        ("Photography",  "Photography"),  ("Viewers",      "Viewer"),
    ]),
    ("Productivity", "Office",      "applications-office-symbolic", [
        ("Office Suite", "WordProcessor"),("Spreadsheets", "Spreadsheet"),
        ("Presentation", "Presentation"), ("Calendar",     "Calendar"),
        ("Notes",        "Notes"),
    ]),
    ("Development",  "Development", "applications-development-symbolic", [
        ("IDEs",         "IDE"),          ("Version Control","RevisionControl"),
        ("Debuggers",    "Debugger"),     ("Web Dev",      "WebDevelopment"),
    ]),
    ("System",       "System",      "applications-system-symbolic", [
        ("File Managers","FileManager"),  ("Terminal",     "TerminalEmulator"),
        ("Monitors",     "Monitor"),      ("Security",     "Security"),
    ]),
    ("Utilities",    "Utility",     "applications-utilities-symbolic", [
        ("Archiving",    "Archiving"),    ("Calculators",  "Calculator"),
        ("Clocks",       "Clock"),        ("Text Editors", "TextEditor"),
    ]),
    ("Education",    "Education",   "applications-science-symbolic", [
        ("Languages",    "Languages"),    ("Mathematics",  "Math"),
        ("Science",      "Science"),
    ]),
]


class ExplorePage(Gtk.Box):
    def __init__(self, window):
        super().__init__(orientation=Gtk.Orientation.VERTICAL)
        self._win = window
        self._set_vexpand(True)

        # Stack: "categories" | "subcategories" | "apps"
        self._nav_stack = Gtk.Stack()
        self._nav_stack.set_vexpand(True)
        self._nav_stack.set_transition_type(Gtk.StackTransitionType.SLIDE_LEFT_RIGHT)
        self.append(self._nav_stack)

        self._build_category_view()

    def _set_vexpand(self, v):
        self.set_vexpand(v)

    # ── Top-level categories ──────────────────────────────────────────────────

    def _build_category_view(self):
        scroll = Gtk.ScrolledWindow()
        scroll.set_vexpand(True)
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        box.set_margin_top(12)
        box.set_margin_bottom(12)

        list_box = Gtk.ListBox()
        list_box.set_selection_mode(Gtk.SelectionMode.NONE)
        list_box.add_css_class("boxed-list")
        list_box.set_margin_start(12)
        list_box.set_margin_end(12)

        for label, cat, icon, subcats in CATEGORY_TREE:
            row = Adw.ActionRow()
            row.set_title(label)
            row.set_activatable(True)
            row.add_prefix(Gtk.Image.new_from_icon_name(icon))
            row.add_suffix(Gtk.Image.new_from_icon_name("go-next-symbolic"))
            row.connect("activated",
                        lambda r, l=label, c=cat, s=subcats: self.browse_category(c, l, s))
            list_box.append(row)

        box.append(list_box)
        scroll.set_child(box)
        self._nav_stack.add_named(scroll, "categories")

    # ── Subcategories ─────────────────────────────────────────────────────────

    def browse_category(self, cat: str, label: str, subcats: list = None):
        """Show subcategory list for a top-level category."""
        # Remove old subcat page if exists
        old = self._nav_stack.get_child_by_name("subcategories")
        if old:
            self._nav_stack.remove(old)

        scroll = Gtk.ScrolledWindow()
        scroll.set_vexpand(True)
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        outer.set_margin_top(12)
        outer.set_margin_bottom(12)

        # Back button row
        back_btn = Gtk.Button(label="← All Categories")
        back_btn.add_css_class("flat")
        back_btn.set_halign(Gtk.Align.START)
        back_btn.set_margin_start(12)
        back_btn.connect("clicked", lambda *_: self._nav_stack.set_visible_child_name("categories"))
        outer.append(back_btn)

        # "All <Category>" row at top
        list_box = Gtk.ListBox()
        list_box.set_selection_mode(Gtk.SelectionMode.NONE)
        list_box.add_css_class("boxed-list")
        list_box.set_margin_start(12)
        list_box.set_margin_end(12)

        all_row = Adw.ActionRow()
        all_row.set_title(f"All {label}")
        all_row.set_activatable(True)
        all_row.add_suffix(Gtk.Image.new_from_icon_name("go-next-symbolic"))
        all_row.connect("activated", lambda *_: self._show_apps(cat, f"All {label}"))
        list_box.append(all_row)

        if subcats:
            for sub_label, sub_cat in subcats:
                row = Adw.ActionRow()
                row.set_title(sub_label)
                row.set_activatable(True)
                row.add_suffix(Gtk.Image.new_from_icon_name("go-next-symbolic"))
                row.connect("activated",
                            lambda r, sl=sub_label, sc=sub_cat: self._show_apps(sc, sl))
                list_box.append(row)

        outer.append(list_box)
        scroll.set_child(outer)
        self._nav_stack.add_named(scroll, "subcategories")
        self._nav_stack.set_visible_child_name("subcategories")

    # ── App grid ──────────────────────────────────────────────────────────────

    def _show_apps(self, cat: str, label: str, back_to_home: bool = False):
        old = self._nav_stack.get_child_by_name("apps")
        if old:
            self._nav_stack.remove(old)

        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        outer.set_vexpand(True)

        # Back button — goes to Home when launched from Home, else to subcategories
        def _go_back(*_):
            if back_to_home:
                self._nav_stack.set_visible_child_name("categories")
                self._win._stack.set_visible_child_name("home")
            else:
                self._nav_stack.set_visible_child_name("subcategories")

        back_btn = Gtk.Button(label="← Home" if back_to_home else "← Categories")
        back_btn.add_css_class("flat")
        back_btn.set_halign(Gtk.Align.START)
        back_btn.set_margin_start(12)
        back_btn.set_margin_top(8)
        back_btn.connect("clicked", _go_back)
        outer.append(back_btn)

        scroll = Gtk.ScrolledWindow()
        scroll.set_vexpand(True)
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self._apps_flow = Gtk.FlowBox()
        self._apps_flow.set_homogeneous(True)
        self._apps_flow.set_min_children_per_line(2)
        self._apps_flow.set_max_children_per_line(4)
        self._apps_flow.set_column_spacing(8)
        self._apps_flow.set_row_spacing(8)
        self._apps_flow.set_margin_start(12)
        self._apps_flow.set_margin_end(12)
        self._apps_flow.set_margin_top(8)
        self._apps_flow.set_margin_bottom(12)
        self._apps_flow.set_selection_mode(Gtk.SelectionMode.NONE)

        spinner = Gtk.Spinner()
        spinner.start()
        spinner.set_halign(Gtk.Align.CENTER)
        spinner.set_valign(Gtk.Align.CENTER)
        spinner.set_size_request(-1, 80)
        self._apps_flow.append(spinner)

        scroll.set_child(self._apps_flow)
        outer.append(scroll)
        self._nav_stack.add_named(outer, "apps")
        self._nav_stack.set_visible_child_name("apps")

        def _worker():
            try:
                result = _pkg.get_by_category(cat, limit=60)
                apps = result.get("items", []) if isinstance(result, dict) else result
            except Exception:
                apps = []
            GLib.idle_add(self._populate_apps, apps)

        threading.Thread(target=_worker, daemon=True).start()

    def _populate_apps(self, apps: list):
        # Clear spinner
        while True:
            child = self._apps_flow.get_first_child()
            if child is None:
                break
            self._apps_flow.remove(child)

        if not apps:
            lbl = Gtk.Label(label="No apps found")
            lbl.add_css_class("dim-label")
            self._apps_flow.append(lbl)
            return False

        for app in apps:
            card = self._make_app_card(app)
            self._apps_flow.append(card)
        return False

    def _make_app_card(self, app: dict) -> Gtk.Widget:
        btn = Gtk.Button()
        btn.add_css_class("card")
        btn.set_hexpand(True)

        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.set_margin_top(10)
        box.set_margin_bottom(10)
        box.set_margin_start(12)
        box.set_margin_end(12)

        icon_img = Gtk.Image()
        icon_img.set_pixel_size(48)
        icon_path = resolve_icon_path(app)
        if icon_path and os.path.exists(icon_path):
            icon_img.set_from_file(icon_path)
        else:
            icon_img.set_from_icon_name(app.get("icon", "application-x-executable-symbolic"))

        text_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2,
                           valign=Gtk.Align.CENTER)
        name_lbl = Gtk.Label(label=app.get("name", ""))
        name_lbl.set_halign(Gtk.Align.START)
        name_lbl.add_css_class("body")
        name_lbl.set_ellipsize(Pango.EllipsizeMode.END)
        name_lbl.set_max_width_chars(20)

        sum_lbl = Gtk.Label(label=app.get("summary", ""))
        sum_lbl.set_halign(Gtk.Align.START)
        sum_lbl.add_css_class("caption")
        sum_lbl.add_css_class("dim-label")
        sum_lbl.set_ellipsize(Pango.EllipsizeMode.END)
        sum_lbl.set_max_width_chars(28)

        text_box.append(name_lbl)
        text_box.append(sum_lbl)
        box.append(icon_img)
        box.append(text_box)
        btn.set_child(box)

        btn.connect("clicked", lambda *_: self._win.navigate_to("detail", app=app))
        return btn
