/**
 * Knotcoin Simple & Robust UI
 * Real-time blockchain sync, no glitches
 */

(function() {
    'use strict';
    
    // Configuration
    const RPC_URL = 'http://127.0.0.1:9001';
    const SYNC_INTERVAL = 2000; // 2 seconds
    const KNOTS_PER_KOT = 100000000;
    
    // Global state
    let state = {
        authToken: '',
        connected: false,
        wallet: {
            mnemonic: null,
            address: null,
            balance: 0
        },
        blockchain: {
            height: 0,
            difficulty: '',
            peers: 0,
            mempool: 0
        },
        mining: {
            active: false,
            blocks: 0
        },
        syncTimer: null
    };
    
    // Helper functions
    function $(id) { return document.getElementById(id); }
    
    function knots2kot(knots) {
        return (knots / KNOTS_PER_KOT).toFixed(8);
    }
    
    function shortAddr(addr) {
        if (!addr || addr.length < 16) return addr || '';
        return addr.slice(0, 10) + '...' + addr.slice(-6);
    }
    
    function toast(msg, type = 'success') {
        const t = document.createElement('div');
        t.className = 'toast ' + type;
        t.textContent = msg;
        $('toasts').appendChild(t);
        setTimeout(() => t.remove(), 3000);
    }
    
    // RPC call
    async function rpc(method, params = []) {
        const headers = { 'Content-Type': 'application/json' };
        if (state.authToken) {
            headers['Authorization'] = 'Bearer ' + state.authToken;
        }
        
        try {
            const response = await fetch(RPC_URL, {
                method: 'POST',
                headers,
                body: JSON.stringify({
                    jsonrpc: '2.0',
                    method,
                    params,
                    id: Date.now()
                })
            });
            
            const data = await response.json();
            if (data.error) throw new Error(data.error.message || JSON.stringify(data.error));
            
            state.connected = true;
            return data.result;
        } catch (error) {
            state.connected = false;
            throw error;
        }
    }
    
    // Load auth token
    async function loadAuth() {
        if (window.__TAURI__) {
            try {
                const invoke = window.__TAURI__.core?.invoke || window.__TAURI__.invoke;
                if (invoke) {
                    state.authToken = await invoke('get_rpc_auth_token');
                    console.log('✅ Auth token loaded');
                }
            } catch (e) {
                console.warn('⚠️ Could not load auth token:', e.message);
            }
        }
    }
    
    // Check if wallet exists
    function hasWallet() {
        return !!localStorage.getItem('knotcoin_address');
    }
    
    // Load wallet from localStorage
    function loadWallet() {
        state.wallet.mnemonic = localStorage.getItem('knotcoin_mnemonic');
        state.wallet.address = localStorage.getItem('knotcoin_address');
        return state.wallet.address !== null;
    }
    
    // Save wallet to localStorage
    function saveWallet(mnemonic, address) {
        localStorage.setItem('knotcoin_mnemonic', mnemonic);
        localStorage.setItem('knotcoin_address', address);
        state.wallet.mnemonic = mnemonic;
        state.wallet.address = address;
    }
    
    // Delete wallet completely
    function deleteWallet() {
        console.log('🗑️ Deleting wallet...');
        
        // Stop sync
        if (state.syncTimer) {
            clearInterval(state.syncTimer);
            state.syncTimer = null;
        }
        
        // Clear localStorage
        localStorage.clear();
        
        // Clear state
        state.wallet = { mnemonic: null, address: null, balance: 0 };
        
        console.log('✅ Wallet deleted');
        
        // Reload page
        window.location.reload();
    }
    
    // Real-time sync with blockchain
    async function syncBlockchain() {
        if (!state.connected) return;
        
        try {
            // Get blockchain info
            const miningInfo = await rpc('getmininginfo');
            state.blockchain.height = miningInfo.blocks || 0;
            state.blockchain.difficulty = miningInfo.difficulty || '';
            state.blockchain.mempool = miningInfo.mempool || 0;
            
            // Update UI
            if ($('stat-height')) $('stat-height').textContent = state.blockchain.height;
            if ($('settings-height')) $('settings-height').textContent = state.blockchain.height;
            
            // Update node status
            const now = new Date();
            const timeStr = now.toLocaleTimeString();
            if ($('node-label')) {
                $('node-label').textContent = `Block ${state.blockchain.height} • ${timeStr}`;
            }
            if ($('node-dot')) {
                $('node-dot').className = 'status-dot connected';
            }
            
            // Get peer info
            const peerInfo = await rpc('getpeerinfo');
            state.blockchain.peers = peerInfo.peer_count || 0;
            if ($('stat-peers')) $('stat-peers').textContent = state.blockchain.peers;
            
            // Get balance if wallet exists
            if (state.wallet.address) {
                const balanceInfo = await rpc('getbalance', [state.wallet.address]);
                state.wallet.balance = balanceInfo.balance_knots || 0;
                
                // Update balance display
                if ($('wallet-balance')) {
                    $('wallet-balance').innerHTML = knots2kot(state.wallet.balance) + '<span class="balance-unit">KOT</span>';
                }
            }
            
            console.log(`🔄 Synced: Block ${state.blockchain.height}, Balance ${knots2kot(state.wallet.balance)} KOT`);
            
        } catch (error) {
            console.error('❌ Sync error:', error.message);
            state.connected = false;
            if ($('node-dot')) $('node-dot').className = 'status-dot';
            if ($('node-label')) $('node-label').textContent = 'Disconnected';
        }
    }
    
    // Start real-time sync
    function startSync() {
        if (state.syncTimer) return;
        
        console.log('🔄 Starting real-time sync...');
        syncBlockchain(); // Immediate sync
        state.syncTimer = setInterval(syncBlockchain, SYNC_INTERVAL);
    }
    
    // Stop sync
    function stopSync() {
        if (state.syncTimer) {
            clearInterval(state.syncTimer);
            state.syncTimer = null;
            console.log('🔄 Stopped sync');
        }
    }
    
    // Show/hide screens
    function showOnboarding() {
        if ($('onboarding')) $('onboarding').classList.remove('hidden');
        if ($('app')) $('app').classList.add('hidden');
        stopSync();
    }
    
    function showApp() {
        if ($('onboarding')) $('onboarding').classList.add('hidden');
        if ($('app')) $('app').classList.remove('hidden');
        startSync();
    }
    
    // Create wallet
    window.createWallet = async function() {
        try {
            const result = await rpc('wallet_create');
            saveWallet(result.mnemonic, result.address);
            
            // Show mnemonic
            if ($('mnemonic-words')) {
                $('mnemonic-words').textContent = result.mnemonic;
            }
            
            toast('Wallet created: ' + shortAddr(result.address));
            return result;
        } catch (error) {
            toast('Failed to create wallet: ' + error.message, 'error');
            throw error;
        }
    };
    
    // Import wallet
    window.importWallet = async function(mnemonic) {
        try {
            const result = await rpc('wallet_get_address', [mnemonic]);
            saveWallet(mnemonic, result.address);
            toast('Wallet imported: ' + shortAddr(result.address));
            showApp();
        } catch (error) {
            toast('Failed to import wallet: ' + error.message, 'error');
            throw error;
        }
    };
    
    // Delete wallet (exposed globally)
    window.deleteWalletNow = function() {
        if (!confirm('⚠️ DELETE WALLET?\n\nThis will permanently delete your wallet data.\n\nMake sure you have backed up your mnemonic!')) {
            return;
        }
        
        if (!confirm('FINAL WARNING!\n\nThis cannot be undone.\n\nClick OK to delete.')) {
            return;
        }
        
        deleteWallet();
    };
    
    // Initialize app
    async function init() {
        console.log('🚀 Initializing Knotcoin...');
        
        // Load auth
        await loadAuth();
        
        // Wait for node
        let attempts = 0;
        while (attempts < 30) {
            try {
                await rpc('getblockcount');
                console.log('✅ Node connected');
                break;
            } catch (e) {
                attempts++;
                if (attempts < 30) {
                    await new Promise(resolve => setTimeout(resolve, 1000));
                }
            }
        }
        
        // Check if wallet exists
        if (loadWallet()) {
            console.log('✅ Wallet loaded:', shortAddr(state.wallet.address));
            showApp();
        } else {
            console.log('ℹ️ No wallet found');
            showOnboarding();
        }
    }
    
    // Start when DOM is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
    
    // Expose state for debugging
    window.knotcoinState = state;
    window.knotcoinRPC = rpc;
    
})();
