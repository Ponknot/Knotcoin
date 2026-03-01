// Knotcoin Tauri Integration — Node Control & Live Status
// Manages: node start/stop, RPC auth, status polling, log streaming, crash recovery
(function () {
    const isTauri = window.__TAURI__ !== undefined;

    if (!isTauri) {
        console.log('[Tauri] Browser mode — using direct RPC');
        return;
    }

    console.log('[Tauri] Desktop mode — full node management active');

    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;
    let rpcAuthToken = null;
    let nodeRunning = false;
    let statusPollInterval = null;
    let reconnectAttempts = 0;
    const MAX_RECONNECT = 5;

    // ── RPC Override ──────────────────────────────────────────────────────
    window.rpc = async function (method, params = []) {
        if (!rpcAuthToken) {
            try {
                rpcAuthToken = await invoke('get_rpc_auth_token');
            } catch (e) {
                // Token not available yet
            }
        }

        try {
            const headers = { 'Content-Type': 'application/json' };
            if (rpcAuthToken) {
                headers['Authorization'] = `Bearer ${rpcAuthToken}`;
            }

            const res = await fetch('http://localhost:9001/rpc', {
                method: 'POST',
                headers,
                body: JSON.stringify({ jsonrpc: '2.0', method, params, id: Date.now() }),
            });

            const payload = await res.json();
            reconnectAttempts = 0;
            updateConnectionStatus(true);
            return payload.result;
        } catch (error) {
            reconnectAttempts++;
            if (reconnectAttempts <= MAX_RECONNECT) {
                updateConnectionStatus(false);
            }
            return null;
        }
    };

    // ── UI Status Updates ─────────────────────────────────────────────────
    function updateConnectionStatus(connected) {
        const status = document.getElementById('conn-status');
        if (status) {
            status.textContent = connected ? 'ONLINE' : 'OFFLINE';
            status.className = connected ? 'status good' : 'status bad';
        }
    }

    function updateNodeButton(running) {
        const btn = document.getElementById('node-control-btn');
        if (!btn) return;
        nodeRunning = running;
        btn.disabled = false;

        if (running) {
            btn.textContent = '⏹ STOP NODE';
            btn.style.background = 'var(--bad)';
            btn.style.borderColor = 'var(--bad)';
        } else {
            btn.textContent = '▶ START NODE';
            btn.style.background = 'var(--ok)';
            btn.style.borderColor = 'var(--ok)';
        }
    }

    // ── Node Start/Stop ───────────────────────────────────────────────────
    async function startNode() {
        const btn = document.getElementById('node-control-btn');
        if (!btn) return;
        
        btn.disabled = true;
        btn.textContent = '⏳ STARTING...';
        btn.style.background = 'var(--warn)';

        try {
            console.log('[Node] Starting knotcoind...');
            const result = await invoke('start_node');
            console.log('[Node] Start result:', result);

            // Progressive timeout - check every 2 seconds for up to 20 seconds
            let attempts = 0;
            const maxAttempts = 10;
            
            while (attempts < maxAttempts) {
                await new Promise(resolve => setTimeout(resolve, 2000));
                attempts++;
                
                try {
                    // Try to get auth token
                    rpcAuthToken = await invoke('get_rpc_auth_token');
                    console.log('[Node] Auth token acquired on attempt', attempts);
                    
                    // Test RPC connection
                    const height = await window.rpc('getblockcount');
                    if (height !== null) {
                        console.log('[Node] RPC responding, height:', height);
                        updateNodeButton(true);
                        startStatusPolling();
                        
                        // Trigger immediate refresh of all data
                        setTimeout(() => {
                            if (window.refreshHome) window.refreshHome();
                            if (window.refreshBlocks) window.refreshBlocks();
                        }, 1000);
                        
                        return; // Success!
                    }
                } catch (e) {
                    console.log(`[Node] Attempt ${attempts} failed:`, e.message);
                }
                
                btn.textContent = `⏳ STARTING... (${attempts}/${maxAttempts})`;
            }
            
            throw new Error('Node started but RPC not responding after 20 seconds');
            
        } catch (error) {
            console.error('[Node] Start failed:', error);
            btn.textContent = '❌ START FAILED';
            btn.style.background = 'var(--bad)';
            setTimeout(() => updateNodeButton(false), 3000);
        }
    }

    async function stopNode() {
        const btn = document.getElementById('node-control-btn');
        if (!btn) return;
        
        btn.disabled = true;
        btn.textContent = '⏳ STOPPING...';
        btn.style.background = 'var(--warn)';

        try {
            console.log('[Node] Stopping knotcoind...');
            await invoke('stop_node');
            
            rpcAuthToken = null;
            stopStatusPolling();
            updateNodeButton(false);
            updateConnectionStatus(false);
            
            console.log('[Node] Node stopped successfully');
        } catch (error) {
            console.error('[Node] Stop failed:', error);
            btn.textContent = '❌ STOP FAILED';
            btn.style.background = 'var(--bad)';
            setTimeout(() => {
                btn.disabled = false;
                btn.textContent = '⏹ STOP NODE';
                btn.style.background = 'var(--bad)';
            }, 3000);
        }
    }

    // ── Status Polling ────────────────────────────────────────────────────
    function startStatusPolling() {
        stopStatusPolling();
        statusPollInterval = setInterval(async () => {
            const height = await window.rpc('getblockcount');
            if (height !== null) {
                // Update topbar stats
                const el = (id) => document.getElementById(id);
                const heightEl = el('demo-height') || el('dash-height');
                if (heightEl) heightEl.textContent = height;

                // Fetch miner count for peer display
                try {
                    const miners = await window.rpc('getminers');
                    if (miners && el('peer-count')) {
                        el('peer-count').textContent = Array.isArray(miners) ? miners.length : 0;
                    }
                } catch (e) { }
            } else {
                // RPC went down — node may have crashed
                const stillRunning = await invoke('is_node_running').catch(() => false);
                if (!stillRunning) {
                    console.warn('[Node] Process no longer running');
                    stopStatusPolling();
                    updateNodeButton(false);
                    updateConnectionStatus(false);
                }
            }
        }, 5000);
    }

    function stopStatusPolling() {
        if (statusPollInterval) {
            clearInterval(statusPollInterval);
            statusPollInterval = null;
        }
    }

    // ── Event Listeners ───────────────────────────────────────────────────
    async function setupEventListeners() {
        // Listen for node crash events from Tauri
        try {
            await listen('node-crashed', (event) => {
                console.error('[Node] Crashed with code:', event.payload);
                stopStatusPolling();
                updateNodeButton(false);
                updateConnectionStatus(false);
            });

            await listen('node-log', (event) => {
                // Could feed into a live log panel
                const line = event.payload;
                if (line && line.includes('[tor]')) {
                    console.log('[Tor Status]', line);
                }
            });
        } catch (e) {
            console.warn('[Tauri] Event listener setup failed:', e);
        }
    }

    // ── Initialization ────────────────────────────────────────────────────
    async function setupNodeControl() {
        const btn = document.getElementById('node-control-btn');
        if (!btn) {
            setTimeout(setupNodeControl, 200);
            return;
        }

        btn.textContent = '⏳ CHECKING...';
        btn.disabled = true;

        // Check if node is already running
        const height = await window.rpc('getblockcount');
        if (height !== null) {
            nodeRunning = true;
            updateNodeButton(true);

            try {
                rpcAuthToken = await invoke('get_rpc_auth_token');
            } catch (e) { }

            startStatusPolling();

            if (window.refreshDashboard) {
                setTimeout(() => window.refreshDashboard(), 500);
            }
        } else {
            updateNodeButton(false);
        }

        // Setup click handler
        btn.addEventListener('click', async (e) => {
            e.preventDefault();
            e.stopPropagation();
            
            if (btn.disabled) return;
            
            if (nodeRunning) {
                await stopNode();
            } else {
                await startNode();
            }
        });
    }

    // Boot
    async function boot() {
        await setupEventListeners();
        setupNodeControl();
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', boot);
    } else {
        boot();
    }
})();
