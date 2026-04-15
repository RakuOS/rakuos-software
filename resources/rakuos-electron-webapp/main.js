/**
 * rakuos-webapp — Electron main process
 *
 * Usage: electron /usr/lib/rakuos-electron-webapp/ <url> <name> [custom_css] [session_group] [file]
 * Uses castlabs Electron (ECS) with Widevine fully wired up.
 */

const { app, BrowserWindow, Menu, Tray, components, nativeImage, shell } = require('electron');
const path = require('path');
const fs   = require('fs');

console.log('[rakuos-webapp] process.argv:', process.argv);

// MIME type map for local file open
const MIME_TYPES = {
    '.docx': 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    '.doc':  'application/msword',
    '.xlsx': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    '.xls':  'application/vnd.ms-excel',
    '.pptx': 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    '.ppt':  'application/vnd.ms-powerpoint',
    '.odt':  'application/vnd.oasis.opendocument.text',
    '.ods':  'application/vnd.oasis.opendocument.spreadsheet',
    '.odp':  'application/vnd.oasis.opendocument.presentation',
    '.pdf':  'application/pdf',
    '.txt':  'text/plain',
    '.csv':  'text/csv',
};

function sanitizeAppId(input) {
    return String(input || '')
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-|-$/g, '');
}

function parseNamedArgs(argv) {
    const parsed = {};
    for (let i = 1; i < argv.length; i++) {
        const arg = argv[i];
        if (!arg.startsWith('--')) continue;
        const key = arg.slice(2);
        const value = argv[i + 1];
        if (typeof value === 'string' && !value.startsWith('--')) {
            parsed[key] = value;
            i += 1;
        } else {
            parsed[key] = 'true';
        }
    }
    return parsed;
}

function loadSidecarConfig(configPath) {
    if (!configPath || !fs.existsSync(configPath)) return {};
    try {
        const parsed = JSON.parse(fs.readFileSync(configPath, 'utf8'));
        return parsed && typeof parsed === 'object' ? parsed : {};
    } catch (error) {
        console.warn('[rakuos-webapp] Failed to read config file:', configPath, error);
        return {};
    }
}

function normaliseTrayConfig(tray) {
    if (!tray || typeof tray !== 'object') {
        return { enabled: false, close_to_tray: false, menu: [] };
    }
    const allowedActions = new Set(['separator', 'navigate', 'open_external', 'reload', 'click_selector']);
    const menu = Array.isArray(tray.menu) ? tray.menu
        .filter(item => item && typeof item === 'object' && allowedActions.has(item.action))
        .map(item => ({
            id:       typeof item.id === 'string' ? item.id : '',
            label:    typeof item.label === 'string' ? item.label : '',
            action:   item.action,
            target:   typeof item.target === 'string' ? item.target : '',
            selector: typeof item.selector === 'string' ? item.selector : '',
        }))
        : [];
    return {
        enabled: tray.enabled === true,
        close_to_tray: tray.close_to_tray === true,
        menu,
    };
}

// Find named args first, then fall back to the old positional format.
function parseArgs() {
    const named = parseNamedArgs(process.argv);
    if (named.url || named.name) {
        return {
            url:          named.url || '',
            name:         named.name || 'Web App',
            css:          named.css || '',
            sessionGroup: named['session-group'] || '',
            fileArg:      named.file && fs.existsSync(named.file) ? named.file : '',
            appId:        named['app-id'] || '',
            configFile:   named['config-file'] || '',
        };
    }

    const argv = process.argv;
    for (let i = 1; i < argv.length; i++) {
        if (argv[i].startsWith('http://') || argv[i].startsWith('https://')) {
            const fileArg = argv[i + 4] || '';
            return {
                url:          argv[i],
                name:         argv[i + 1] || 'Web App',
                css:          argv[i + 2] || '',
                sessionGroup: argv[i + 3] || '',
                fileArg:      fileArg && fs.existsSync(fileArg) ? fileArg : '',
                appId:        argv[i + 5] || '',
                configFile:   '',
            };
        }
    }
    console.error('[rakuos-webapp] No URL found in argv:', argv);
    app.quit();
    return null;
}

const parsed = parseArgs();
if (!parsed) process.exit(1);

const sidecarConfig = loadSidecarConfig(parsed.configFile);
const targetUrl = parsed.url || sidecarConfig.url || '';
const appName = parsed.name || sidecarConfig.name || 'Web App';
const customCss = sidecarConfig.custom_css || parsed.css || '';
const sessionGroup = parsed.sessionGroup || sidecarConfig.session_group || '';
const fileArg = parsed.fileArg;
const appId = parsed.appId || sidecarConfig.id || sanitizeAppId(appName);
const trayConfig = normaliseTrayConfig(sidecarConfig.tray);

console.log('[rakuos-webapp] Launching:', targetUrl, '|', appName, fileArg ? `| file: ${fileArg}` : '');

// Suite members share a userData dir (keyed on sessionGroup) so Electron stores
// cookies/localStorage in the same Partitions sub-directory — giving them a
// truly shared session.  Standalone apps use their own appId-keyed dir.
const groupId = sessionGroup || appId;
const dataDir = path.join(
    app.getPath('home'),
    '.local', 'share', 'rakuos', 'webapps', 'electron-data', groupId
);
fs.mkdirSync(dataDir, { recursive: true });
app.setPath('userData', dataDir);
app.setName(appName);  // use real app name so taskbar shows it correctly

// Window size persistence — per-app file inside the (possibly shared) dataDir
const stateFile = path.join(dataDir, `window-state-${appId}.json`);

function loadWindowState() {
    try {
        const s = JSON.parse(fs.readFileSync(stateFile, 'utf8'));
        // Validate saved values are reasonable
        if (s.width > 400 && s.height > 300) return s;
    } catch (_) {}
    // Default: 75% of primary display
    const { screen } = require('electron');
    const { width, height } = screen.getPrimaryDisplay().workAreaSize;
    return {
        width:  Math.round(width  * 0.75),
        height: Math.round(height * 0.75),
    };
}

function saveWindowState(win) {
    if (win.isMaximized() || win.isMinimized()) return;
    const b = win.getBounds();
    fs.writeFileSync(stateFile, JSON.stringify({
        width:  b.width,
        height: b.height,
        x:      b.x,
        y:      b.y,
    }));
}

// Chromium flags — must be before ready
app.commandLine.appendSwitch('autoplay-policy', 'no-user-gesture-required');
app.commandLine.appendSwitch('enable-features',
    'WidevineDrm,PlatformEncryptedMediaExtensions,HardwareSecureDecryption');
app.commandLine.appendSwitch('disable-features', 'MediaSessionService');

let win = null;
let tray = null;
let isQuitting = false;

function showWindow() {
    if (!win) return;
    if (win.isMinimized()) {
        win.restore();
    }
    win.show();
    win.focus();
}

function hideWindow() {
    if (!win) return;
    win.hide();
}

function resolveNavigationTarget(target) {
    if (!target) return targetUrl;
    try {
        return new URL(target, targetUrl).toString();
    } catch (_) {
        return target;
    }
}

function runSelectorAction(selector) {
    if (!win || !selector) return;
    showWindow();
    win.webContents.executeJavaScript(`
        (function() {
            const el = document.querySelector(${JSON.stringify(selector)});
            if (!el) {
                console.warn('[rakuos-webapp] tray selector not found:', ${JSON.stringify(selector)});
                return false;
            }
            el.click();
            return true;
        })();
    `).catch(error => {
        console.error('[rakuos-webapp] tray click_selector failed:', error);
    });
}

function executeTrayMenuItem(item) {
    if (!item || !item.action) return;
    if (item.action === 'navigate') {
        if (!win) return;
        const dest = resolveNavigationTarget(item.target);
        showWindow();
        win.loadURL(dest).catch(error => {
            console.error('[rakuos-webapp] tray navigate failed:', dest, error);
        });
    } else if (item.action === 'open_external') {
        if (item.target) {
            shell.openExternal(item.target).catch(error => {
                console.error('[rakuos-webapp] tray open_external failed:', item.target, error);
            });
        }
    } else if (item.action === 'reload') {
        if (win) {
            showWindow();
            win.webContents.reload();
        }
    } else if (item.action === 'click_selector') {
        runSelectorAction(item.selector);
    }
}

function buildTrayMenu() {
    const items = [];
    for (const item of trayConfig.menu) {
        if (item.action === 'separator') {
            items.push({ type: 'separator' });
            continue;
        }
        items.push({
            label: item.label || 'Action',
            click: () => executeTrayMenuItem(item),
        });
    }
    if (items.length > 0) {
        items.push({ type: 'separator' });
    }
    items.push({
        label: `Open ${appName}`,
        click: () => showWindow(),
    });
    items.push({
        label: `Quit ${appName}`,
        click: () => {
            isQuitting = true;
            app.quit();
        },
    });
    return Menu.buildFromTemplate(items);
}

function createTray() {
    if (!trayConfig.enabled || tray) return;

    const iconPath = path.join(
        app.getPath('home'),
        '.local', 'share', 'rakuos', 'webapps', 'icons', `${appId}.png`
    );
    let trayIcon = fs.existsSync(iconPath)
        ? nativeImage.createFromPath(iconPath)
        : nativeImage.createEmpty();
    if (!trayIcon.isEmpty()) {
        trayIcon = trayIcon.resize({ width: 22, height: 22 });
    }

    tray = new Tray(trayIcon.isEmpty() ? nativeImage.createEmpty() : trayIcon);
    tray.setToolTip(appName);
    tray.setContextMenu(buildTrayMenu());
    tray.on('click', () => {
        if (!win) return;
        if (win.isVisible()) hideWindow();
        else showWindow();
    });
    tray.on('right-click', () => {
        tray.popUpContextMenu(buildTrayMenu());
    });
}

async function createWindow() {
    // Wait for Widevine CDM (castlabs ECS)
    if (components && typeof components.whenReady === 'function') {
        try {
            await components.whenReady();
            console.log('[rakuos-webapp] Widevine status:', components.status());
        } catch (e) {
            console.warn('[rakuos-webapp] components.whenReady error:', e);
        }
    }

    const state = loadWindowState();

    // Load app icon from the cached webapps icon dir
    const iconPath = path.join(
        app.getPath('home'),
        '.local', 'share', 'rakuos', 'webapps', 'icons', `${appId}.png`
    );
    const icon = fs.existsSync(iconPath)
        ? nativeImage.createFromPath(iconPath)
        : nativeImage.createEmpty();

    win = new BrowserWindow({
        width:           state.width,
        height:          state.height,
        x:               state.x,
        y:               state.y,
        title:           appName,
        icon:            icon,          // taskbar + title bar icon
        autoHideMenuBar: true,
        show:            false,         // show only on ready-to-show
        webPreferences: {
            plugins:          true,
            contextIsolation: false,
            nodeIntegration:  false,
            webSecurity:      true,
            // Share session (cookies, storage) across suite members; standalone apps get their own
            partition:        `persist:${sessionGroup || appId}`,
        },
    });

    // If a local file was passed, read it in the main process and inject as base64
    // so the renderer can reconstruct a File object without any protocol plumbing.
    if (fileArg) {
        const fileExt  = path.extname(fileArg).toLowerCase();
        const fileName = path.basename(fileArg);
        const mimeType = MIME_TYPES[fileExt] || 'application/octet-stream';

        let fileBase64 = '';
        try {
            fileBase64 = fs.readFileSync(fileArg).toString('base64');
            console.log(`[rakuos-webapp] Loaded file for injection: ${fileName} (${fileBase64.length} b64 chars)`);
        } catch (e) {
            console.error('[rakuos-webapp] Failed to read file arg:', e);
        }

        if (fileBase64) {
            // Override window.showOpenFilePicker on every dom-ready / navigation so
            // it returns our file instead of showing the OS file dialog.
            const injectFileOverride = () => {
                const safeFileName = fileName.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
                const safeMime     = mimeType.replace(/"/g, '\\"');
                win.webContents.executeJavaScript(`
                    (function() {
                        if (window.__rakuos_file_injected) return;
                        window.__rakuos_file_injected = true;

                        const _b64 = "${fileBase64}";
                        function _buildFile() {
                            try {
                                const bin  = atob(_b64);
                                const u8   = new Uint8Array(bin.length);
                                for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
                                return new File([u8], "${safeFileName}", { type: "${safeMime}" });
                            } catch(e) {
                                console.error('[rakuos-webapp] base64 decode error:', e);
                                return null;
                            }
                        }

                        window.showOpenFilePicker = async function() {
                            const file = _buildFile();
                            if (!file) throw new DOMException('File build failed', 'AbortError');
                            console.log('[rakuos-webapp] showOpenFilePicker returning injected file:', file.name, file.size);
                            return [{
                                kind:              'file',
                                name:              "${safeFileName}",
                                getFile:           async () => file,
                                isSameEntry:       async () => false,
                                queryPermission:   async () => 'granted',
                                requestPermission: async () => 'granted',
                                createWritable:    async () => ({
                                    write:  async () => {},
                                    close:  async () => {},
                                    abort:  async () => {},
                                }),
                            }];
                        };
                        console.log('[rakuos-webapp] showOpenFilePicker overridden for: ${safeFileName}');
                    })();
                `).catch(console.error);
            };

            win.webContents.on('dom-ready', injectFileOverride);
            win.webContents.on('did-navigate', injectFileOverride);

            // Auto-click the upload button once the page finishes loading
            win.webContents.on('did-finish-load', () => {
                setTimeout(() => {
                    win.webContents.executeJavaScript(`
                        (function() {
                            const selectors = [
                                "button[data-testid='0301']",   // Microsoft Word/Excel/PPT upload
                                "button[aria-label*='pload']",
                                "input[type='file']",
                            ];
                            for (const sel of selectors) {
                                const el = document.querySelector(sel);
                                if (el) {
                                    console.log('[rakuos-webapp] auto-clicking upload:', sel);
                                    el.click();
                                    return;
                                }
                            }
                            for (const btn of document.querySelectorAll('button, [role="button"]')) {
                                const t = (btn.innerText || btn.textContent || '').toLowerCase();
                                if (t.includes('upload') || t.includes('open file')) {
                                    console.log('[rakuos-webapp] auto-clicking upload by text:', t.trim());
                                    btn.click();
                                    return;
                                }
                            }
                            console.log('[rakuos-webapp] no upload button found — user must click manually');
                        })();
                    `).catch(console.error);
                }, 1500);
            });
        }
    }

    // Wayland: set app_id so compositor uses the right icon
    if (process.platform === 'linux') {
        win.setTitle(appName);
        // app.setName() above handles WM_CLASS for X11
        // For Wayland, pass --ozone-platform-hint=auto and set the icon
        app.commandLine.appendSwitch('ozone-platform-hint', 'auto');
    }

    // Show window once page starts rendering
    win.once('ready-to-show', () => win.show());

    // Save window size/position on resize and move
    win.on('resize', () => saveWindowState(win));
    win.on('move',   () => saveWindowState(win));
    win.on('close',  () => saveWindowState(win));

    // Spoof UA to real Chrome
    const chromeVer = process.versions.chrome || '124.0.0.0';
    win.webContents.setUserAgent(
        `Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 ` +
        `(KHTML, like Gecko) Chrome/${chromeVer} Safari/537.36`
    );

    // Inject custom CSS as early as possible (dom-ready fires when HTML is
    // parsed, before external resources — eliminates the flash of unstyled content)
    if (customCss) {
        const injectCss = () => win.webContents.insertCSS(customCss).catch(console.error);
        win.webContents.on('dom-ready', injectCss);
        // Re-inject on full navigations (multi-page sites)
        win.webContents.on('did-navigate', injectCss);
    }

    win.webContents.on('did-fail-load', (_event, code, desc, url) => {
        console.error(`[rakuos-webapp] Load failed: ${code} ${desc} — ${url}`);
    });

    win.webContents.on('did-finish-load', () => {
        console.log('[rakuos-webapp] Page loaded:', win.webContents.getURL());
    });

    // Keep title bar as app name
    win.on('page-title-updated', (e) => e.preventDefault());

    console.log('[rakuos-webapp] Loading URL:', targetUrl);
    win.loadURL(targetUrl).catch(e => {
        console.error('[rakuos-webapp] loadURL error:', e);
    });

    win.on('close', (event) => {
        saveWindowState(win);
        if (trayConfig.enabled && trayConfig.close_to_tray && !isQuitting) {
            event.preventDefault();
            hideWindow();
        }
    });

    win.on('closed', () => { win = null; });
}

app.whenReady().then(async () => {
    createTray();
    await createWindow();
});

app.on('before-quit', () => {
    isQuitting = true;
});

app.on('window-all-closed', () => {
    if (!trayConfig.enabled) {
        app.quit();
    }
});

app.on('activate', () => {
    if (!win) {
        createWindow();
    } else {
        showWindow();
    }
});
