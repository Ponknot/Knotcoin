const { app, BrowserWindow, Tray, Menu, nativeImage, globalShortcut } = require('electron');
app.setName('Knotcoin');

// Auto-updater is optional - only load if available (for packaged builds)
let autoUpdater = null;
try {
  autoUpdater = require('electron-updater').autoUpdater;
} catch (e) {
  console.log('[updater] electron-updater not available (dev mode or missing dependency)');
}

const path = require('path');
const os = require('os');
const fs = require('fs');
const http = require('http');
const { spawn } = require('child_process');

// Suppress EPIPE errors when knotcoind exits unexpectedly
process.on('uncaughtException', (err) => {
  if (err.code === 'EPIPE' || err.message === 'write EPIPE') return;
  console.error('[fatal]', err);
  app.quit();
});

const RPC_PORT = 9001;
const P2P_PORT = 9000;
const PROXY_PORT = 19001;

// Creator seed node (default bootstrap peer). Can be overridden via:
// - env: KNOTCOIN_BOOTSTRAP_PEERS
// - config: <userData>/knotcoin-desktop.json { "bootstrapPeers": "ip:port,ip:port" }

function resolveTrayIconPath() {
  // Prefer a bundled resource if present; fallback to repo icon in dev.
  const packaged = app.isPackaged ? process.resourcesPath : null;
  const candidates = [];

  if (packaged) {
    candidates.push(path.join(packaged, 'icon.png'));
    candidates.push(path.join(packaged, 'explorer', 'icon.png'));
  }

  candidates.push(path.join(__dirname, '..', 'src-tauri', 'icons', 'icon.png'));

  for (const p of candidates) {
    try {
      if (fs.existsSync(p)) return p;
    } catch (_) { }
  }
  return null;
}

function setupTray() {
  if (tray) return;

  const iconPath = resolveTrayIconPath();
  if (!iconPath) {
    console.warn('[main] No tray icon found, skipping Tray creation');
    return;
  }
  const icon = nativeImage.createFromPath(iconPath);
  tray = new Tray(icon);
  tray.setToolTip('Knotcoin (node running in background)');

  const buildMenu = () =>
    Menu.buildFromTemplate([
      {
        label: mainWindow && mainWindow.isVisible() ? 'Hide' : 'Show',
        click: () => {
          if (!mainWindow) return;
          if (mainWindow.isVisible()) {
            mainWindow.hide();
          } else {
            mainWindow.show();
            mainWindow.focus();
          }
        }
      },
      { type: 'separator' },
      {
        label: 'Quit',
        click: () => {
          isQuitting = true;
          app.quit();
        }
      }
    ]);

  tray.setContextMenu(buildMenu());
  tray.on('click', () => {
    if (!mainWindow) return;
    if (mainWindow.isVisible()) {
      mainWindow.hide();
    } else {
      mainWindow.show();
      mainWindow.focus();
    }
    tray.setContextMenu(buildMenu());
  });
}
// Default bootstrap peers - DNS-based for privacy
// Users can override via KNOTCOIN_BOOTSTRAP_PEERS environment variable
const DEFAULT_BOOTSTRAP_PEERS = "seed.knotcoin.network:9000";

let knotcoindProcess = null;
let proxyServer = null;
let mainWindow = null;
let tray = null;
let isQuitting = false;

function setupAutoUpdates() {
  // Skip if electron-updater not available
  if (!autoUpdater) {
    console.log('[updater] Auto-updates disabled (electron-updater not available)');
    return;
  }
  
  // electron-updater uses GitHub Releases when build.publish is set.
  // This keeps user installs "one-time" with incremental updates afterwards.
  autoUpdater.logger = console;
  autoUpdater.autoDownload = true;

  autoUpdater.on('checking-for-update', () => console.log('[updater] checking for update...'));
  autoUpdater.on('update-available', (info) => console.log('[updater] update available', info && info.version));
  autoUpdater.on('update-not-available', () => console.log('[updater] update not available'));
  autoUpdater.on('error', (err) => console.error('[updater] error', err));
  autoUpdater.on('download-progress', (p) => {
    console.log(`[updater] download ${Math.round(p.percent)}% (${p.transferred}/${p.total})`);
  });
  autoUpdater.on('update-downloaded', () => {
    console.log('[updater] update downloaded; will install on quit');
  });

  // Non-blocking; safe to ignore failures when running unsigned/dev.
  try {
    autoUpdater.checkForUpdatesAndNotify();
  } catch (e) {
    console.warn('[updater] check failed:', e && e.message ? e.message : e);
  }
}

function getDataDir() {
  // Matches Rust: ~/.knotcoin/mainnet
  const home = os.homedir();
  return process.env.KNOTCOIN_DATA_DIR || path.join(home, '.knotcoin', 'mainnet');
}

function getCookiePath() {
  return path.join(getDataDir(), '.cookie');
}

function getDesktopConfigPath() {
  try {
    return path.join(app.getPath('userData'), 'knotcoin-desktop.json');
  } catch (_) {
    return null;
  }
}

function readBootstrapPeersFromConfig() {
  const p = getDesktopConfigPath();
  if (!p) return null;
  try {
    const raw = fs.readFileSync(p, 'utf8');
    const cfg = JSON.parse(raw);
    if (cfg && typeof cfg.bootstrapPeers === 'string' && cfg.bootstrapPeers.trim().length > 0) {
      return cfg.bootstrapPeers.trim();
    }
  } catch (_) {
    // ignore
  }
  return null;
}

function resolveKnotcoindBinaryPath() {
  // In packaged builds, electron-builder copies extraResources under process.resourcesPath
  const base = app.isPackaged ? process.resourcesPath : __dirname;

  const candidates = [];
  if (process.platform === 'win32') {
    candidates.push(path.join(base, 'binaries', 'knotcoind.exe'));
    candidates.push(path.join(base, 'binaries', 'knotcoind-x86_64-pc-windows-msvc.exe'));
  } else if (process.platform === 'darwin') {
    // Apple Silicon build is the primary target
    candidates.push(path.join(base, 'binaries', 'knotcoind'));
    candidates.push(path.join(base, 'binaries', 'knotcoind-aarch64-apple-darwin'));
  } else {
    candidates.push(path.join(base, 'binaries', 'knotcoind'));
    candidates.push(path.join(base, 'binaries', 'knotcoind-x86_64-unknown-linux-gnu'));
  }

  for (const p of candidates) {
    try {
      if (fs.existsSync(p)) return p;
    } catch (_) { }
  }

  return null;
}

async function isKnotcoindRunning() {
  // Check if knotcoind is already running by trying to connect to RPC port
  return new Promise((resolve) => {
    const req = http.request({
      hostname: '127.0.0.1',
      port: RPC_PORT,
      method: 'POST',
      timeout: 1000
    }, (res) => {
      resolve(true); // Got a response, server is running
    });
    req.on('error', () => resolve(false));
    req.on('timeout', () => { req.destroy(); resolve(false); });
    req.end();
  });
}

function startKnotcoind() {
  if (knotcoindProcess) return;

  const bin = resolveKnotcoindBinaryPath();
  if (!bin) {
    throw new Error('knotcoind binary not found. Place it in electron/binaries/ (it will be bundled by electron-builder).');
  }

  // Ensure datadir exists
  fs.mkdirSync(getDataDir(), { recursive: true });

  const bootstrapPeers =
    process.env.KNOTCOIN_BOOTSTRAP_PEERS ||
    readBootstrapPeersFromConfig() ||
    DEFAULT_BOOTSTRAP_PEERS;

  knotcoindProcess = spawn(bin, [`--rpc-port=${RPC_PORT}`, `--p2p-port=${P2P_PORT}`], {
    stdio: ['ignore', 'pipe', 'pipe'],
    env: {
      ...process.env,
      // Ensure consistent data dir for cookie discovery
      KNOTCOIN_DATA_DIR: getDataDir(),
      KNOTCOIN_RPC_PORT: String(RPC_PORT),
      KNOTCOIN_P2P_PORT: String(P2P_PORT),
      // Early-network growth: always bootstrap from creator seed unless overridden.
      KNOTCOIN_BOOTSTRAP_PEERS: bootstrapPeers
    }
  });

  knotcoindProcess.stdout.on('data', (d) => {
    try { console.log(`[knotcoind] ${String(d).trimEnd()}`); } catch (_) { }
  });
  knotcoindProcess.stdout.on('error', () => { });
  knotcoindProcess.stderr.on('data', (d) => {
    try { console.error(`[knotcoind] ${String(d).trimEnd()}`); } catch (_) { }
  });
  knotcoindProcess.stderr.on('error', () => { });

  knotcoindProcess.on('exit', (code, signal) => {
    try { console.log(`[knotcoind] exited code=${code} signal=${signal}`); } catch (_) { }
    knotcoindProcess = null;

    // Auto-restart if not intentionally quitting
    if (!isQuitting) {
      console.log('[main] knotcoind stopped unexpectedly, automatically restarting in 3s...');
      setTimeout(() => {
        if (!isQuitting && !knotcoindProcess) {
          try { startKnotcoind(); } catch (e) { console.error('[main] Auto-restart failed:', e); }
        }
      }, 3000);
    }
  });
}

async function waitForCookie(timeoutMs = 20000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const token = fs.readFileSync(getCookiePath(), 'utf8').trim();
      if (token && token.length >= 32) return token;
    } catch (_) {
      // ignore
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error('RPC auth cookie not found after waiting. knotcoind may have failed to start.');
}

function startRpcProxy(authToken) {
  if (proxyServer) return;

  const explorerDir = app.isPackaged
    ? path.join(process.resourcesPath, 'explorer')
    : path.join(__dirname, '..', 'share', 'explorer');

  proxyServer = http.createServer((req, res) => {
    if (req.method === 'OPTIONS') {
      res.writeHead(200, {
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': 'POST, GET, OPTIONS',
        'Access-Control-Allow-Headers': 'Content-Type'
      });
      res.end();
      return;
    }

    // Serve static files for browser preview
    if (req.method === 'GET') {
      let filePath = req.url === '/' ? '/index.html' : req.url;
      const fullPath = path.join(explorerDir, filePath);
      try {
        if (fs.existsSync(fullPath)) {
          const content = fs.readFileSync(fullPath);
          const ext = path.extname(filePath).toLowerCase();
          const mimeTypes = {
            '.html': 'text/html',
            '.css': 'text/css',
            '.js': 'application/javascript',
            '.png': 'image/png',
            '.jpg': 'image/jpeg',
            '.svg': 'image/svg+xml',
            '.ico': 'image/x-icon'
          };
          res.writeHead(200, {
            'Content-Type': mimeTypes[ext] || 'text/plain',
            'Access-Control-Allow-Origin': '*'
          });
          res.end(content);
          return;
        }
      } catch (_) { }
      res.writeHead(404, { 'Content-Type': 'text/plain' });
      res.end('Not Found');
      return;
    }

    if (req.method !== 'POST' || (req.url !== '/rpc' && req.url !== '/')) {
      res.writeHead(404, { 'Content-Type': 'application/json', 'Access-Control-Allow-Origin': '*' });
      res.end(JSON.stringify({ error: { code: -32601, message: 'Not Found' } }));
      return;
    }

    const chunks = [];
    req.on('data', (c) => chunks.push(c));
    req.on('end', () => {
      const body = Buffer.concat(chunks);

      const upstream = http.request(
        {
          host: '127.0.0.1',
          port: RPC_PORT,
          path: '/rpc',
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${authToken}`
          }
        },
        (upRes) => {
          const out = [];
          upRes.on('data', (c) => out.push(c));
          upRes.on('end', () => {
            const respBody = Buffer.concat(out);
            res.writeHead(upRes.statusCode || 500, {
              'Content-Type': 'application/json',
              'Access-Control-Allow-Origin': '*'
            });
            res.end(respBody);
          });
        }
      );

      upstream.on('error', (e) => {
        res.writeHead(502, { 'Content-Type': 'application/json', 'Access-Control-Allow-Origin': '*' });
        res.end(JSON.stringify({ error: { code: -32000, message: `Bad Gateway: ${e.message}` } }));
      });

      upstream.write(body);
      upstream.end();
    });
  });

  proxyServer.listen(PROXY_PORT, '127.0.0.1', () => {
    console.log(`[proxy] listening on http://127.0.0.1:${PROXY_PORT}`);
  });
}

function createWindow() {
  const iconPath = path.join(__dirname, 'icon.png');
  const iconExists = fs.existsSync(iconPath);
  if (process.platform === 'darwin' && iconExists) {
    try { app.dock.setIcon(nativeImage.createFromPath(iconPath)); } catch (_) { }
  }
  
  const win = new BrowserWindow({
    width: 1100,
    height: 750,
    minWidth: 800,
    minHeight: 550,
    title: 'Knotcoin',
    icon: iconExists ? iconPath : undefined,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false
    }
  });

  mainWindow = win;

  const explorerDir = app.isPackaged
    ? path.join(process.resourcesPath, 'explorer')
    : path.join(__dirname, '..', 'share', 'explorer');

  win.loadFile(path.join(explorerDir, 'index.html'));

  // On macOS hide to tray on close; on other platforms quit.
  win.on('close', (e) => {
    if (isQuitting) return;
    
    // On macOS, hide to tray by default (standard macOS behavior)
    // On Windows/Linux, quit by default (standard behavior)
    if (process.platform === 'darwin') {
      e.preventDefault();
      win.hide();
      
      // Show notification that app is still running
      if (tray) {
        // App is in tray, user can quit from there
      }
    } else {
      // Windows/Linux: actually quit
      isQuitting = true;
    }
  });

  // Cmd+Q / Ctrl+Q always fully quits
  win.webContents.on('before-input-event', (_e, input) => {
    if ((input.meta || input.control) && input.key === 'q') {
      isQuitting = true;
      app.quit();
    }
  });
  
  // Create application menu with Quit option
  if (process.platform === 'darwin') {
    const template = [
      {
        label: 'Knotcoin',
        submenu: [
          { role: 'about' },
          { type: 'separator' },
          { 
            label: 'Quit Knotcoin',
            accelerator: 'Cmd+Q',
            click: () => {
              isQuitting = true;
              app.quit();
            }
          }
        ]
      },
      {
        label: 'Edit',
        submenu: [
          { role: 'undo' },
          { role: 'redo' },
          { type: 'separator' },
          { role: 'cut' },
          { role: 'copy' },
          { role: 'paste' },
          { role: 'selectAll' }
        ]
      },
      {
        label: 'View',
        submenu: [
          { role: 'reload' },
          { role: 'forceReload' },
          { type: 'separator' },
          { role: 'resetZoom' },
          { role: 'zoomIn' },
          { role: 'zoomOut' },
          { type: 'separator' },
          { role: 'togglefullscreen' }
        ]
      },
      {
        label: 'Window',
        submenu: [
          { role: 'minimize' },
          { role: 'zoom' },
          { type: 'separator' },
          { role: 'front' }
        ]
      }
    ];
    
    const menu = Menu.buildFromTemplate(template);
    Menu.setApplicationMenu(menu);
  }
}

app.whenReady().then(async () => {
  // Create window immediately — don't make user wait for knotcoind
  createWindow();
  setupTray();
  setupAutoUpdates();

  // Check if knotcoind is already running (e.g., via launchd seed node service)
  const alreadyRunning = await isKnotcoindRunning();
  if (alreadyRunning) {
    console.log('[main] knotcoind already running, connecting to existing instance');
  } else {
    // Start backend in background; proxy starts as soon as cookie appears
    try {
      startKnotcoind();
    } catch (e) {
      console.error('[main] knotcoind start failed:', e.message);
    }
  }

  // Proxy start happens async; frontend polls RPC itself
  waitForCookie(30000)
    .then((token) => startRpcProxy(token))
    .catch((e) => console.error('[main] cookie timeout:', e.message));

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    isQuitting = true;
    app.quit();
  }
  // macOS: stay running in tray
});

app.on('before-quit', () => {
  try {
    if (proxyServer) {
      proxyServer.close();
      proxyServer = null;
    }
  } catch (_) { }

  try {
    if (knotcoindProcess) {
      knotcoindProcess.kill('SIGKILL');
      knotcoindProcess = null;
    }
  } catch (_) { }
});
