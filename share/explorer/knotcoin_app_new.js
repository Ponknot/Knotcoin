/**
 * Knotcoin Clean Frontend
 * Simple, working, real-time blockchain sync
 */

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
        peers: 0,
        mempool: 0
    },
    mining: {
        active: false,
        blocks: 0
    },
    referral: {
        code: '',
        total: 0,
        earnings: 0
    },
    syncTimer: null
};

// Helper functions
function $(id) {
    return document.getElementById(id);
}

function knots2kot(knots) {
    return (knots / KNOTS_PER_KOT).toFixed(8);
}

function shortAddr(addr) {
    if (!addr || addr.length < 16) return addr || '';
    return addr.slice(0, 10) + '...' + addr.slice(-6);
}

function toast(msg) {
    const container = $('toast-container');
    const toast = document.createElement('div');
    toast.className = 'toast';
    toast.textContent = msg;
    container.appendChild(toast);
    setTimeout(() => toast.remove(), 3000);
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
        updateStatus();
        return data.result;
    } catch (error) {
        state.connected = false;
        updateStatus();
        throw error;
    }
}

// Update status bar
function updateStatus() {
    const dot = $('status-dot');
    const text = $('status-text');
    const time = $('sync-time');
    
    if (state.connected) {
        dot.classList.add('connected');
        text.textContent = `Block ${state.blockchain.height} • ${state.blockchain.peers} peers`;
    } else {
        dot.classList.remove('connected');
        text.textContent = 'Disconnected';
    }
    
    const now = new Date();
    time.textContent = now.toLocaleTimeString();
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

// Real-time sync with blockchain
async function syncBlockchain() {
    if (!state.connected) return;
    
    try {
        // Get blockchain info
        const miningInfo = await rpc('getmininginfo');
        state.blockchain.height = miningInfo.blocks || 0;
        state.blockchain.mempool = miningInfo.mempool || 0;
        
        // Get peer info
        const peerInfo = await rpc('getpeerinfo');
        state.blockchain.peers = peerInfo.peer_count || 0;
        
        // Get balance if wallet exists
        if (state.wallet.address) {
            const balanceInfo = await rpc('getbalance', [state.wallet.address]);
            state.wallet.balance = balanceInfo.balance_knots || 0;
            
            // Get referral info
            try {
                const referralInfo = await rpc('getreferralinfo', [state.wallet.address]);
                state.referral.code = referralInfo.privacy_code || '';
                state.referral.total = referralInfo.total_referred_miners || 0;
                state.referral.earnings = parseFloat(referralInfo.total_referral_bonus_kot) || 0;
            } catch (e) {
                console.warn('Could not get referral info:', e.message);
            }
        }
        
        // Update UI
        updateUI();
        
        console.log(`🔄 Synced: Block ${state.blockchain.height}, Balance ${knots2kot(state.wallet.balance)} KOT`);
        
    } catch (error) {
        console.error('❌ Sync error:', error.message);
        state.connected = false;
    }
    
    updateStatus();
}

// Update UI with current state
function updateUI() {
    // Balance
    if ($('balance-amount')) {
        $('balance-amount').textContent = knots2kot(state.wallet.balance);
    }
    
    // Address
    if ($('wallet-address')) {
        $('wallet-address').textContent = state.wallet.address || 'Loading...';
    }
    
    // Blockchain stats
    if ($('block-height')) $('block-height').textContent = state.blockchain.height;
    if ($('peer-count')) $('peer-count').textContent = state.blockchain.peers;
    if ($('mempool-size')) $('mempool-size').textContent = state.blockchain.mempool;
    
    // Referral
    if ($('referral-link')) {
        $('referral-link').textContent = state.referral.code ? 
            `knotcoin:?ref=${state.referral.code}` : 'Loading...';
    }
    if ($('total-referred')) $('total-referred').textContent = state.referral.total;
    if ($('referral-earnings')) $('referral-earnings').textContent = state.referral.earnings.toFixed(8) + ' KOT';
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
    $('onboarding').classList.remove('hidden');
    $('main-app').classList.add('hidden');
    $('create-wallet').classList.add('hidden');
    $('import-wallet').classList.add('hidden');
    stopSync();
}

function showApp() {
    $('onboarding').classList.add('hidden');
    $('main-app').classList.remove('hidden');
    updateUI();
    startSync();
}

// Onboarding functions
window.showCreateWallet = async function() {
    try {
        $('create-wallet').classList.remove('hidden');
        $('onboarding').querySelector('.card').classList.add('hidden');
        
        // Generate wallet
        const result = await rpc('wallet_create');
        $('mnemonic-display').textContent = result.mnemonic;
        
        // Store temporarily
        window.tempWallet = result;
    } catch (error) {
        toast('Failed to create wallet: ' + error.message);
        showOnboarding();
    }
};

window.showImportWallet = function() {
    $('import-wallet').classList.remove('hidden');
    $('onboarding').querySelector('.card').classList.add('hidden');
};

window.confirmCreate = function() {
    if (!window.tempWallet) {
        toast('No wallet to confirm');
        return;
    }
    
    saveWallet(window.tempWallet.mnemonic, window.tempWallet.address);
    
    const referral = $('referral-input').value.trim();
    if (referral) {
        localStorage.setItem('knotcoin_referrer', referral);
    }
    
    toast('Wallet created: ' + shortAddr(window.tempWallet.address));
    delete window.tempWallet;
    showApp();
};

window.confirmImport = async function() {
    const mnemonic = $('import-mnemonic').value.trim();
    if (!mnemonic) {
        toast('Please enter your mnemonic');
        return;
    }
    
    try {
        const result = await rpc('wallet_get_address', [mnemonic]);
        saveWallet(mnemonic, result.address);
        toast('Wallet imported: ' + shortAddr(result.address));
        showApp();
    } catch (error) {
        toast('Failed to import wallet: ' + error.message);
    }
};

// Tab navigation
window.showTab = function(tabName) {
    // Update tab buttons
    document.querySelectorAll('.tab').forEach(tab => {
        tab.classList.remove('active');
        if (tab.textContent.toLowerCase().includes(tabName)) {
            tab.classList.add('active');
        }
    });
    
    // Update tab content
    document.querySelectorAll('.tab-content').forEach(content => {
        content.classList.remove('active');
    });
    $('tab-' + tabName).classList.add('active');
};

// Mining
window.toggleMining = async function() {
    const btn = $('mining-btn');
    
    if (state.mining.active) {
        // Stop mining
        try {
            await rpc('stop_mining');
            state.mining.active = false;
            btn.textContent = 'Start Mining';
            btn.className = 'btn btn-primary btn-block';
            $('mining-status').textContent = 'Stopped';
            toast('Mining stopped');
        } catch (error) {
            toast('Failed to stop mining: ' + error.message);
        }
    } else {
        // Start mining
        try {
            const mnemonic = state.wallet.mnemonic;
            const referrer = localStorage.getItem('knotcoin_referrer');
            const args = referrer ? [mnemonic, 2, referrer] : [mnemonic, 2];
            
            await rpc('start_mining', args);
            state.mining.active = true;
            btn.textContent = 'Stop Mining';
            btn.className = 'btn btn-danger btn-block';
            $('mining-status').textContent = 'Active';
            toast('Mining started');
        } catch (error) {
            toast('Failed to start mining: ' + error.message);
        }
    }
};

// Referral
window.copyReferral = function() {
    const link = $('referral-link').textContent;
    if (link && link !== 'Loading...') {
        navigator.clipboard.writeText(link).then(() => {
            toast('Referral link copied!');
        });
    }
};

// Settings
window.revealMnemonic = function() {
    const box = $('mnemonic-reveal');
    const words = $('mnemonic-words');
    
    if (box.classList.contains('hidden')) {
        words.textContent = state.wallet.mnemonic || 'No mnemonic found';
        box.classList.remove('hidden');
    } else {
        box.classList.add('hidden');
    }
};

window.deleteWallet = function() {
    console.log('🗑️ deleteWallet called');
    
    if (!confirm('⚠️ DELETE WALLET?\n\nThis will permanently delete your wallet data.\n\nMake sure you have backed up your 24-word mnemonic!\n\nAre you absolutely sure?')) {
        console.log('❌ Cancelled');
        return;
    }
    
    if (!confirm('FINAL WARNING!\n\nThis action cannot be undone.\n\nClick OK to delete permanently.')) {
        console.log('❌ Cancelled at final warning');
        return;
    }
    
    console.log('🗑️ Deleting wallet...');
    
    // Stop sync
    stopSync();
    
    // Clear localStorage
    localStorage.clear();
    console.log('✅ localStorage cleared');
    
    // Clear state
    state.wallet = { mnemonic: null, address: null, balance: 0 };
    
    toast('Wallet deleted');
    
    // Reload page
    setTimeout(() => {
        console.log('🔄 Reloading...');
        window.location.reload();
    }, 1000);
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

// Expose for debugging
window.knotcoinState = state;
window.knotcoinRPC = rpc;
