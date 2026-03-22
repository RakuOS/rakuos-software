"""
packages.py — Native package management via rakuos install/remove and AppStream metadata
"""

import json
import logging
import os
import re
import gzip
import locale
import subprocess
import threading
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Optional

# ── Timezone → default locale mapping (last-resort fallback) ──────────────────
_TZ_LANG_MAP: dict[str, str] = {
    "America/New_York":    "en_US", "America/Chicago":      "en_US",
    "America/Denver":      "en_US", "America/Los_Angeles":  "en_US",
    "America/Toronto":     "en_CA", "America/Vancouver":    "en_CA",
    "America/Montreal":    "fr_CA", "America/Sao_Paulo":    "pt_BR",
    "America/Buenos_Aires":"es_AR", "America/Mexico_City":  "es_MX",
    "America/Bogota":      "es_CO", "America/Lima":         "es_PE",
    "America/Santiago":    "es_CL", "America/Caracas":      "es_VE",
    "America/Havana":      "es_CU",
    "Europe/London":       "en_GB", "Europe/Dublin":        "en_IE",
    "Europe/Paris":        "fr_FR", "Europe/Brussels":      "fr_BE",
    "Europe/Berlin":       "de_DE", "Europe/Vienna":        "de_AT",
    "Europe/Zurich":       "de_CH", "Europe/Madrid":        "es_ES",
    "Europe/Rome":         "it_IT", "Europe/Amsterdam":     "nl_NL",
    "Europe/Warsaw":       "pl_PL", "Europe/Prague":        "cs_CZ",
    "Europe/Budapest":     "hu_HU", "Europe/Bucharest":     "ro_RO",
    "Europe/Stockholm":    "sv_SE", "Europe/Copenhagen":    "da_DK",
    "Europe/Helsinki":     "fi_FI", "Europe/Oslo":          "nb_NO",
    "Europe/Lisbon":       "pt_PT", "Europe/Athens":        "el_GR",
    "Europe/Kiev":         "uk_UA", "Europe/Kyiv":          "uk_UA",
    "Europe/Moscow":       "ru_RU", "Europe/Istanbul":      "tr_TR",
    "Asia/Tokyo":          "ja_JP", "Asia/Shanghai":        "zh_CN",
    "Asia/Hong_Kong":      "zh_HK", "Asia/Taipei":          "zh_TW",
    "Asia/Seoul":          "ko_KR", "Asia/Kolkata":         "hi_IN",
    "Asia/Bangkok":        "th_TH", "Asia/Jakarta":         "id_ID",
    "Asia/Kuala_Lumpur":   "ms_MY", "Asia/Singapore":       "en_SG",
    "Asia/Riyadh":         "ar_SA", "Asia/Tehran":          "fa_IR",
    "Asia/Jerusalem":      "he_IL", "Asia/Karachi":         "ur_PK",
    "Australia/Sydney":    "en_AU", "Australia/Melbourne":  "en_AU",
    "Pacific/Auckland":    "en_NZ", "Africa/Cairo":         "ar_EG",
    "Africa/Lagos":        "en_NG", "Africa/Johannesburg":  "en_ZA",
    "Africa/Nairobi":      "sw_KE", "Africa/Casablanca":    "ar_MA",
}

_TZ_AREA_FALLBACK: list[tuple[str, str]] = [
    ("America/",  "en_US"), ("Europe/",   "en_GB"),
    ("Asia/",     "en_US"), ("Australia/","en_AU"),
    ("Pacific/",  "en_US"), ("Africa/",   "en_ZA"),
]


def _detect_timezone() -> str:
    """Return the system timezone string e.g. 'America/New_York'."""
    import os as _os
    tz = _os.environ.get("TZ", "")
    if tz:
        return tz
    try:
        return open("/etc/timezone").read().strip()
    except Exception:
        pass
    try:
        link = _os.readlink("/etc/localtime")
        idx = link.find("zoneinfo/")
        if idx >= 0:
            return link[idx + 9:]
    except Exception:
        pass
    return ""


def _lang_from_timezone() -> str:
    """Return full locale code (e.g. 'en_US') inferred from system timezone."""
    tz = _detect_timezone()
    if tz in _TZ_LANG_MAP:
        return _TZ_LANG_MAP[tz]
    for prefix, fallback in _TZ_AREA_FALLBACK:
        if tz.startswith(prefix):
            return fallback
    return ""


def _get_system_lang() -> str:
    """
    Detect the user's locale, write LANG to os.environ, return full locale code.

    Returns the full locale string e.g. 'en_US', 'de_DE', 'pt_BR'.
    Callers extract the 2-letter code with .split('_')[0].

    Detection order:
      1. Env vars: LANG / LANGUAGE / LC_ALL / LC_MESSAGES / LC_TIME / LC_CTYPE
         (all checked — when LANG=C.UTF-8 the LC_* vars often carry the real locale)
      2. /etc/locale.conf and ~/.config/locale.conf — ALL LC_* keys scanned,
         not just LANG=, so LC_TIME=en_US.UTF-8 is found even when LANG=C.UTF-8
      3. localectl status — checks LC_MESSAGES then LANG
      4. Python locale module
      5. Timezone inference (last resort)
    """
    import os as _os
    import subprocess as _sp

    def _normalise_and_set(full_val: str) -> str:
        """Strip encoding, write LANG, return full locale e.g. 'en_US'."""
        base = full_val.split(".")[0]   # "en_US.UTF-8" → "en_US" / "de" → "de"
        _os.environ["LANG"] = base + ".UTF-8"
        return base

    def _bad(val: str) -> bool:
        return val.upper() in ("", "C", "POSIX", "C.UTF-8", "C.UTF8")

    # Priority order for LC_ vars: messages first (most relevant for UI text),
    # then time (often set by desktop even when LANG=C), then others.
    _LC_PRIORITY = ("LANG", "LANGUAGE", "LC_ALL", "LC_MESSAGES",
                    "LC_TIME", "LC_CTYPE", "LC_NUMERIC")

    # 1. Env vars ─────────────────────────────────────────────────────────────
    for var in _LC_PRIORITY:
        val = _os.environ.get(var, "")
        if val and not _bad(val):
            base = val.split(".")[0]
            code = base.split("_")[0].lower()
            if code and code != "c":
                _os.environ["LANG"] = base + ".UTF-8"
                return base

    # 2. locale.conf files — scan ALL LC_* keys, not just LANG= ───────────────
    for conf in ("/etc/locale.conf",
                 _os.path.expanduser("~/.config/locale.conf")):
        try:
            lc_found: dict[str, str] = {}
            for line in open(conf):
                line = line.strip()
                for key in _LC_PRIORITY:
                    if line.startswith(key + "="):
                        val = line[len(key) + 1:].strip("\"'")
                        if val and not _bad(val):
                            lc_found[key] = val
            # Use the highest-priority key found in this file
            for key in _LC_PRIORITY:
                if key in lc_found:
                    return _normalise_and_set(lc_found[key])
        except Exception:
            pass

    # 3. localectl — check LC_MESSAGES then LANG ──────────────────────────────
    try:
        out = _sp.run(["localectl", "status"],
                      capture_output=True, text=True, timeout=3)
        lc_found = {}
        for line in out.stdout.splitlines():
            for key in _LC_PRIORITY:
                m = re.search(rf'{key}=([^\s"\']+)', line)
                if m:
                    val = m.group(1)
                    if not _bad(val):
                        lc_found[key] = val
        for key in _LC_PRIORITY:
            if key in lc_found:
                return _normalise_and_set(lc_found[key])
    except Exception:
        pass

    # 4. Python locale module ─────────────────────────────────────────────────
    try:
        loc = locale.getlocale()[0] or ""
        if loc and loc.upper() not in ("C", "POSIX"):
            return _normalise_and_set(loc)
    except Exception:
        pass

    # 5. Timezone inference ───────────────────────────────────────────────────
    tz_locale = _lang_from_timezone()
    if tz_locale:
        return _normalise_and_set(tz_locale)

    # Final fallback
    _os.environ.setdefault("LANG", "en_US.UTF-8")
    return "en_US"


# ── Module-level locale constants (evaluated once at import) ──────────────────
_SYSTEM_LOCALE = _get_system_lang()              # e.g. "en_US", "de_DE", "pt_BR"
SYSTEM_LANG    = _SYSTEM_LOCALE.split("_")[0].lower()  # e.g. "en", "de", "pt"

def _get_fedora_version() -> str:
    try:
        with open("/etc/os-release") as f:
            for line in f:
                if line.startswith("VERSION_ID="):
                    return line.split("=", 1)[1].strip().strip('"')
    except Exception:
        pass
    return "43"

_FEDORA_VER = _get_fedora_version()
# Normalised forms used for by_lang_full dict lookups (xml:lang is lowercased)
_SYSLOCALE_US  = _SYSTEM_LOCALE.lower()                     # "en_us", "pt_br"
_SYSLOCALE_HYP = _SYSTEM_LOCALE.lower().replace("_", "-")   # "en-us", "pt-br"

print(f"[packages] SYSTEM_LANG={SYSTEM_LANG!r}  SYSTEM_LOCALE={_SYSTEM_LOCALE!r}"
      f"  LANG={os.environ.get('LANG', 'unset')}")
XML_LANG = "{http://www.w3.org/XML/1998/namespace}lang"


# Languages that use Latin script — prefer these as last resort over others
_LATIN_LANGS = {
    "en","de","fr","es","it","pt","nl","sv","da","no","fi","pl","cs","sk",
    "hu","ro","hr","sl","lt","lv","et","tr","id","ms","vi","af","eu","ca",
}

def _pick_lang(by_lang: dict, default: str) -> str:
    """Pick best text from a lang→text dict, preferring system lang then English."""
    # 1. Exact system language match (2-letter code, e.g. "de", "fr")
    if by_lang.get(SYSTEM_LANG):
        return by_lang[SYSTEM_LANG]
    # 2. Untagged default — English content in AppStream is NEVER tagged with
    #    xml:lang="en"; it is always the untagged default element.
    if default:
        return default
    # 3. Any Latin-script language (better than CJK/RTL for Latin readers)
    for lang, text in by_lang.items():
        if lang.split("-")[0] in _LATIN_LANGS and text:
            return text
    # 4. Absolute last resort — first available
    return next((v for v in by_lang.values() if v), "")


def _lang_key(lang: str) -> str:
    """Return 2-letter language code for country-regional variants only.

    Strips 2-letter country subtags (en-GB → en, de-AT → de, pt-BR → pt).
    Does NOT strip script subtags (en-shaw, zh-Hans, be-Cyrl) — those
    would corrupt by_lang["en"] with Shaw/Cyrillic alphabet text.
    Returns "" for script variants so callers can skip adding to by_lang.
    """
    parts = lang.replace("_", "-").split("-")
    code = parts[0].lower()
    if len(parts) == 1:
        return code
    # Only strip if the subtag is exactly 2 alpha chars (ISO 3166 country code).
    # 4-letter script tags (shaw, Latn, Cyrl, Hans, Hant) are NOT stripped.
    if len(parts[1]) == 2 and parts[1].isalpha():
        return code
    return ""  # script/variant tag — caller should skip by_lang entry


def _get_localized(comp, tag: str) -> str:
    """Get localized text matching system locale with English fallback.

    AppStream XML facts (verified from real data):
      - English text is usually the untagged default element. Some Flatpak entries
        also have en-CA / en-GB (country variants) and en-shaw (Shaw alphabet) tags.
      - Script variants like en-shaw, zh-Hans, be-Cyrl must NOT be stripped to their
        base code — _lang_key() returns "" for them so they never pollute by_lang.
      - Regional variants use both POSIX (pt_BR) and BCP-47 (pt-BR) style tags.
      - We check the full locale in both forms so e.g. pt_BR users get Brazilian
        Portuguese, not European Portuguese.
    """
    els = comp.findall(tag)
    by_lang = {}       # normalised 2-letter code → text  e.g. "de" → "..."
    by_lang_full = {}  # full lowercased tag → text       e.g. "pt_br" → "..."
    default = ""
    for el in els:
        raw_lang = el.get(XML_LANG, "")
        text = el.text or ""
        if raw_lang == "":
            default = text
        else:
            by_lang_full[raw_lang.lower()] = text
            code = _lang_key(raw_lang)
            if code:  # skip script variants (en-shaw, zh-Hans, be-Cyrl…)
                by_lang[code] = text
    # 1. Full regional locale — both POSIX and BCP-47 forms
    # 2. 2-letter language code (country variants ok, script variants excluded)
    # 3. Untagged default (English for the vast majority of AppStream entries)
    result = (
        by_lang_full.get(_SYSLOCALE_US)    # "pt_br" matches xml:lang="pt_BR"
        or by_lang_full.get(_SYSLOCALE_HYP)  # "pt-br" matches xml:lang="pt-BR"
        or by_lang.get(SYSTEM_LANG)          # "pt" matches xml:lang="pt"
        or default
    )
    return result or _pick_lang(by_lang, default)


def _get_localized_description(comp) -> str:
    """Get localized description, handling nested p/ul/li tags."""
    by_lang = {}
    by_lang_full = {}
    default = ""
    for desc_el in comp.findall("description"):
        raw_lang = desc_el.get(XML_LANG, "")
        text = " ".join(desc_el.itertext()).strip()
        if raw_lang == "":
            default = text
        else:
            by_lang_full[raw_lang.lower()] = text
            code = _lang_key(raw_lang)
            if code:
                by_lang[code] = text
    result = (
        by_lang_full.get(_SYSLOCALE_US)
        or by_lang_full.get(_SYSLOCALE_HYP)
        or by_lang.get(SYSTEM_LANG)
        or default
    )
    return result or _pick_lang(by_lang, default)

# ── Package manager detection ─────────────────────────────────────────────────

def is_rakuos() -> bool:
    """True if running on RakuOS — packages.list and rakuos CLI present."""
    import shutil
    return PACKAGES_LIST.exists() or shutil.which("rakuos") is not None


def _get_pkg_manager() -> list[str]:
    """
    Return the best available package manager command as a list.
    Priority: rakuos CLI → dnf5 → dnf
    This allows the software center to work on any DNF-based distro.
    """
    import shutil
    if shutil.which("rakuos"):
        return ["rakuos"]
    if shutil.which("dnf5"):
        return ["dnf5"]
    if shutil.which("dnf"):
        return ["dnf"]
    return ["dnf"]


def _get_dnf() -> list[str]:
    """
    Return dnf5 or dnf directly — never rakuos.
    Used for read-only queries (repoquery, info) that rakuos doesn't support.
    """
    import shutil
    if shutil.which("dnf5"):
        return ["dnf5"]
    if shutil.which("dnf"):
        return ["dnf"]
    return ["dnf"]


def _get_install_cmd(pkg_name: str) -> list[str]:
    """Return the full privileged install command for pkg_name."""
    import shutil
    if shutil.which("rakuos"):
        return ["sudo", "/usr/libexec/rakuos/rakuos-install", pkg_name]
    mgr = _get_pkg_manager()
    return ["sudo"] + mgr + ["install", "-y", pkg_name]


def _get_remove_cmd(pkg_name: str) -> list[str]:
    """Return the full privileged remove command for pkg_name."""
    import shutil
    if shutil.which("rakuos"):
        return ["sudo", "/usr/libexec/rakuos/rakuos-remove", pkg_name]
    mgr = _get_pkg_manager()
    return ["sudo"] + mgr + ["remove", "-y", pkg_name]


PACKAGES_LIST = Path("/var/lib/rakuos/packages.list")

APPSTREAM_DIRS = [
    # RakuOS custom appstream entries (highest priority — loaded first)
    ("/usr/share/rakuos/appstream/data", "native"),
    ("/usr/share/swcatalog/xml", "native"),
    ("/usr/share/app-info/xmls", "native"),
    ("/var/cache/app-info/xmls", "native"),
    ("/var/lib/flatpak/appstream/flathub/x86_64/active", "flatpak"),
]

ICON_DIRS = [
    # RakuOS custom icons (checked first)
    "/usr/share/rakuos/appstream/icons",
    "/usr/share/swcatalog/icons/fedora/64x64",
    "/usr/share/swcatalog/icons/fedora/128x128",
    "/usr/share/swcatalog/icons/rpmfusion-free-43/64x64",
    "/usr/share/swcatalog/icons/rpmfusion-nonfree-43/64x64",
    # Terra (fyraLabs) repos — version read from os-release at import time
    *[f"/usr/share/swcatalog/icons/terra{_FEDORA_VER}{suffix}/{size}"
      for suffix in ("", "-mesa", "-nvidia", "-extras", "-multimedia")
      for size in ("64x64", "128x128")],
    "/var/lib/flatpak/appstream/flathub/x86_64/active/icons/64x64",
    "/var/lib/flatpak/appstream/flathub/x86_64/active/icons/128x128",
]

_appstream_cache: dict = {}

# ── AppStream data directory ───────────────────────────────────────────────────
# Both files ship at /usr/share/rakuos/appstream/ and are sourced from
# resources/appstream/ in the rakuos-software repo.
_APPSTREAM_DATA_DIR = "/usr/share/rakuos/appstream"
_OVERRIDES_PATH     = f"{_APPSTREAM_DATA_DIR}/appstream-overrides.json"
_FP_TO_RPM_PATH     = f"{_APPSTREAM_DATA_DIR}/flatpak-to-rpm.json"


def _load_overrides() -> dict:
    """Load appstream-overrides.json, return dict of app_id → override fields."""
    try:
        with open(_OVERRIDES_PATH, encoding="utf-8") as f:
            data = json.load(f)
        # Strip comment keys
        return {k: v for k, v in data.items() if not k.startswith("_")}
    except FileNotFoundError:
        return {}
    except Exception as e:
        print(f"[packages] overrides load error: {e}")
        return {}


def _load_flatpak_to_rpm() -> dict:
    """Load flatpak-to-rpm.json, return dict of lowercase flatpak_id → rpm_name."""
    try:
        with open(_FP_TO_RPM_PATH, encoding="utf-8") as f:
            data = json.load(f)
        # Strip comment keys, ensure all keys are lowercase
        return {k.lower(): v for k, v in data.items() if not k.startswith("_")}
    except FileNotFoundError:
        return {}
    except Exception as e:
        print(f"[packages] flatpak-to-rpm load error: {e}")
        return {}


def _apply_overrides(apps: dict, overrides: dict) -> None:
    """Apply metadata overrides to parsed AppStream entries in-place."""
    for app_id, fields in overrides.items():
        # Match against both the plain id and flatpak: prefixed key
        for key in (app_id, f"flatpak:{app_id}"):
            if key in apps:
                for field in ("name", "summary", "description"):
                    if fields.get(field):
                        apps[key][field] = fields[field]
_cache_lock = threading.Lock()
_cache_ready = threading.Event()


def get_installed_packages() -> list[str]:
    """
    Return list of user-installed packages.
    On RakuOS: reads /var/lib/rakuos/packages.list (overlay).
    On other distros: queries DNF for explicitly installed packages.
    """
    if PACKAGES_LIST.exists():
        return [
            p.strip() for p in PACKAGES_LIST.read_text().splitlines()
            if p.strip() and not p.strip().startswith("#")
        ]

    # Non-RakuOS fallback — ask DNF for explicitly installed packages
    # (userinstalled excludes base OS packages, giving a manageable list)
    mgr = _get_pkg_manager()
    for args in [
        _get_dnf() + ["repoquery", "--userinstalled", "--queryformat", "%{name}"],
        mgr + ["history", "userinstalled"],          # dnf4 fallback
    ]:
        try:
            r = subprocess.run(args, capture_output=True, text=True, timeout=20)
            if r.returncode == 0 and r.stdout.strip():
                return [p.strip() for p in r.stdout.splitlines() if p.strip()]
        except Exception:
            continue
    return []


def is_installed_native(pkg_name: str) -> bool:
    """Return True if pkg_name is actually installed on the system via rpm."""
    try:
        r = subprocess.run(
            ["rpm", "-q", "--quiet", pkg_name],
            capture_output=True, timeout=5
        )
        return r.returncode == 0
    except Exception:
        pass
    # Fallback — check packages.list
    if PACKAGES_LIST.exists():
        return pkg_name in get_installed_packages()
    return False


# Simple in-process cache so repeated calls during one AppStream load don't hammer dnf
_repo_exists_cache: dict[str, bool] = {}

def pkg_exists_in_repos(pkg_name: str) -> bool:
    """Return True if pkg_name exists in the configured dnf repos (or is already installed)."""
    if not pkg_name or "." in pkg_name.lstrip("."):
        # Looks like a Flatpak ID, not an RPM name
        return False
    if pkg_name in _repo_exists_cache:
        return _repo_exists_cache[pkg_name]
    # Installed packages always count
    if is_installed_native(pkg_name):
        _repo_exists_cache[pkg_name] = True
        return True
    try:
        r = subprocess.run(
            _get_dnf() + ["repoquery", "--queryformat", "%{name}", pkg_name],
            capture_output=True, text=True, timeout=10
        )
        found = any(l.strip() == pkg_name for l in r.stdout.splitlines())
        _repo_exists_cache[pkg_name] = found
        return found
    except Exception:
        _repo_exists_cache[pkg_name] = False
        return False


_flatpak_installed_cache: set[str] | None = None
_flatpak_installed_lock = threading.Lock()

def _get_flatpak_installed() -> set[str]:
    """Return set of installed Flatpak app IDs. Cached per process lifetime."""
    global _flatpak_installed_cache
    with _flatpak_installed_lock:
        if _flatpak_installed_cache is not None:
            return _flatpak_installed_cache
        try:
            result = subprocess.run(
                ["flatpak", "list", "--app", "--columns=application"],
                capture_output=True, text=True, timeout=10
            )
            _flatpak_installed_cache = {
                line.strip() for line in result.stdout.splitlines() if line.strip()
            }
        except Exception:
            _flatpak_installed_cache = set()
        return _flatpak_installed_cache


def invalidate_flatpak_cache() -> None:
    """Call after install/remove to force refresh of installed Flatpak list."""
    global _flatpak_installed_cache
    with _flatpak_installed_lock:
        _flatpak_installed_cache = None


def is_installed_flatpak(app_id: str) -> bool:
    """Return True only if app_id is an installed Flatpak application (not runtime/extension)."""
    return app_id in _get_flatpak_installed()


def install_package_stream(pkg_name: str):
    """Generator that yields output lines from install command.
    Uses rakuos CLI if available, falls back to dnf5/dnf."""
    try:
        proc = subprocess.Popen(
            _get_install_cmd(pkg_name),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True
        )
        for line in proc.stdout:
            yield line.rstrip()
        proc.wait()
        yield f"__done__{proc.returncode}"
    except Exception as e:
        yield f"Error: {e}"
        yield "__done__1"


def remove_package_stream(pkg_name: str):
    """Generator that yields output lines from remove command.
    Uses rakuos CLI if available, falls back to dnf5/dnf."""
    try:
        proc = subprocess.Popen(
            _get_remove_cmd(pkg_name),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True
        )
        for line in proc.stdout:
            yield line.rstrip()
        proc.wait()
        yield f"__done__{proc.returncode}"
    except Exception as e:
        yield f"Error: {e}"
        yield "__done__1"


# ── AppStream ──────────────────────────────────────────────────────────────

def preload_appstream():
    """Call this in a background thread at startup to preload the cache."""
    threading.Thread(target=_load_appstream, daemon=True).start()


def _load_appstream() -> dict:
    """Load and cache AppStream component data from system XML files."""
    global _appstream_cache
    with _cache_lock:
        if _appstream_cache:
            return _appstream_cache

        apps = {}

        for appstream_dir, source in APPSTREAM_DIRS:
            if not os.path.isdir(appstream_dir):
                continue
            for fname in os.listdir(appstream_dir):
                fpath = os.path.join(appstream_dir, fname)
                tree = None
                try:
                    if fname.endswith(".gz"):
                        with gzip.open(fpath, "rt", encoding="utf-8", errors="ignore") as fh:
                            tree = ET.parse(fh)
                    elif fname.endswith(".zst"):
                        import zstandard as zstd
                        with open(fpath, "rb") as fh:
                            data = zstd.ZstdDecompressor().stream_reader(fh)
                            tree = ET.parse(data)
                    elif fname.endswith(".xml"):
                        with open(fpath, "rt", encoding="utf-8", errors="ignore") as fh:
                            tree = ET.parse(fh)
                    else:
                        continue
                except Exception as e:
                    print(f"AppStream parse error {fname}: {e}")
                    continue

                try:
                    root = tree.getroot()
                    if root.tag == "component":
                        components = [root]
                    else:
                        components = root.findall("component")
                    for comp in components:
                        app = _parse_component(comp, source=source)
                        if not app:
                            continue
                        app_id = app["id"]
                        existing = apps.get(app_id)
                        if existing is None:
                            # No existing entry — just store it
                            apps[app_id] = app
                        elif existing["source"] == "native" and app["source"] == "flatpak":
                            # Flatpak AppStream overwriting a native entry —
                            # keep native metadata (has correct English) but
                            # enrich with Flatpak screenshots/icon if missing
                            if not existing.get("screenshots") and app.get("screenshots"):
                                existing["screenshots"] = app["screenshots"]
                            if not existing.get("icon") and app.get("icon"):
                                existing["icon"] = app["icon"]
                            # Store flatpak version under its own flatpak key too
                            apps[f"flatpak:{app_id}"] = app
                        else:
                            # Same source or flatpak→flatpak — overwrite normally
                            apps[app_id] = app
                except Exception as e:
                    print(f"AppStream process error {fname}: {e}")
                    continue

        # Apply community metadata overrides (correct English names etc.)
        overrides = _load_overrides()
        if overrides:
            _apply_overrides(apps, overrides)
            print(f"AppStream loaded {len(apps)} apps ({len(overrides)} overrides applied)")
        else:
            print(f"AppStream loaded {len(apps)} apps")

        # Inject native stubs for known Flatpak→RPM mappings that have no native AppStream entry.
        # Only inject if the RPM actually exists in repos — avoids ghost "installed" entries.
        for fp_id_key, rpm_name in _FLATPAK_TO_RPM.items():
            fp_id_lower = fp_id_key.lower()
            # Find the matching flatpak entry (case-insensitive id match)
            fp_entry = None
            for key, a in apps.items():
                if a.get("source") == "flatpak" and a.get("id", "").lower() == fp_id_lower:
                    fp_entry = a
                    break
            if fp_entry is None:
                continue
            # Skip if a native entry already exists with this pkg_name
            if any(a.get("source") == "native" and a.get("pkg_name") == rpm_name
                   for a in apps.values()):
                continue
            # Only inject if the RPM is actually installable or already installed
            if not pkg_exists_in_repos(rpm_name):
                continue
            # Inject a native stub using the Flatpak's rich metadata
            stub = dict(fp_entry)
            stub["source"] = "native"
            stub["pkg_name"] = rpm_name
            stub["id"] = rpm_name
            stub["installed"] = False  # enriched later by _enrich_installed → rpm -q
            stub.pop("origin", None)
            stub.pop("remote", None)
            native_key = f"native:{rpm_name}"
            apps[native_key] = stub
        _appstream_cache = apps
        _cache_ready.set()
        return apps


def _parse_component(comp, source: str = "native") -> Optional[dict]:
    """Parse a single AppStream component element."""
    kind = comp.get("type", "")
    is_addon = (kind == "addon")
    if kind not in ("desktop", "desktop-application", "console-application", "", "addon", "runtime", "generic", "codec"):
        return None
    # For non-standard types, only include if they have categories (real user-facing apps)
    if kind in ("runtime", "generic", "codec"):
        cats_el = comp.find("categories")
        if cats_el is None or not cats_el.findall("category"):
            return None

    app_id = (comp.findtext("id") or "").strip()
    if not app_id:
        return None

    name = _get_localized(comp, "name") or app_id
    summary = _get_localized(comp, "summary")

    description = _get_localized_description(comp)

    # Categories
    categories = []
    cats_el = comp.find("categories")
    if cats_el is not None:
        categories = [c.text.strip() for c in cats_el.findall("category") if c.text]

    # Icon — prefer cached then stock
    icon = ""
    for icon_el in comp.findall("icon"):
        if icon_el.get("type") == "cached" and not icon:
            icon = icon_el.text or ""
        elif icon_el.get("type") == "stock" and not icon:
            icon = icon_el.text or ""
    # Flatpak icons are always named after the full app id
    if source == "flatpak":
        icon = f"{app_id}.png"

    # Screenshots
    screenshots = []
    for ss in comp.findall(".//screenshot"):
        img = ss.find("image")
        if img is not None and img.text:
            screenshots.append(img.text.strip())

    # Package name
    if source == "flatpak":
        pkg_name = app_id
    else:
        raw_pkg = comp.findtext("pkgname")
        if not raw_pkg:
            clean_id = app_id.replace(".desktop", "")
            raw_pkg = clean_id.split(".")[-1].lower()
            pkg_name = raw_pkg
            # Flag that this pkg_name was guessed, not from XML — needs repo verification
            result_extra = {"pkg_name_guessed": True}
        else:
            pkg_name = raw_pkg
            result_extra = {}

    # Keywords
    keywords = []
    kw_el = comp.find("keywords")
    if kw_el is not None:
        keywords = [k.text.strip() for k in kw_el.findall("keyword") if k.text]

    # Project URL
    url = ""
    for url_el in comp.findall("url"):
        if url_el.get("type") == "homepage":
            url = url_el.text or ""
            break

    extends_id = (comp.findtext("extends") or "").strip()

    # result_extra only set for native with guessed pkg_name; flatpak path skips it
    if source == "flatpak":
        result_extra = {}

    result = {
        "id": app_id,
        "name": name,
        "summary": summary,
        "description": description,
        "categories": categories,
        "keywords": keywords,
        "icon": icon,
        "screenshots": screenshots,
        "pkg_name": pkg_name,
        "url": url,
        "source": source,
        "installed": False,
        "is_addon": is_addon,
    }
    result.update(result_extra)
    if extends_id:
        result["extends"] = extends_id
    return result


def _enrich_installed(app: dict) -> dict:
    """Add installed status to an app dict.
    Always re-checks the actual system state — never trusts cached installed value.
    """
    app = dict(app)
    if app["source"] == "flatpak":
        # Must use the full Flatpak app ID (e.g. org.kde.gwenview), not pkg_name
        flatpak_id = app.get("id") or app.get("pkg_name", "")
        app["installed"] = is_installed_flatpak(flatpak_id)
    else:
        # Use rpm -q on the RPM package name
        app["installed"] = is_installed_native(app["pkg_name"])
    return app


def _enrich_detail(app: dict) -> dict:
    """
    Fetch version, installed-size, and license for the detail page.
    For native packages: rpm -q (installed) or repoquery (not installed)
    For flatpak: flatpak info (installed) or flatpak remote-info (not installed)
    """
    if not app:
        return app
    app = dict(app)
    if app.get("local_rpm"):
        return app
    logging.debug("_enrich_detail called: pkg_name=%r source=%r installed=%r",
                  app.get("pkg_name"), app.get("source"), app.get("installed"))

    if app.get("source") == "flatpak":
        def _parse_flatpak_lines(text):
            for line in text.splitlines():
                if ":" not in line:
                    continue
                k, _, v = line.partition(":")
                k, v = k.strip().lower(), v.strip()
                if k == "version" and not app.get("version"):
                    app["version"] = v
                elif k in ("installed size", "download size") and not app.get("size"):
                    try:
                        parts = v.split()
                        num = float(parts[0].replace(",", "."))
                        unit = parts[1].upper() if len(parts) > 1 else "B"
                        mult = {"B": 1, "KB": 1024, "MB": 1024**2,
                                "MIB": 1024**2, "GB": 1024**3, "GIB": 1024**3}
                        app["size"] = int(num * mult.get(unit, 1))
                    except Exception:
                        pass

        if app.get("installed"):
            try:
                r = subprocess.run(
                    ["flatpak", "info", app["pkg_name"]],
                    capture_output=True, text=True, timeout=8
                )
                logging.debug("flatpak info: %r", r.stdout[:300])
                _parse_flatpak_lines(r.stdout)
            except Exception as e:
                logging.debug("flatpak info error: %s", e)
        else:
            remote = app.get("origin") or app.get("remote") or "flathub"
            try:
                r = subprocess.run(
                    ["flatpak", "remote-info", remote, app["pkg_name"]],
                    capture_output=True, text=True, timeout=10
                )
                logging.debug("flatpak remote-info: %r", r.stdout[:300])
                _parse_flatpak_lines(r.stdout)
            except Exception as e:
                logging.debug("flatpak remote-info error: %s", e)
    else:
        # DNF5 uses --qf with %{field} but different field names than rpm/dnf4
        # version/release work the same; size is 'downloadsize'/'installsize'; license is 'license'
        fmt_rpm   = "%{VERSION}-%{RELEASE}\t%{SIZE}\t%{LICENSE}"
        fmt_dnf5  = "%{version}-%{release}\t%{installsize}\t%{license}"

        def _parse_rpm_output(stdout):
            parts = stdout.strip().split("\t")
            if len(parts) >= 1 and parts[0] and "not installed" not in parts[0]:
                app["version"] = parts[0]
            if len(parts) >= 2:
                try:
                    app["size"] = int(parts[1])
                except Exception:
                    pass
            if len(parts) >= 3 and parts[2] not in ("", "(none)"):
                app["license"] = parts[2]

        def _try_rpm_q(name):
            """Try rpm -q with a name, return True if it worked."""
            try:
                r = subprocess.run(
                    ["rpm", "-q", "--queryformat", fmt_rpm, name],
                    capture_output=True, text=True, timeout=8
                )
                logging.debug("rpm -q %s: rc=%d out=%r", name, r.returncode, r.stdout[:200])
                if r.returncode == 0 and r.stdout.strip():
                    _parse_rpm_output(r.stdout)
                    return True
            except Exception as e:
                logging.debug("rpm -q error: %s", e)
            return False

        if app.get("installed"):
            if not _try_rpm_q(app["pkg_name"]):
                name_guess = app.get("name", "").lower().replace(" ", "-")
                if not _try_rpm_q(name_guess):
                    try:
                        frag = app["pkg_name"].lower()
                        r = subprocess.run(
                            ["rpm", "-qa", "--queryformat", f"%{{NAME}}\t{fmt_rpm}\n"],
                            capture_output=True, text=True, timeout=10
                        )
                        for line in r.stdout.splitlines():
                            parts = line.split("\t")
                            if len(parts) >= 1 and frag in parts[0].lower():
                                logging.debug("rpm -qa match: %s", parts[0])
                                app["pkg_name"] = parts[0]
                                _parse_rpm_output("\t".join(parts[1:]))
                                break
                    except Exception as e:
                        logging.debug("rpm -qa error: %s", e)
        else:
            def _try_repoquery(name):
                try:
                    mgr = _get_dnf()
                    is_dnf5 = (mgr == ["dnf5"])
                    if is_dnf5:
                        attempts = [("--qf", fmt_dnf5)]
                    else:
                        attempts = [("--queryformat", fmt_rpm)]
                    for qf_flag, qf_fmt in attempts:
                        r = subprocess.run(
                            mgr + ["repoquery", qf_flag, qf_fmt, name],
                            capture_output=True, text=True, timeout=20
                        )
                        logging.debug("repoquery %s %s: rc=%d out=%r err=%r",
                                      qf_flag, name, r.returncode,
                                      r.stdout[:200], r.stderr[:100])
                        if r.returncode == 0 and r.stdout.strip():
                            data_lines = [l for l in r.stdout.strip().splitlines()
                                          if "\t" in l]
                            if not data_lines:
                                # No tab-separated lines — try to grab version from any non-noise line
                                plain = [l.strip() for l in r.stdout.strip().splitlines()
                                         if l.strip() and not l.startswith("Updating")
                                         and not l.startswith("Repositories")]
                                if plain:
                                    app["version"] = plain[-1]
                                    return True
                                continue
                            parts = data_lines[0].split("\t")
                            if len(parts) >= 1 and parts[0]:
                                app["version"] = parts[0]
                            if len(parts) >= 2:
                                try:
                                    app["size"] = int(parts[1])
                                except Exception:
                                    pass
                            if len(parts) >= 3 and parts[2] not in ("", "(none)"):
                                app["license"] = parts[2]
                            return True
                except Exception as e:
                    logging.debug("repoquery exception: %s", e)
                return False

            if not _try_repoquery(app["pkg_name"]):
                name_guess = app.get("name", "").lower().replace(" ", "-")
                _try_repoquery(name_guess)

    logging.debug("_enrich_detail %s: version=%r size=%r license=%r",
                  app.get("pkg_name"), app.get("version"), app.get("size"), app.get("license"))
    return app


def search_dnf(query: str, limit: int = 20) -> list[dict]:
    """Search DNF metadata for packages not in AppStream."""
    try:
        # Use separate queries to avoid multiline description breaking parsing
        name_result = subprocess.run(
            _get_dnf() + ["repoquery", "--queryformat", "%{name}", f"*{query}*"],
            capture_output=True, text=True, timeout=15
        )
        names = [n.strip() for n in name_result.stdout.splitlines() if n.strip()]
        if not names:
            return []

        # Deduplicate and filter out meta/virtual packages
        # e.g. ardour6ardour7ardour8ardour9 — name+digit pattern repeated 3+ times
        seen = set()
        filtered = []
        for name in names:
            if name in seen:
                continue
            seen.add(name)
            # Skip names that look like concatenated versioned packages
            if re.search(r'([a-zA-Z]+\d+){3,}', name):
                continue
            filtered.append(name)
        names = filtered

        apps = _load_appstream()
        existing_pkgs = {a["pkg_name"] for a in apps.values()}
        results = []

        # Build a lookup of Flatpak apps by name for matching
        flatpak_by_name = {}
        for app in apps.values():
            if app["source"] == "flatpak":
                flatpak_by_name[app["name"].lower()] = app
                # Also index by last segment of app id e.g. "Lutris" from "net.lutris.Lutris"
                last_seg = app["id"].split(".")[-1].lower()
                flatpak_by_name[last_seg] = app

        for name in names:
            if name in existing_pkgs:
                continue

            # Check if a Flatpak entry already covers this package
            flatpak_match = flatpak_by_name.get(name.lower())
            if flatpak_match:
                # Use Flatpak metadata but mark as native source so it installs via DNF
                entry = dict(flatpak_match)
                entry["pkg_name"] = name
                entry["source"] = "native"
                entry["installed"] = is_installed_native(name)
                results.append(entry)
                if len(results) >= limit:
                    break
                continue

            # No Flatpak match — fetch details from DNF
            info = subprocess.run(
                _get_dnf() + ["repoquery", "--queryformat",
                 "%{summary}||END||%{url}", name],
                capture_output=True, text=True, timeout=10
            )
            parts = info.stdout.strip().split("||END||")
            summary = parts[0].strip() if parts else ""
            url = parts[1].strip() if len(parts) > 1 else ""

            desc = subprocess.run(
                _get_dnf() + ["repoquery", "--queryformat", "%{description}", name],
                capture_output=True, text=True, timeout=10
            )
            description = desc.stdout.strip()

            results.append({
                "id": name,
                "name": name,
                "summary": summary,
                "description": description,
                "categories": [],
                "icon": "",
                "screenshots": [],
                "pkg_name": name,
                "url": url,
                "source": "native",
                "installed": is_installed_native(name),
            })
            if len(results) >= limit:
                break
        return results
    except Exception as e:
        print(f"DNF search error: {e}")
        return []


def search_packages(query: str, limit: int = 40) -> list[dict]:
    """Search AppStream data by name/summary/id/keywords, supplemented by DNF.

    Results are ranked by relevance:
      6 — name starts with query (or exact match)
      5 — name contains query
      4 — pkg_name or app id contains query
      3 — summary contains query
      2 — any keyword contains query
    """
    apps = _load_appstream()
    q = query.lower()
    scored = []
    for app in apps.values():
        name_lc    = app.get("name", "").lower()
        summary_lc = app.get("summary", "").lower()
        pkg_lc     = app.get("pkg_name", "").lower()
        id_lc      = app.get("id", "").lower()
        keywords   = [k.lower() for k in app.get("keywords", [])]

        if name_lc.startswith(q):
            score = 6
        elif q in name_lc:
            score = 5
        elif q in pkg_lc or q in id_lc:
            score = 4
        elif q in summary_lc:
            score = 3
        elif any(q in kw for kw in keywords):
            score = 2
        else:
            continue

        scored.append((score, app))

    scored.sort(key=lambda x: -x[0])
    results = [_enrich_installed(app) for _, app in scored[:limit]]

    # Supplement with DNF for packages not in AppStream
    results += search_dnf(query, limit=20)
    return results


def get_by_category(category: str, limit: int = 40, offset: int = 0,
                    source: str = "all", parent_cat: str = "") -> dict:
    """Return apps matching category.

    When parent_cat is supplied (subcategory browsing) the app must have BOTH
    the subcategory AND the parent top-level category in its categories list,
    preventing bleed from unrelated top-level categories.
    """
    apps = _load_appstream()
    raw = []
    cat_lc    = category.lower()
    parent_lc = parent_cat.lower() if parent_cat else ""
    for app in apps.values():
        if source != "all" and app["source"] != source:
            continue
        cats_lc = [c.lower() for c in app["categories"]]
        if cat_lc not in cats_lc:
            continue
        if parent_lc and parent_lc not in cats_lc:
            continue
        raw.append(app)

    # Deduplicate by name+source — keep highest pkg_name version
    seen: dict = {}
    for app in raw:
        key = (app["name"].lower(), app["source"])
        existing = seen.get(key)
        if not existing or app["pkg_name"] > existing["pkg_name"]:
            seen[key] = app
    all_results = list(seen.values())

    total = len(all_results)
    page = [_enrich_installed(a) for a in all_results[offset:offset + limit]]
    return {"items": page, "total": total, "offset": offset, "limit": limit}


def get_installed_with_metadata() -> list[dict]:
    """Return installed overlay packages enriched with AppStream metadata where available."""
    installed = get_installed_packages()
    apps = _load_appstream()

    # Build pkg_name map from all AppStream entries (native + flatpak-sourced metadata)
    pkg_map: dict[str, dict] = {}
    for a in apps.values():
        if a.get("is_addon"):
            continue
        pn = a["pkg_name"]
        existing = pkg_map.get(pn)
        if existing is None:
            pkg_map[pn] = a
        elif existing.get("pkg_name_guessed") and not a.get("pkg_name_guessed"):
            pkg_map[pn] = a

    # Build Flatpak lookup by name and last ID segment for fallback matching
    flatpak_by_name = {}
    for app in apps.values():
        if app["source"] == "flatpak":
            flatpak_by_name[app["name"].lower()] = app
            last_seg = app["id"].split(".")[-1].lower()
            flatpak_by_name[last_seg] = app

    results = []
    for pkg in installed:
        # Verify RPM is actually installed via rpm -q
        if not is_installed_native(pkg):
            continue
        if pkg in pkg_map:
            app = dict(pkg_map[pkg])
            app["installed"] = True
            app["source"] = "native"
            results.append(app)
        else:
            # Fallback — borrow Flatpak AppStream metadata for icon/description
            flatpak_match = flatpak_by_name.get(pkg.lower())
            if flatpak_match:
                entry = dict(flatpak_match)
                entry["pkg_name"] = pkg
                entry["source"] = "native"
                entry["installed"] = True
                results.append(entry)
            else:
                results.append({
                    "id": pkg,
                    "name": pkg,
                    "summary": "Installed package",
                    "description": "",
                    "categories": [],
                    "icon": "",
                    "screenshots": [],
                    "pkg_name": pkg,
                    "url": "",
                    "source": "native",
                    "installed": True,
                })
    return results


def find_icon(filename: str) -> Optional[str]:
    """
    Find an icon in AppStream cached icon dirs and /usr/share/pixmaps.
    XDG icon theme lookup is handled in the widget via QIcon.fromTheme.
    """
    stem = filename
    for ext in (".png", ".svg", ".xpm"):
        if stem.endswith(ext):
            stem = stem[:-len(ext)]
            break

    # AppStream swcatalog cached icons
    for icon_dir in ICON_DIRS:
        for fname in (filename, stem + ".png", stem + ".svg"):
            fpath = os.path.join(icon_dir, fname)
            if os.path.exists(fpath):
                return fpath

    # /usr/share/pixmaps fallback
    for ext in (".png", ".svg", ".xpm", ""):
        fpath = os.path.join("/usr/share/pixmaps", stem + ext)
        if os.path.exists(fpath):
            return fpath

    return None

def get_addons_for(app_id: str) -> list[dict]:
    """
    Return AppStream addon components that extend the given app_id.
    Add-ons have <component type="addon"> with <extends>parent_id</extends>.
    Returns list of dicts with: id, name, summary, pkg_name, source, installed.
    """
    cache = _load_appstream()
    return [item for item in cache.values() if item.get("extends") == app_id]


# Flatpak ID → RPM name mappings — loaded from /usr/share/rakuos/appstream/flatpak-to-rpm.json
_FLATPAK_TO_RPM: dict[str, str] = _load_flatpak_to_rpm()


def find_native_counterpart(fp_app: dict) -> dict | None:
    """
    Given a Flatpak app dict, try to find a matching native RPM package.
    Returns a native-source app dict if found, None otherwise.
    Strategy:
      1. Known mapping table (_FLATPAK_TO_RPM)
      2. Last segment of app ID lowercased (org.mozilla.Firefox → firefox)
      3. Display name lowercased with spaces→hyphens (Firefox → firefox)
      4. Display name lowercased no spaces (VLC media player → vlcmediaplayer — skip)
    Only returns a result if repoquery confirms the package actually exists.
    """
    app_id = fp_app.get("id", "").lower()
    name = fp_app.get("name", "")

    candidates = []

    # 1. Known mapping
    if app_id in _FLATPAK_TO_RPM:
        candidates.append(_FLATPAK_TO_RPM[app_id])

    # 2. Last segment of reverse-domain ID
    last_seg = app_id.split(".")[-1].lower()
    if last_seg and last_seg not in candidates:
        candidates.append(last_seg)

    # 3. Display name → rpm name guesses
    name_lower = name.lower()
    name_hyphen = name_lower.replace(" ", "-")
    name_nospace = name_lower.replace(" ", "")
    for guess in (name_lower, name_hyphen):
        if guess and guess not in candidates:
            candidates.append(guess)

    # Check each candidate against repoquery
    for pkg in candidates:
        try:
            r = subprocess.run(
                _get_dnf() + ["repoquery", "--queryformat", "%{name}", pkg],
                capture_output=True, text=True, timeout=10
            )
            matched = [l.strip() for l in r.stdout.splitlines()
                       if l.strip() and l.strip() == pkg]
            if matched:
                # Build a native app dict using the Flatpak's rich metadata
                native = dict(fp_app)
                native["source"] = "native"
                native["pkg_name"] = pkg
                native["installed"] = is_installed_native(pkg)
                native.pop("origin", None)
                native.pop("remote", None)
                return native
        except Exception:
            continue
    return None


def get_local_rpm_info(rpm_path: str) -> dict:
    """
    Extract metadata from a local .rpm file using rpm -qip.
    Returns an app dict compatible with the detail page.
    Falls back gracefully if rpm isn't available.
    """
    fields = {
        "Name":        "%{NAME}",
        "Version":     "%{VERSION}-%{RELEASE}",
        "Summary":     "%{SUMMARY}",
        "Description": "%{DESCRIPTION}",
        "URL":         "%{URL}",
        "License":     "%{LICENSE}",
        "Size":        "%{SIZE}",
        "Arch":        "%{ARCH}",
    }
    fmt = "\n".join(f"{k}: {v}" for k, v in fields.items())
    try:
        r = subprocess.run(
            ["rpm", "--queryformat", fmt, "-qp", rpm_path],
            capture_output=True, text=True, timeout=10
        )
        data = {}
        for line in r.stdout.splitlines():
            if ": " in line:
                k, _, v = line.partition(": ")
                data[k.strip()] = v.strip()
    except Exception:
        data = {}

    name    = data.get("Name", os.path.basename(rpm_path).replace(".rpm", ""))
    version = data.get("Version", "")
    summary = data.get("Summary", "")
    desc    = data.get("Description", "")
    url     = data.get("URL", "") if data.get("URL", "") != "(none)" else ""
    size    = int(data.get("Size", 0) or 0)

    # Check if already installed
    already_installed = is_installed_native(name)

    return {
        "id":           name,
        "name":         name,
        "version":      version,
        "summary":      summary,
        "description":  desc,
        "categories":   [],
        "icon":         "",
        "screenshots":  [],
        "pkg_name":     name,
        "url":          url,
        "source":       "native",
        "local_rpm":    rpm_path,       # signals detail page this is a local file
        "installed":    already_installed,
        "size":         size,
        "license":      data.get("License", ""),
        "arch":         data.get("Arch", ""),
    }


def install_local_rpm_stream(rpm_path: str):
    """
    Generator that yields output lines from installing a local .rpm file.
    Uses dnf5/dnf to install so deps are resolved automatically.
    """
    mgr = _get_pkg_manager()
    cmd = mgr + ["install", "-y", rpm_path]
    try:
        proc = subprocess.Popen(
            ["pkexec"] + cmd,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
        )
        for line in proc.stdout:
            yield line.rstrip()
        proc.wait()
        yield f"__done__{proc.returncode}"
    except Exception as e:
        yield f"Error: {e}"
        yield "__done__1"


# ── Flathub icon cache ────────────────────────────────────────────────────────
_ICON_CACHE_DIR = os.path.join(os.path.expanduser("~"), ".cache", "rakuos", "icons")
os.makedirs(_ICON_CACHE_DIR, exist_ok=True)


def get_flathub_icon_url(app_id: str) -> Optional[str]:
    """
    Fetch the icon URL for an app from the Flathub API.
    Returns a direct PNG URL or None.
    Caches the URL on disk so subsequent calls are instant.
    """
    import json, urllib.request, urllib.error

    if not app_id:
        return None

    cache_file = os.path.join(_ICON_CACHE_DIR, f"{app_id}.url")

    # Return cached URL if we have it (even empty string = known miss)
    if os.path.exists(cache_file):
        cached = open(cache_file).read().strip()
        return cached if cached else None

    try:
        url = f"https://flathub.org/api/v2/appstream/{app_id}"
        req = urllib.request.Request(url, headers={"User-Agent": "RakuOS-Software/1.0"})
        with urllib.request.urlopen(req, timeout=8) as r:
            data = json.loads(r.read())

        # API returns icon URLs under data["icon"] or data["icons"]
        icon_url = ""
        if data.get("icon"):
            icon_url = data["icon"]
        elif data.get("icons"):
            icons = data["icons"]
            # Prefer 128x128, fall back to any available
            icon_url = (icons.get("128") or icons.get("64") or
                        next(iter(icons.values()), ""))

        # Write to cache (empty string = confirmed miss)
        with open(cache_file, "w") as f:
            f.write(icon_url)

        return icon_url if icon_url else None

    except Exception as e:
        logging.debug("flathub icon API error for %s: %s", app_id, e)
        # Cache the miss so we don't hammer the API
        with open(cache_file, "w") as f:
            f.write("")
        return None


def get_cached_icon_path(app_id: str) -> Optional[str]:
    """Return path to a locally cached icon PNG, or None if not cached."""
    path = os.path.join(_ICON_CACHE_DIR, f"{app_id}.png")
    return path if os.path.exists(path) else None


def save_icon_to_cache(app_id: str, data: bytes) -> str:
    """Save raw image bytes to the icon cache and return the path."""
    path = os.path.join(_ICON_CACHE_DIR, f"{app_id}.png")
    with open(path, "wb") as f:
        f.write(data)
    return path
