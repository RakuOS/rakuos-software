"""
pages/updates.py — Updates page (Discover-style).

Groups (only shown when updates exist):
  1. Applications  — GUI RPMs + Flatpak apps combined, flatpaks tagged
  2. Add-ons       — Flatpak runtimes / extensions
  3. System Deps   — Non-GUI overlay RPM dependencies
  4. OS Image      — bootc image update

Top bar: "Check for Updates" (always) + "Update All" (when updates exist)
Empty state: large green checkmark with "Check for Updates" top-right.

Per-package rows have inline progress bars during update.
Rows dissolve when complete; terminal only shown on error.
OS image update always runs last; success → reboot-required screen.
"""

import re as _re
import subprocess
from PyQt6.QtWidgets import (
    QWidget, QVBoxLayout, QScrollArea, QFrame, QLabel,
    QHBoxLayout, QPushButton, QProgressBar, QSizePolicy,
)
from PyQt6.QtCore import pyqtSignal, Qt, QTimer
from PyQt6.QtGui import QPixmap

from ..workers import Worker, StreamWorker
from ..widgets import SectionTitle, LoadingWidget, TerminalWidget, IconWidget, hline
from ..theme import dimmed
from backend import flatpak, updates as upd


# ── Progress parsers ──────────────────────────────────────────────────────────

def _parse_bootc_progress(line: str) -> int | None:
    """Parse 'layers[N/M]' from bootc output → 0-100."""
    m = _re.search(r'layers\[(\d+)/(\d+)\]', line)
    if m:
        n, total = int(m.group(1)), int(m.group(2))
        if total > 0:
            return min(int(n * 100 / total), 100)
    return None


def _parse_flatpak_progress(line: str) -> int | None:
    """Parse percentage or [N/M] from flatpak output → 0-100."""
    m = _re.search(r'(\d+)%', line)
    if m:
        return min(int(m.group(1)), 100)
    m = _re.search(r'\[(\d+)/(\d+)\]', line)
    if m:
        n, total = int(m.group(1)), int(m.group(2))
        if total > 0:
            return min(int(n * 100 / total), 100)
    return None


def _parse_dnf_progress(line: str) -> int | None:
    """Parse [N/M] transaction steps from dnf5 output → 0-100."""
    m = _re.search(r'\[(\d+)/(\d+)\]', line)
    if m:
        n, total = int(m.group(1)), int(m.group(2))
        if total > 0:
            return min(int(n * 100 / total), 100)
    return None


# ── Per-package row ───────────────────────────────────────────────────────────

class PackageUpdateRow(QFrame):
    update_clicked = pyqtSignal(dict)

    def __init__(self, pkg: dict, show_icon: bool = False, parent=None):
        super().__init__(parent)
        self._pkg = pkg
        self._alive = True
        self.setObjectName("updateRow")
        self.setFrameShape(QFrame.Shape.NoFrame)

        hl = QHBoxLayout(self)
        hl.setContentsMargins(4, 8, 8, 8)
        hl.setSpacing(10)

        if show_icon:
            icon_w = IconWidget(size=36)
            if pkg.get("is_appimage") and pkg.get("icon_path"):
                pix = QPixmap(pkg["icon_path"]).scaled(
                    36, 36,
                    Qt.AspectRatioMode.KeepAspectRatio,
                    Qt.TransformationMode.SmoothTransformation,
                )
                if not pix.isNull():
                    icon_w.setPixmap(pix)
                    icon_w.setText("")
            else:
                app_id = pkg.get("app_id", "") or pkg.get("id", "")
                icon_w.set_icon_name(
                    pkg.get("icon", ""),
                    app_id=app_id,
                    pkg_name=pkg.get("name", ""),
                    flatpak_id=app_id if pkg.get("is_flatpak") else "",
                )
            hl.addWidget(icon_w)

        info_col = QVBoxLayout()
        info_col.setSpacing(1)

        name_lbl = QLabel(pkg.get("name") or pkg.get("app_id", ""))
        nf = name_lbl.font()
        nf.setPointSize(nf.pointSize() + 1)
        name_lbl.setFont(nf)
        info_col.addWidget(name_lbl)

        ver_row = QHBoxLayout()
        ver_row.setSpacing(6)
        ver = pkg.get("version", "")
        if ver:
            ver_row.addWidget(dimmed(QLabel(f"→  {ver}")))
        if pkg.get("is_flatpak"):
            badge = QLabel("Flatpak")
            badge.setStyleSheet(
                "QLabel { background: rgba(100,180,255,0.18); color: #60aaff;"
                " border-radius: 4px; padding: 1px 6px; font-size: 10px; }")
            ver_row.addWidget(badge)
        if pkg.get("is_appimage"):
            badge = QLabel("AppImage")
            badge.setStyleSheet(
                "QLabel { background: rgba(255,180,50,0.2); color: #ffb432;"
                " border-radius: 4px; padding: 1px 6px; font-size: 10px; }")
            ver_row.addWidget(badge)
        ver_row.addStretch()
        info_col.addLayout(ver_row)
        hl.addLayout(info_col, stretch=1)

        self._progress = QProgressBar()
        self._progress.setRange(0, 0)
        self._progress.setFixedHeight(6)
        self._progress.setTextVisible(False)
        self._progress.setSizePolicy(
            QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        self._progress.setStyleSheet(
            "QProgressBar { border: none; background: rgba(128,128,128,0.2);"
            " border-radius: 3px; }"
            "QProgressBar::chunk { background: #4caf50; border-radius: 3px; }"
        )
        self._progress.hide()
        hl.addWidget(self._progress, stretch=1)

        self._status = QLabel("")
        self._status.hide()
        hl.addWidget(self._status)

        self._btn = QPushButton("Update")
        self._btn.setFixedWidth(88)
        self._btn.clicked.connect(lambda: self.update_clicked.emit(self._pkg))
        hl.addWidget(self._btn)

    def set_updating(self):
        self._btn.hide()
        self._progress.setRange(0, 0)
        self._progress.show()

    def set_progress(self, pct: int):
        self._progress.setRange(0, 100)
        self._progress.setValue(pct)

    def set_done(self, success: bool):
        self._progress.hide()
        if success:
            # Dissolve the row after a short delay
            QTimer.singleShot(400, self._dissolve)
        else:
            self._status.setText("✗")
            self._status.setStyleSheet("color: #e53935; font-size: 16px;")
            self._status.show()

    def _dissolve(self):
        if not self._alive:
            return
        self._alive = False
        self.hide()
        p = self.parent()
        if p and hasattr(p, "_on_row_hidden"):
            p._on_row_hidden(self)
        self.deleteLater()


# ── Section card ──────────────────────────────────────────────────────────────

class UpdateSection(QFrame):
    update_all_clicked = pyqtSignal(list)

    def __init__(self, title: str, packages: list,
                 show_icons: bool = False, parent=None):
        super().__init__(parent)
        self._packages = packages
        self._rows: list[PackageUpdateRow] = []
        self._row_handler = None
        self.setObjectName("card")
        self.setFrameShape(QFrame.Shape.StyledPanel)

        vl = QVBoxLayout(self)
        vl.setContentsMargins(16, 12, 16, 12)
        vl.setSpacing(0)

        hdr = QHBoxLayout()
        hdr.addWidget(SectionTitle(title))
        hdr.addStretch()
        n = len(packages)
        hdr.addWidget(dimmed(QLabel(f"{n} update{'s' if n != 1 else ''}")))
        hdr.addSpacing(12)
        self._update_all_btn = QPushButton("Update All")
        self._update_all_btn.setFixedWidth(96)
        self._update_all_btn.clicked.connect(self._on_update_all_clicked)
        hdr.addWidget(self._update_all_btn)
        vl.addLayout(hdr)
        vl.addWidget(hline())
        vl.addSpacing(4)

        for i, pkg in enumerate(packages):
            row = PackageUpdateRow(pkg, show_icon=show_icons, parent=self)
            row.update_clicked.connect(
                lambda p, r=row: self._on_row_clicked(r, p))
            vl.addWidget(row)
            self._rows.append(row)
            if i < len(packages) - 1:
                sep = QFrame()
                sep.setFrameShape(QFrame.Shape.HLine)
                sep.setStyleSheet("color: rgba(128,128,128,0.15);")
                vl.addWidget(sep)

    def set_row_handler(self, handler):
        self._row_handler = handler

    def rows_with_pkgs(self) -> list[tuple]:
        """Return [(row, pkg), ...] for use in sequential update queue."""
        return list(zip(self._rows, self._packages))

    def _on_row_clicked(self, row: PackageUpdateRow, pkg: dict):
        row.set_updating()
        if self._row_handler:
            self._row_handler(row, pkg)

    def _on_update_all_clicked(self):
        self.update_all_clicked.emit(self._packages)

    def _on_row_hidden(self, row: PackageUpdateRow):
        """Called by a row after it dissolves. Hide section when all rows gone."""
        visible = [r for r in self._rows if r.isVisible()]
        if not visible:
            self.hide()
            self.deleteLater()

    def set_all_updating(self):
        self._update_all_btn.setEnabled(False)
        self._update_all_btn.setText("Updating…")


# ── OS Image card ─────────────────────────────────────────────────────────────

class ImageUpdateCard(QFrame):
    update_clicked = pyqtSignal()

    def __init__(self, info: dict, parent=None):
        super().__init__(parent)
        self._info = info
        self.setObjectName("card")
        self.setFrameShape(QFrame.Shape.StyledPanel)

        vl = QVBoxLayout(self)
        vl.setContentsMargins(16, 12, 16, 12)
        vl.setSpacing(0)

        hdr = QHBoxLayout()
        hdr.addWidget(SectionTitle("Operating System"))
        hdr.addStretch()
        hdr.addWidget(dimmed(QLabel("1 update")))
        vl.addLayout(hdr)
        vl.addWidget(hline())
        vl.addSpacing(4)

        booted      = info.get("booted", "current")
        available   = info.get("available", "new")
        update_type = info.get("type", "switch")

        row_hl = QHBoxLayout()
        row_hl.setContentsMargins(4, 8, 8, 8)
        row_hl.setSpacing(10)

        icon_lbl = QLabel("🖥")
        icon_lbl.setFixedSize(36, 36)
        icon_lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
        f = icon_lbl.font()
        f.setPointSize(20)
        icon_lbl.setFont(f)
        row_hl.addWidget(icon_lbl)

        info_col = QVBoxLayout()
        info_col.setSpacing(1)
        name_lbl = QLabel("RakuOS")
        nf = name_lbl.font()
        nf.setPointSize(nf.pointSize() + 1)
        name_lbl.setFont(nf)
        info_col.addWidget(name_lbl)
        if update_type == "switch":
            info_col.addWidget(dimmed(QLabel(f"{booted}  →  {available}")))
        else:
            info_col.addWidget(dimmed(QLabel(f"Refresh of {booted}")))
        row_hl.addLayout(info_col, stretch=1)

        self._progress = QProgressBar()
        self._progress.setRange(0, 0)
        self._progress.setFixedHeight(8)
        self._progress.setTextVisible(False)
        self._progress.setSizePolicy(
            QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        self._progress.setStyleSheet(
            "QProgressBar { border: none; background: rgba(128,128,128,0.2);"
            " border-radius: 4px; }"
            "QProgressBar::chunk { background: #4caf50; border-radius: 4px; }"
        )
        self._progress.hide()
        row_hl.addWidget(self._progress, stretch=1)

        self._status_lbl = QLabel("")
        self._status_lbl.hide()
        row_hl.addWidget(self._status_lbl)

        self._update_btn = QPushButton(
            "Update" if update_type == "switch" else "Apply Hotfix")
        self._update_btn.setFixedWidth(110)
        self._update_btn.clicked.connect(self.update_clicked)
        row_hl.addWidget(self._update_btn)
        vl.addLayout(row_hl)

    def set_updating(self):
        self._update_btn.hide()
        self._progress.setRange(0, 0)
        self._progress.show()

    def set_progress(self, pct: int):
        self._progress.setRange(0, 100)
        self._progress.setValue(pct)

    def set_done(self, success: bool):
        self._progress.hide()
        self._status_lbl.setText("✓" if success else "✗")
        self._status_lbl.setStyleSheet(
            "color: #4caf50; font-size: 16px;" if success
            else "color: #e53935; font-size: 16px;")
        self._status_lbl.show()


# ── Main page ─────────────────────────────────────────────────────────────────

class UpdatesPage(QWidget):
    check_done = pyqtSignal(dict)   # emitted after every fresh check (for tray update)

    def __init__(self):
        super().__init__()
        self._workers: list = []
        self._terminal: TerminalWidget | None = None
        self._image_card: ImageUpdateCard | None = None
        self._image_info: dict = {}
        self._all_sections: list[UpdateSection] = []
        self._last_result: dict | None = None   # None = never checked / needs refresh
        self._busy: bool = False   # True while any update sequence is running

        # ── Top bar ──────────────────────────────────────────────────────────
        topbar = QHBoxLayout()
        topbar.setContentsMargins(24, 16, 24, 8)

        title_lbl = QLabel("Updates")
        tf = title_lbl.font()
        tf.setPointSize(tf.pointSize() + 4)
        tf.setBold(True)
        title_lbl.setFont(tf)
        topbar.addWidget(title_lbl)
        topbar.addStretch()

        self._update_all_btn = QPushButton("⬆  Update All")
        self._update_all_btn.setFixedWidth(130)
        self._update_all_btn.hide()
        self._update_all_btn.clicked.connect(self._do_update_all)
        topbar.addWidget(self._update_all_btn)

        self._overall_bar = QProgressBar()
        self._overall_bar.setRange(0, 0)
        self._overall_bar.setFixedHeight(8)
        self._overall_bar.setFixedWidth(200)
        self._overall_bar.setTextVisible(False)
        self._overall_bar.setStyleSheet(
            "QProgressBar { border: none; background: rgba(128,128,128,0.2);"
            " border-radius: 4px; }"
            "QProgressBar::chunk { background: #4caf50; border-radius: 4px; }"
        )
        self._overall_bar.hide()
        topbar.addWidget(self._overall_bar)

        self._refresh_btn = QPushButton("↻  Check for Updates")
        self._refresh_btn.setFixedWidth(160)
        self._refresh_btn.clicked.connect(self._force_check)
        topbar.addWidget(self._refresh_btn)

        topbar_w = QWidget()
        topbar_w.setLayout(topbar)

        # ── Scroll ───────────────────────────────────────────────────────────
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        self._content = QWidget()
        self._vl = QVBoxLayout(self._content)
        self._vl.setContentsMargins(24, 8, 24, 24)
        self._vl.setSpacing(16)
        scroll.setWidget(self._content)

        outer = QVBoxLayout(self)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)
        outer.addWidget(topbar_w)
        outer.addWidget(scroll)

    # ── Load ─────────────────────────────────────────────────────────────────

    def _force_check(self):
        """Manual refresh — clear cache and fetch fresh."""
        self._last_result = None
        self.load(None)

    def load(self, result: dict = None):
        if self._busy:
            return
        # Use cached result if we already have one and no explicit result provided
        if result is None and self._last_result is not None:
            return
        self._clear()
        self._update_all_btn.hide()
        self._overall_bar.hide()
        if result is not None:
            self._render(result)
        else:
            self._vl.addWidget(LoadingWidget("Checking for updates…"))
            w = Worker(self._fetch_updates)
            w.result.connect(self._render)
            w.start()
            self._workers.append(w)

    def _fetch_updates(self) -> dict:
        import json
        RAKUOS_UPDATE = "/usr/libexec/rakuos/rakuos-update"

        def _check(cmd):
            try:
                r = subprocess.run([RAKUOS_UPDATE, cmd],
                    capture_output=True, text=True, timeout=120)
                data = json.loads(r.stdout)
                return r.returncode == 0, data.get("updates", [])
            except Exception:
                return False, []

        def _check_image():
            try:
                r = subprocess.run([RAKUOS_UPDATE, "check-image"],
                    capture_output=True, text=True, timeout=30)
                data = json.loads(r.stdout)
                return r.returncode == 0, data
            except Exception:
                return False, {}

        _, pkgs             = _check("check")
        _, fps              = _check("check-flatpak")
        img_avail, img_info = _check_image()

        return {
            "packages":        pkgs,
            "flatpak":         fps,
            "image_available": img_avail,
            "image_info":      img_info,
            "total": len(pkgs) + len(fps) + (1 if img_avail else 0),
        }

    # ── Render ────────────────────────────────────────────────────────────────

    def _render(self, result: dict):
        self._clear()
        self._all_sections = []

        self._last_result = result
        self.check_done.emit(result)   # notify window to update tray/badge
        pkgs      = result.get("packages", [])
        fps       = result.get("flatpak", [])
        ais       = result.get("appimages", [])
        img_avail = result.get("image_available", False)
        img_info  = result.get("image_info", {})
        total     = result.get("total", 0)
        self._image_info = img_info

        if total == 0:
            self._vl.addStretch()
            up_lbl = QLabel("✓  Fully up to date")
            f = up_lbl.font()
            f.setPointSize(f.pointSize() + 6)
            up_lbl.setFont(f)
            up_lbl.setStyleSheet("color: #4caf50;")
            up_lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
            self._vl.addWidget(up_lbl, alignment=Qt.AlignmentFlag.AlignCenter)
            sub = dimmed(QLabel("Your system and all apps are up to date."))
            sub.setAlignment(Qt.AlignmentFlag.AlignCenter)
            self._vl.addWidget(sub, alignment=Qt.AlignmentFlag.AlignCenter)
            self._vl.addStretch()
            return

        self._update_all_btn.show()

        gui_pkgs = [p for p in pkgs if p.get("gui")]
        fp_apps  = [dict(p, is_flatpak=True) for p in fps if not p.get("runtime")]
        ai_apps  = [dict(a, is_appimage=True) for a in ais]
        app_group = gui_pkgs + fp_apps + ai_apps
        if app_group:
            sec = UpdateSection("Applications", app_group, show_icons=True)
            sec.update_all_clicked.connect(
                lambda _: self._run_section_update(sec))
            sec.set_row_handler(self._on_single_row_update)
            self._vl.addWidget(sec)
            self._all_sections.append(sec)

        fp_runtimes = [dict(p, is_flatpak=True) for p in fps if p.get("runtime")]
        if fp_runtimes:
            sec = UpdateSection("Add-ons", fp_runtimes)
            sec.update_all_clicked.connect(
                lambda _: self._run_section_update(sec))
            sec.set_row_handler(self._on_single_row_update)
            self._vl.addWidget(sec)
            self._all_sections.append(sec)

        sys_pkgs = [p for p in pkgs if not p.get("gui")]
        if sys_pkgs:
            sec = UpdateSection("System Dependencies", sys_pkgs)
            sec.update_all_clicked.connect(
                lambda _: self._run_section_update(sec))
            sec.set_row_handler(self._on_single_row_update)
            self._vl.addWidget(sec)
            self._all_sections.append(sec)

        if img_avail:
            self._image_card = ImageUpdateCard(img_info)
            self._image_card.update_clicked.connect(self._do_image_update)
            self._vl.addWidget(self._image_card)

        self._vl.addStretch()

    # ── Sequential update engine ──────────────────────────────────────────────

    def _run_step_sequence(self, steps: list, on_complete):
        """Run a flat list of callables sequentially.
        Each callable receives a done(success: bool) callback.
        """
        if not steps:
            try:
                on_complete()
            except Exception:
                pass
            return
        step, remaining = steps[0], steps[1:]

        def after(success):
            QTimer.singleShot(300, lambda: self._run_step_sequence(remaining, on_complete))

        try:
            step(after)
        except Exception:
            after(False)

    def _do_update_all(self):
        if self._busy:
            return
        self._busy = True
        self._update_all_btn.hide()
        self._overall_bar.setRange(0, 0)
        self._overall_bar.show()

        for sec in self._all_sections:
            sec.set_all_updating()

        # Collect all tasks grouped by type — RPMs batch, then fps/ais one-by-one
        rpm_rows, fp_tasks, ai_tasks = [], [], []
        for sec in self._all_sections:
            for row, pkg in sec.rows_with_pkgs():
                if pkg.get("is_appimage"):
                    ai_tasks.append((row, pkg))
                elif pkg.get("is_flatpak"):
                    fp_tasks.append((row, pkg))
                else:
                    rpm_rows.append((row, pkg))

        steps = []
        if rpm_rows:
            steps.append(lambda cb, _r=rpm_rows: self._run_rpm_batch(_r, cb))
        for row, pkg in fp_tasks:
            steps.append(lambda cb, r=row, p=pkg: self._run_flatpak_update(r, p, cb))
        for row, pkg in ai_tasks:
            steps.append(lambda cb, r=row, p=pkg: self._run_appimage_update(r, p, cb))

        def on_apps_done():
            self._overall_bar.hide()
            if self._image_card:
                # Image update always runs last; _busy cleared in _on_image_done
                self._do_image_update()
            else:
                self._busy = False
                self._last_result = None   # force fresh check after updates done
                QTimer.singleShot(600, lambda: self.load(None))

        self._run_step_sequence(steps, on_apps_done)

    def _run_section_update(self, sec: "UpdateSection"):
        """Update All within a single section — sequential, same engine."""
        if self._busy:
            return
        self._busy = True
        sec.set_all_updating()
        rpm_rows, fp_tasks, ai_tasks = [], [], []
        for row, pkg in sec.rows_with_pkgs():
            if pkg.get("is_appimage"):
                ai_tasks.append((row, pkg))
            elif pkg.get("is_flatpak"):
                fp_tasks.append((row, pkg))
            else:
                rpm_rows.append((row, pkg))

        steps = []
        if rpm_rows:
            steps.append(lambda cb, _r=rpm_rows: self._run_rpm_batch(_r, cb))
        for row, pkg in fp_tasks:
            steps.append(lambda cb, r=row, p=pkg: self._run_flatpak_update(r, p, cb))
        for row, pkg in ai_tasks:
            steps.append(lambda cb, r=row, p=pkg: self._run_appimage_update(r, p, cb))

        def _done():
            self._busy = False
            self._last_result = None
            QTimer.singleShot(600, lambda: self.load(None))

        self._run_step_sequence(steps, _done)

    def _on_single_row_update(self, row: PackageUpdateRow, pkg: dict):
        """Single-row Update button — reuse the same helpers."""
        if self._busy:
            return
        self._busy = True

        def _done(_=None):
            self._busy = False
            self._last_result = None
            QTimer.singleShot(600, lambda: self.load(None))

        if pkg.get("is_appimage"):
            self._run_appimage_update(row, pkg, _done)
        elif pkg.get("is_flatpak"):
            self._run_flatpak_update(row, pkg, _done)
        else:
            self._run_rpm_batch([(row, pkg)], _done)

    # ── Cache helpers ─────────────────────────────────────────────────────────

    def _remove_from_cache(self, pkgs: list[dict]):
        """Remove completed packages from cache and emit check_done to update tray."""
        if not self._last_result:
            return
        ids = {p.get("app_id") or p.get("id", "") for p in pkgs}
        for key in ("packages", "flatpak", "appimages"):
            self._last_result[key] = [
                p for p in self._last_result.get(key, [])
                if (p.get("app_id") or p.get("id", "")) not in ids
            ]
        self._last_result["total"] = (
            len(self._last_result.get("packages", []))
            + len(self._last_result.get("flatpak", []))
            + (1 if self._last_result.get("image_available") else 0)
        )
        self.check_done.emit(self._last_result)

    # ── Per-type update helpers ───────────────────────────────────────────────

    def _run_rpm_batch(self, rpm_rows: list, on_done):
        """Run a single dnf upgrade for all RPM rows together."""
        for row, _ in rpm_rows:
            row.set_updating()
        buf = []

        def _line(line):
            pct = _parse_dnf_progress(line)
            if pct is not None:
                for row, _ in rpm_rows:
                    row.set_progress(pct)
            buf.append(line)

        def _done(code):
            success = code == 0
            for row, _ in rpm_rows:
                row.set_done(success)
            if success:
                self._remove_from_cache([pkg for _, pkg in rpm_rows])
            else:
                self._show_error_in_terminal("RPM packages", code, buf)
            on_done(success)

        w = StreamWorker(upd.upgrade_packages_stream)
        w.line.connect(_line)
        w.done.connect(_done)
        w.start()
        self._workers.append(w)

    def _run_flatpak_update(self, row: PackageUpdateRow, pkg: dict, on_done):
        app_id = pkg.get("app_id") or pkg.get("id", "")
        name   = pkg.get("name", app_id)
        row.set_updating()
        buf = []

        def _line(line):
            pct = _parse_flatpak_progress(line)
            if pct is not None:
                row.set_progress(pct)
            buf.append(line)

        def _done(code):
            success = code == 0
            row.set_done(success)
            if success:
                self._remove_from_cache([pkg])
            else:
                self._show_error_in_terminal(name, code, buf)
            on_done(success)

        w = StreamWorker(lambda _id=app_id: flatpak.update_flatpak_stream(_id))
        w.line.connect(_line)
        w.done.connect(_done)
        w.start()
        self._workers.append(w)

    def _run_appimage_update(self, row: PackageUpdateRow, pkg: dict, on_done):
        from backend import appimages as _aim
        app_id = pkg.get("id", "")
        dl_url = pkg.get("download_url", "")
        name   = pkg.get("name", app_id)

        if not app_id or not dl_url:
            row.set_done(False)
            on_done(False)
            return

        row.set_updating()
        buf = []

        def _line(line):
            if line.startswith("DOWNLOAD:"):
                try:
                    row.set_progress(int(line.split(":")[1]))
                except ValueError:
                    pass
            else:
                buf.append(line)

        def _done(code):
            success = code == 0
            row.set_done(success)
            if success:
                self._remove_from_cache([pkg])
            else:
                self._show_error_in_terminal(name, code, buf)
            on_done(success)

        w = StreamWorker(
            lambda _id=app_id, _url=dl_url: _aim.update_appimage_stream(_id, _url))
        w.line.connect(_line)
        w.done.connect(_done)
        w.start()
        self._workers.append(w)

    # ── Image update ──────────────────────────────────────────────────────────

    def _do_image_update(self):
        if self._image_card:
            self._image_card.set_updating()
        info = self._image_info or {}
        buf  = []

        def _img_line(line):
            pct = _parse_bootc_progress(line)
            if pct is not None and self._image_card:
                self._image_card.set_progress(pct)
            buf.append(line)

        def _done(code):
            self._busy = False
            self._last_result = None   # force fresh check after image update
            if code == 0:
                self._show_reboot_required()
            else:
                if self._image_card:
                    self._image_card.set_done(False)
                self._show_error_in_terminal("OS image update", code, buf)

        w = StreamWorker(
            upd.upgrade_image_stream,
            info.get("type", "switch"),
            info.get("repo", ""),
            info.get("available", ""),
        )
        w.line.connect(_img_line)
        w.done.connect(_done)
        w.start()
        self._workers.append(w)

    # ── Reboot required screen ────────────────────────────────────────────────

    def _show_reboot_required(self):
        self._clear()
        self._update_all_btn.hide()
        self._overall_bar.hide()

        self._vl.addStretch()

        icon_lbl = QLabel("⟳")
        f = icon_lbl.font()
        f.setPointSize(f.pointSize() + 24)
        icon_lbl.setFont(f)
        icon_lbl.setStyleSheet("color: #4caf50;")
        icon_lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._vl.addWidget(icon_lbl, alignment=Qt.AlignmentFlag.AlignCenter)

        title_lbl = QLabel("Update staged — reboot to apply")
        tf = title_lbl.font()
        tf.setPointSize(tf.pointSize() + 4)
        tf.setBold(True)
        title_lbl.setFont(tf)
        title_lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._vl.addWidget(title_lbl, alignment=Qt.AlignmentFlag.AlignCenter)

        sub_lbl = dimmed(QLabel(
            "The new system image has been downloaded and staged.\n"
            "Reboot to boot into the updated system."))
        sub_lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._vl.addSpacing(8)
        self._vl.addWidget(sub_lbl, alignment=Qt.AlignmentFlag.AlignCenter)

        self._vl.addSpacing(24)

        reboot_btn = QPushButton("🔄  Reboot Now")
        reboot_btn.setFixedWidth(180)
        reboot_btn.setFixedHeight(40)
        reboot_btn.setStyleSheet(
            "QPushButton { background: #4caf50; color: white; border-radius: 6px;"
            " font-size: 14px; font-weight: bold; }"
            "QPushButton:hover { background: #43a047; }"
        )
        reboot_btn.clicked.connect(upd.schedule_reboot)
        self._vl.addWidget(reboot_btn, alignment=Qt.AlignmentFlag.AlignCenter)

        self._vl.addStretch()

    # ── Error terminal (lazy) ─────────────────────────────────────────────────

    def _get_or_create_terminal(self) -> TerminalWidget:
        if self._terminal is None:
            self._terminal = TerminalWidget()
            # Insert before the final stretch
            idx = max(self._vl.count() - 1, 0)
            self._vl.insertWidget(idx, self._terminal)
        self._terminal.show()
        return self._terminal

    def _show_error_in_terminal(self, name: str, code: int, buf: list):
        term = self._get_or_create_terminal()
        for line in buf:
            term.append_line(line)
        term.append_line(f"\n✗ {name} failed (exit {code}).")

    # ── Helpers ───────────────────────────────────────────────────────────────

    def _clear(self):
        self._terminal = None
        self._image_card = None
        self._image_info = {}
        self._all_sections = []
        while self._vl.count():
            item = self._vl.takeAt(0)
            if item.widget():
                item.widget().deleteLater()
