"""
webapps.py — Web App management for RakuOS Software Center.

System catalog:   /usr/share/rakuos/webapps/*.json  (read-only, ships with image)
User installed:   ~/.local/share/rakuos/webapps/*.json
Desktop files:    ~/.local/share/applications/rakuos-webapp-{id}.desktop
Icons:            ~/.local/share/rakuos/webapps/icons/{id}.png (cached from catalog)

Each catalog JSON:
{
  "id":          "netflix",
  "name":        "Netflix",
  "url":         "https://netflix.com",
  "description": "Watch TV shows and movies",
  "summary":     "Streaming entertainment",
  "icon":        "netflix.png",          # relative to catalog dir
  "icon_url":    "https://...",          # fallback remote icon
  "categories":  ["AudioVideo", "Video"],
  "keywords":    ["streaming", "movies", "tv"]
}

Bundle/suite JSON (optional "apps" array):
{
  "id":          "google-workspace",
  "name":        "Google Workspace",
  "url":         "https://workspace.google.com",   # hub/portal entry
  "icon_url":    "https://...",
  "categories":  ["Office"],
  "keywords":    ["google", "office", "docs"],
  "apps": [
    { "id": "google-docs",   "name": "Google Docs",   "url": "https://docs.google.com",   "icon_url": "https://..." },
    { "id": "google-sheets", "name": "Google Sheets", "url": "https://sheets.google.com", "icon_url": "https://..." }
  ]
}
Sub-apps inherit categories/keywords/website from parent; override any field locally.
The parent hub entry is also included if it has its own url/website.
"""

import os
import json
import shutil
import subprocess
import urllib.request
from pathlib import Path


# ── Paths ─────────────────────────────────────────────────────────────────────

CATALOG_DIR   = Path("/usr/share/rakuos/webapps")
INSTALL_DIR   = Path.home() / ".local/share/rakuos/webapps"
ICON_DIR      = Path.home() / ".local/share/rakuos/webapps/icons"
DESKTOP_DIR   = Path.home() / ".local/share/applications"
DESKTOP_PREFIX = "rakuos-webapp-"


def _ensure_dirs():
    INSTALL_DIR.mkdir(parents=True, exist_ok=True)
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    DESKTOP_DIR.mkdir(parents=True, exist_ok=True)


# ── Catalog ───────────────────────────────────────────────────────────────────

def _finalise(entry: dict) -> dict:
    """Attach runtime fields (source, installed, icon_path, screenshots) to a catalog entry."""
    entry["source"]    = "webapp"
    entry["installed"] = is_installed(entry["id"])
    entry["icon_path"] = _resolve_icon(entry)
    entry.setdefault("screenshots", [])
    return entry


def _expand_catalog_data(data: dict) -> list[dict]:
    """
    Expand a raw catalog dict into one or more app entries.

    - If the dict has no 'apps' key it is returned as-is (single entry).
    - If it has an 'apps' array each sub-app is merged with inherited parent
      fields (categories, keywords, website/url), then _finalised.
    - The parent hub entry is also included when it has its own url/website.
    """
    sub_apps = data.get("apps")
    if not sub_apps:
        return [_finalise(dict(data))]

    entries = []

    # Parent hub (portal / landing page)
    if data.get("url") or data.get("website"):
        hub = {k: v for k, v in data.items() if k != "apps"}
        entries.append(_finalise(hub))

    # Inherited fields sub-apps receive from parent unless they override
    inherited = {
        "categories": data.get("categories", []),
        "keywords":   data.get("keywords", []),
        "website":    data.get("website") or data.get("url", ""),
        "screenshots": data.get("screenshots", []),
        "custom_css": data.get("custom_css", ""),
    }

    for sub in sub_apps:
        entry = dict(inherited)
        entry.update(sub)          # sub-app fields take priority
        entry.setdefault("session_group", data["id"])   # share session with suite
        entries.append(_finalise(entry))

    return entries


def get_catalog() -> list[dict]:
    """
    Return all web apps from the system catalog.
    Icons are resolved and downloaded here — call from a worker thread.
    """
    apps = []
    if not CATALOG_DIR.exists():
        return apps
    for path in sorted(CATALOG_DIR.glob("*.json")):
        try:
            data = json.loads(path.read_text())
            apps.extend(_expand_catalog_data(data))
        except Exception as e:
            print(f"[webapps] Failed to read {path}: {e}")
    return apps


def resolve_icon_for(app_id: str) -> str:
    """Resolve icon for a single app by id. Returns cached path or ''."""
    app = get_catalog_by_id(app_id)
    if not app:
        return ""
    return _resolve_icon(app)


def get_catalog_by_id(app_id: str) -> dict | None:
    """Return a single catalog entry by id, searching bundle sub-apps too."""
    if not CATALOG_DIR.exists():
        return None

    for path in sorted(CATALOG_DIR.glob("*.json")):
        try:
            data = json.loads(path.read_text())
        except Exception:
            continue

        # Simple (non-bundle) JSON matching by id
        if data.get("id") == app_id and not data.get("apps"):
            return _finalise(dict(data))

        # Bundle: check hub and sub-apps
        if data.get("apps"):
            # Hub entry
            if data.get("id") == app_id:
                hub = {k: v for k, v in data.items() if k != "apps"}
                return _finalise(hub)
            # Sub-apps
            inherited = {
                "categories": data.get("categories", []),
                "keywords":   data.get("keywords", []),
                "website":    data.get("website") or data.get("url", ""),
                "screenshots": data.get("screenshots", []),
                "custom_css": data.get("custom_css", ""),
            }
            for sub in data["apps"]:
                if sub.get("id") == app_id:
                    entry = dict(inherited)
                    entry.update(sub)
                    entry.setdefault("session_group", data["id"])
                    return _finalise(entry)

    return None


def _resolve_icon(app: dict) -> str:
    """
    Return local cached icon path for app, downloading from icon_url if needed.

    Resolution order:
      1. Already cached at ICON_DIR/{id}.{ext}  → return immediately
      2. Download from icon_url → save as ICON_DIR/{id}.{ext} → return path
      3. Fall back to empty string (UI shows placeholder)

    The `icon` field in the catalog JSON is just the filename used for the
    .desktop Icon= entry and is derived from the cached path — it is NOT
    used as a local lookup. All icons live in the user cache dir.
    """
    app_id   = app.get("id", "")
    icon_url = app.get("icon_url", "")

    # Derive extension from icon_url, default png
    ext = "png"
    if icon_url:
        url_path = icon_url.split("?")[0]  # strip query string
        if "." in url_path.split("/")[-1]:
            ext = url_path.rsplit(".", 1)[-1].lower()
            # Normalise ico → png since QPixmap handles png best
            if ext not in ("png", "svg", "jpg", "jpeg", "webp"):
                ext = "png"

    cached = ICON_DIR / f"{app_id}.{ext}"

    # 1. Already cached
    if cached.exists():
        return str(cached)

    # 2. Download from icon_url
    if icon_url:
        try:
            _ensure_dirs()
            import tempfile, shutil
            tmp = Path(tempfile.mktemp(suffix=f".{ext}"))
            req = urllib.request.Request(
                icon_url,
                headers={"User-Agent": "Mozilla/5.0 RakuOS-Software-Center/1.0"}
            )
            with urllib.request.urlopen(req, timeout=10) as resp:
                tmp.write_bytes(resp.read())

            # If .ico, convert to PNG using Qt so QPixmap loads cleanly
            if ext == "ico" or icon_url.lower().endswith(".ico"):
                try:
                    # Lazy import — only needed for ico conversion
                    import subprocess
                    result = subprocess.run(
                        ["python3", "-c",
                         f"from PyQt6.QtGui import QImage; "
                         f"img = QImage('{tmp}'); "
                         f"img.save('{cached}', 'PNG')"],
                        capture_output=True, timeout=5
                    )
                    tmp.unlink(missing_ok=True)
                    if cached.exists():
                        return str(cached)
                except Exception:
                    pass  # fall through to raw copy

            shutil.move(str(tmp), cached)
            return str(cached)
        except Exception as e:
            print(f"[webapps] Failed to download icon for {app_id}: {e}")

    return ""


# ── Install / Uninstall ───────────────────────────────────────────────────────

def search_webapps(query: str) -> list[dict]:
    """Return catalog web apps whose name, summary, description, or keywords match query."""
    q = query.lower()
    results = []
    for app in get_catalog():
        if (q in app.get("name", "").lower() or
                q in app.get("summary", "").lower() or
                q in app.get("description", "").lower() or
                any(q in kw.lower() for kw in app.get("keywords", []))):
            results.append(app)
    return results


def is_installed(app_id: str) -> bool:
    return (INSTALL_DIR / f"{app_id}.json").exists()


def get_installed() -> list[dict]:
    """Return all installed web apps with full metadata."""
    apps = []
    if not INSTALL_DIR.exists():
        return apps
    for path in sorted(INSTALL_DIR.glob("*.json")):
        try:
            data = json.loads(path.read_text())
            data["installed"] = True
            data["source"]    = "webapp"   # always set so InstalledRow renders correctly
            apps.append(data)
        except Exception as e:
            print(f"[webapps] Failed to read installed {path}: {e}")
    return apps


def install(app_id: str) -> tuple[bool, str]:
    """
    Install a web app from the catalog.
    Returns (success, message).
    """
    app = get_catalog_by_id(app_id)
    if not app:
        return False, f"Web app '{app_id}' not found in catalog."

    try:
        _ensure_dirs()

        # Resolve and cache icon — downloads from icon_url, retrying once on failure
        icon_path = _resolve_icon(app)
        if not icon_path:
            # Retry once in case of transient network error
            icon_path = _resolve_icon(app)
        if not icon_path:
            icon_path = "web-browser"

        # Resolve launch URL — url field takes priority, website is the fallback
        launch_url = app.get("url") or app.get("website", "")

        # Write installed JSON sidecar
        installed_data = {
            "id":          app["id"],
            "name":        app["name"],
            "url":         launch_url,
            "website":     app.get("website", ""),
            "description": app.get("description", ""),
            "summary":     app.get("summary", ""),
            "icon_path":   icon_path,
            "categories":  app.get("categories", []),
            "keywords":    app.get("keywords", []),
            "screenshots":   app.get("screenshots", []),
            "custom_css":    app.get("custom_css", ""),
            "session_group": app.get("session_group", ""),
            "source":        "webapp",
            "installed":     True,
        }
        (INSTALL_DIR / f"{app_id}.json").write_text(
            json.dumps(installed_data, indent=2))

        # Write custom CSS using the name-derived id so it matches the
        # launcher's APP_ID computation (lowercase, non-alnum → hyphen)
        import re
        css_id = re.sub(r'[^a-z0-9]+', '-', app['name'].lower()).strip('-')
        css_file = INSTALL_DIR / f"{css_id}.css"
        custom_css = app.get("custom_css", "")
        if custom_css:
            css_file.write_text(custom_css)
        elif css_file.exists():
            css_file.unlink()

        # Write .desktop file
        _write_desktop(app, icon_path)

        # Update desktop database
        subprocess.run(
            ["update-desktop-database", str(DESKTOP_DIR)],
            capture_output=True)

        return True, f"{app['name']} installed successfully."
    except Exception as e:
        return False, f"Failed to install {app_id}: {e}"


def uninstall(app_id: str) -> tuple[bool, str]:
    """
    Uninstall a web app.
    Returns (success, message).
    """
    try:
        name = app_id
        sidecar = INSTALL_DIR / f"{app_id}.json"
        if sidecar.exists():
            data = json.loads(sidecar.read_text())
            name = data.get("name", app_id)
            sidecar.unlink()

        desktop = DESKTOP_DIR / f"{DESKTOP_PREFIX}{app_id}.desktop"
        if desktop.exists():
            desktop.unlink()

        import re
        css_id = re.sub(r'[^a-z0-9]+', '-', name.lower()).strip('-')
        css_file = INSTALL_DIR / f"{css_id}.css"
        if css_file.exists():
            css_file.unlink()

        subprocess.run(
            ["update-desktop-database", str(DESKTOP_DIR)],
            capture_output=True)

        return True, f"{name} uninstalled."
    except Exception as e:
        return False, f"Failed to uninstall {app_id}: {e}"


def _write_desktop(app: dict, icon_path: str):
    """Write a .desktop launcher for the web app using cefpython3."""
    app_id = app["id"]
    name   = app["name"]
    url    = app.get("url") or app.get("website", "")
    desc   = app.get("summary") or app.get("description", "")
    cats   = ";".join(app.get("categories", ["Network"])) + ";"

    # Icon= uses the cached path so the file manager and app launcher both
    # pick up the downloaded icon without needing a named theme icon
    icon = icon_path or "web-browser"

    session_group = app.get("session_group", "")
    exec_cmd = f"/usr/bin/rakuos-webapp-launcher '{url}' '{name}'"
    if session_group:
        exec_cmd += f" '{session_group}'"

    desktop_content = f"""[Desktop Entry]
Name={name}
Comment={desc}
Exec={exec_cmd}
Icon={icon}
Terminal=false
Type=Application
Categories={cats}
StartupNotify=true
StartupWMClass=rakuos-webapp-{app_id}
X-RakuOS-WebApp=true
X-RakuOS-WebApp-ID={app_id}
X-RakuOS-WebApp-URL={url}
"""
    desktop_path = DESKTOP_DIR / f"{DESKTOP_PREFIX}{app_id}.desktop"
    desktop_path.write_text(desktop_content)
    desktop_path.chmod(0o755)
