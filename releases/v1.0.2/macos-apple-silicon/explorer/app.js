/**
 * Knotcoin Core Integration
 * Real blockchain integration with proper RPC authentication
 * Based on actual knotcoind RPC methods and architecture
 */

class KnotcoinApp {
    constructor() {
        this.state = {
            connected: false,
            nodeRunning: false,
            blockHeight: 0,
            balance: 0,
            walletAddress: null,
            authToken: null,
            walletUnlocked: false,
            mining: {
                active: false,
                blocksFound: 0
            },
            network: {
                peers: 0,
                difficulty: '0',
                mempool: 0,
                poncRounds: 512
            },
            referral: {
                privacyCode: '',
                totalReferred: 0,
                totalBonus: 0
            }
        };
        
        this.refreshInterval = null;
        this.isTauri = window.__TAURI__ !== undefined;
        
        // Initialize wallet manager
        this.walletManager = new WalletManager(this.rpc.bind(this));
        
        this.init();
    }

    async init() {
        console.log('🚀 Initializing Knotcoin App...');
        console.log('📱 Mode:', this.isTauri ? 'Desktop (Tauri)' : 'Web Browser');
        
        this.setupEventListeners();
        this.loadWallet();
        
        // Try to connect if node might be running
        if (this.isTauri) {
            await this.checkNodeStatus();
        } else {
            // Web mode - try direct connection
            await this.testConnection();
        }
    }

    setupEventListeners() {
        // Navigation
        document.querySelectorAll('.nav-item').forEach(item => {
            item.addEventListener('click', () => {
                const page = item.dataset.page;
                this.showPage(page);
            });
        });

        // Node control
        const nodeBtn = document.getElementById('node-control-btn');
        if (nodeBtn) {
            nodeBtn.addEventListener('click', () => this.toggleNode());
        }

        // Logout button
        const logoutBtn = document.getElementById('logout-btn');
        if (logoutBtn) {
            logoutBtn.addEventListener('click', () => this.logout());
        }

        // Mining controls
        const miningBtn = document.getElementById('mining-btn');
        if (miningBtn) {
            miningBtn.addEventListener('click', () => this.toggleMining());
        }

        const clearLogBtn = document.getElementById('clear-log-btn');
        if (clearLogBtn) {
            clearLogBtn.addEventListener('click', () => this.clearMiningLog());
        }
    }

    showPage(pageName) {
        // Hide all pages
        document.querySelectorAll('.page').forEach(page => {
            page.classList.remove('active');
        });
        
        // Remove active from nav items
        document.querySelectorAll('.nav-item').forEach(nav => {
            nav.classList.remove('active');
        });

        // Show selected page
        const page = document.getElementById(`${pageName}-page`);
        const nav = document.querySelector(`[data-page="${pageName}"]`);
        
        if (page) page.classList.add('active');
        if (nav) nav.classList.add('active');
    }

    async checkNodeStatus() {
        if (!this.isTauri) return;

        try {
            const isRunning = await window.__TAURI__.core.invoke('is_node_running');
            if (isRunning) {
                console.log('📡 Node is already running');
                this.state.nodeRunning = true;
                await this.acquireAuthToken();
                await this.testConnection();
                this.updateNodeButton();
            }
        } catch (error) {
            console.log('📡 Node status check failed:', error.message);
        }
    }

    async acquireAuthToken() {
        if (!this.isTauri) return;

        try {
            this.state.authToken = await window.__TAURI__.core.invoke('get_rpc_auth_token');
            console.log('🔑 RPC auth token acquired');
        } catch (error) {
            console.warn('🔑 Could not get auth token:', error.message);
        }
    }

    async rpc(method, params = []) {
        const url = 'http://localhost:9001/rpc';
        const headers = { 'Content-Type': 'application/json' };
        
        // Add auth token for Tauri mode
        if (this.isTauri && this.state.authToken) {
            headers['Authorization'] = `Bearer ${this.state.authToken}`;
        }

        try {
            const response = await fetch(url, {
                method: 'POST',
                headers,
                body: JSON.stringify({
                    jsonrpc: '2.0',
                    method,
                    params,
                    id: Date.now()
                })
            });

            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            const data = await response.json();
            
            if (data.error) {
                throw new Error(`RPC Error: ${data.error.message || data.error}`);
            }

            // Update connection status on successful response
            if (!this.state.connected) {
                this.state.connected = true;
                this.updateConnectionStatus();
                console.log('✅ Connected to Knotcoin node');
                
                // Start regular refresh
                if (!this.refreshInterval) {
                    this.startDataRefresh();
                }
            }

            return data.result;
        } catch (error) {
            console.error(`❌ RPC ${method} failed:`, error.message);
            
            // Update connection status on error
            if (this.state.connected) {
                this.state.connected = false;
                this.updateConnectionStatus();
                console.log('❌ Lost connection to Knotcoin node');
            }
            
            throw error;
        }
    }

    async testConnection() {
        try {
            const height = await this.rpc('getblockcount');
            console.log('📊 Current block height:', height);
            return true;
        } catch (error) {
            console.log('📊 Connection test failed:', error.message);
            return false;
        }
    }

    async toggleNode() {
        const btn = document.getElementById('node-control-btn');
        if (!btn || !this.isTauri) return;

        if (!this.state.nodeRunning) {
            await this.startNode();
        } else {
            await this.stopNode();
        }
    }

    async startNode() {
        const btn = document.getElementById('node-control-btn');
        if (!btn) return;

        btn.disabled = true;
        btn.textContent = 'Starting...';

        try {
            console.log('🚀 Starting Knotcoin node...');
            
            // Start the node
            const result = await window.__TAURI__.core.invoke('start_node');
            console.log('📡 Node start result:', result);

            // Progressive connection attempts
            let attempts = 0;
            const maxAttempts = 15; // 30 seconds total
            
            while (attempts < maxAttempts) {
                await new Promise(resolve => setTimeout(resolve, 2000));
                attempts++;
                
                btn.textContent = `Starting... (${attempts}/${maxAttempts})`;
                
                try {
                    // Try to get auth token
                    await this.acquireAuthToken();
                    
                    // Test connection
                    if (await this.testConnection()) {
                        console.log('✅ Node is ready!');
                        this.state.nodeRunning = true;
                        this.updateNodeButton();
                        return;
                    }
                } catch (e) {
                    console.log(`⏳ Connection attempt ${attempts}/${maxAttempts}: ${e.message}`);
                }
            }
            
            throw new Error('Node started but RPC not responding after 30 seconds');
            
        } catch (error) {
            console.error('❌ Failed to start node:', error);
            btn.textContent = '❌ START FAILED';
            btn.style.background = 'var(--danger)';
            
            setTimeout(() => {
                btn.textContent = 'Start Node';
                btn.style.background = 'var(--primary)';
                btn.disabled = false;
            }, 3000);
        }
    }

    async stopNode() {
        const btn = document.getElementById('node-control-btn');
        if (!btn) return;

        btn.disabled = true;
        btn.textContent = 'Stopping...';

        try {
            await window.__TAURI__.core.invoke('stop_node');
            console.log('🛑 Node stopped');
            
            this.state.nodeRunning = false;
            this.state.connected = false;
            this.stopDataRefresh();
            this.updateNodeButton();
            this.updateConnectionStatus();
            
        } catch (error) {
            console.error('❌ Failed to stop node:', error);
        }
    }

    updateNodeButton() {
        const btn = document.getElementById('node-control-btn');
        if (!btn) return;

        btn.disabled = false;
        
        if (this.state.nodeRunning && this.state.connected) {
            btn.textContent = 'Stop Node';
            btn.style.background = 'var(--danger)';
        } else {
            btn.textContent = 'Start Node';
            btn.style.background = 'var(--primary)';
        }
    }

    updateConnectionStatus() {
        const dot = document.getElementById('connection-dot');
        const text = document.getElementById('connection-text');
        
        if (this.state.connected) {
            dot.classList.add('connected');
            text.textContent = 'Connected';
        } else {
            dot.classList.remove('connected');
            text.textContent = 'Disconnected';
        }
    }

    updateWalletStatus() {
        const dot = document.getElementById('wallet-dot');
        const text = document.getElementById('wallet-status');
        const logoutBtn = document.getElementById('logout-btn');
        
        if (this.state.walletUnlocked && this.state.walletAddress) {
            if (dot) dot.classList.add('connected');
            if (text) text.textContent = 'Unlocked';
            if (logoutBtn) logoutBtn.style.display = 'block';
        } else if (this.walletManager.hasWallet()) {
            if (dot) dot.classList.remove('connected');
            if (text) text.textContent = 'Locked';
            if (logoutBtn) logoutBtn.style.display = 'none';
        } else {
            if (dot) dot.classList.remove('connected');
            if (text) text.textContent = 'No Wallet';
            if (logoutBtn) logoutBtn.style.display = 'none';
        }
    }

    startDataRefresh() {
        if (this.refreshInterval) return;
        
        console.log('🔄 Starting data refresh...');
        this.refreshData(); // Initial refresh
        this.refreshInterval = setInterval(() => this.refreshData(), 5000);
    }

    stopDataRefresh() {
        if (this.refreshInterval) {
            clearInterval(this.refreshInterval);
            this.refreshInterval = null;
            console.log('🔄 Stopped data refresh');
        }
    }

    async refreshData() {
        if (!this.state.connected) return;

        try {
            // Get basic network info
            await Promise.all([
                this.refreshBlockHeight(),
                this.refreshMiningInfo(),
                this.refreshBalance()
            ]);
        } catch (error) {
            console.error('🔄 Data refresh failed:', error.message);
        }
    }

    async refreshBlockHeight() {
        try {
            const height = await this.rpc('getblockcount');
            this.state.blockHeight = height;
            
            const heightEl = document.getElementById('block-height');
            if (heightEl) heightEl.textContent = height;
        } catch (error) {
            console.error('📊 Failed to get block height:', error.message);
        }
    }

    async refreshMiningInfo() {
        try {
            const info = await this.rpc('getmininginfo');
            if (info) {
                this.state.network.difficulty = info.difficulty || '0';
                this.state.network.poncRounds = info.ponc_rounds || 512;
                
                // Update UI
                const diffEl = document.getElementById('difficulty');
                const poncEl = document.getElementById('ponc-rounds');
                
                if (diffEl) {
                    // Show shortened difficulty
                    const shortDiff = info.difficulty ? 
                        info.difficulty.substring(0, 8) + '...' : '0';
                    diffEl.textContent = shortDiff;
                }
                
                if (poncEl) poncEl.textContent = this.state.network.poncRounds;
            }

            // Get mempool info
            const mempoolInfo = await this.rpc('getmempoolinfo');
            if (mempoolInfo) {
                this.state.network.mempool = mempoolInfo.size || 0;
                const mempoolEl = document.getElementById('mempool-size');
                if (mempoolEl) mempoolEl.textContent = this.state.network.mempool;
            }
        } catch (error) {
            console.error('⛏️ Failed to get mining info:', error.message);
        }
    }

    async refreshBalance() {
        if (!this.state.walletAddress || !this.state.walletUnlocked) return;

        try {
            const balance = await this.rpc('getbalance', [this.state.walletAddress]);
            if (balance) {
                this.state.balance = parseFloat(balance.balance_kot) || 0;
                
                const balanceEl = document.getElementById('total-balance');
                if (balanceEl) {
                    balanceEl.textContent = this.state.balance.toFixed(8);
                }
                
                console.log('💰 Balance updated:', this.state.balance.toFixed(8), 'KOT');
            }

            // Get referral info
            const referralInfo = await this.rpc('getreferralinfo', [this.state.walletAddress]);
            if (referralInfo) {
                this.state.referral.privacyCode = referralInfo.privacy_code || '';
                this.state.referral.totalReferred = referralInfo.total_referred_miners || 0;
                this.state.referral.totalBonus = parseFloat(referralInfo.total_referral_bonus_kot) || 0;
                
                const bonusEl = document.getElementById('referral-bonus');
                if (bonusEl) {
                    bonusEl.textContent = this.state.referral.totalBonus.toFixed(8);
                }
                
                const referredEl = document.getElementById('total-referred');
                if (referredEl) {
                    referredEl.textContent = this.state.referral.totalReferred;
                }
                
                const privacyCodeEl = document.getElementById('privacy-code');
                if (privacyCodeEl) {
                    privacyCodeEl.textContent = this.state.referral.privacyCode;
                }
                
                console.log('👥 Referral info updated:', this.state.referral);
            }
        } catch (error) {
            console.error('💰 Failed to get balance:', error.message);
        }
    }

    async toggleMining() {
        const btn = document.getElementById('mining-btn');
        const addressInput = document.getElementById('mining-address');
        const blocksInput = document.getElementById('mining-blocks');
        const threadsInput = document.getElementById('mining-threads');
        
        if (!btn || !addressInput) return;

        if (!this.state.mining.active) {
            await this.startMining();
        } else {
            this.stopMining();
        }
    }

    async startMining() {
        const btn = document.getElementById('mining-btn');
        const addressInput = document.getElementById('mining-address');
        const blocksInput = document.getElementById('mining-blocks');
        const threadsInput = document.getElementById('mining-threads');
        
        // Auto-fill wallet address if not set
        let address = addressInput.value.trim();
        if (!address && this.state.walletAddress) {
            address = this.state.walletAddress;
            addressInput.value = address;
        }
        
        const blocks = parseInt(blocksInput.value) || 1;
        const threads = parseInt(threadsInput.value) || 4;

        if (!address) {
            alert('Please unlock your wallet first or enter a mining address');
            return;
        }

        this.state.mining.active = true;
        this.updateMiningStatus();
        
        btn.textContent = 'Stop Mining';
        btn.style.background = 'var(--danger)';

        this.addMiningLog(`🚀 Started mining ${blocks} blocks with ${threads} threads`);
        this.addMiningLog(`📍 Mining to address: ${address}`);

        try {
            // Use the actual generatetoaddress RPC method
            const result = await this.rpc('generatetoaddress', [blocks, address, null, threads]);
            
            if (result && result.length > 0) {
                this.state.mining.blocksFound += result.length;
                this.addMiningLog(`✅ Successfully mined ${result.length} blocks!`);
                
                result.forEach((hash, index) => {
                    this.addMiningLog(`🎯 Block ${index + 1}: ${hash.substring(0, 16)}...`);
                });
                
                // Update blocks found counter
                const blocksFoundEl = document.getElementById('blocks-found');
                if (blocksFoundEl) {
                    blocksFoundEl.textContent = this.state.mining.blocksFound;
                }
                
                // Refresh balance
                await this.refreshBalance();
            } else {
                this.addMiningLog('❌ Mining failed - no blocks generated');
            }
        } catch (error) {
            this.addMiningLog(`❌ Mining error: ${error.message}`);
        }

        this.state.mining.active = false;
        this.updateMiningStatus();
        
        btn.textContent = 'Start Mining';
        btn.style.background = 'var(--primary)';
    }

    stopMining() {
        this.state.mining.active = false;
        this.updateMiningStatus();
        this.addMiningLog('🛑 Mining stopped by user');
        
        const btn = document.getElementById('mining-btn');
        if (btn) {
            btn.textContent = 'Start Mining';
            btn.style.background = 'var(--primary)';
        }
    }

    updateMiningStatus() {
        const indicator = document.getElementById('mining-indicator');
        const statusText = document.getElementById('mining-status-text');
        
        if (indicator) {
            if (this.state.mining.active) {
                indicator.classList.add('active');
            } else {
                indicator.classList.remove('active');
            }
        }
        
        if (statusText) {
            statusText.textContent = this.state.mining.active ? 'Mining' : 'Stopped';
        }
    }

    addMiningLog(message) {
        const logEl = document.getElementById('mining-log');
        if (!logEl) return;

        const timestamp = new Date().toLocaleTimeString();
        const entry = document.createElement('div');
        entry.style.marginBottom = '4px';
        entry.style.color = 'var(--text-secondary)';
        entry.textContent = `[${timestamp}] ${message}`;
        
        logEl.insertBefore(entry, logEl.firstChild);
        
        // Keep only last 50 entries
        const entries = logEl.children;
        if (entries.length > 50) {
            for (let i = 50; i < entries.length; i++) {
                entries[i].remove();
            }
        }
    }

    clearMiningLog() {
        const logEl = document.getElementById('mining-log');
        if (logEl) {
            logEl.innerHTML = '<div>Mining log cleared...</div>';
        }
    }

    loadWallet() {
        // Check if wallet exists
        if (this.walletManager.hasWallet()) {
            const walletInfo = this.walletManager.getWalletInfo();
            console.log('💰 Wallet found:', walletInfo.address);
            
            // Show unlock screen or auto-unlock if session exists
            this.showWalletUnlockScreen();
        } else {
            console.log('💰 No wallet found - showing create/import screen');
            this.showWalletCreateScreen();
        }
    }

    showWalletUnlockScreen() {
        // TODO: Implement unlock UI
        console.log('💰 Wallet unlock screen');
    }

    showWalletCreateScreen() {
        // TODO: Implement create/import UI
        console.log('💰 Wallet create screen');
    }

    async createWallet(password) {
        try {
            const { mnemonic, address } = await this.walletManager.createWallet(password);
            this.state.walletAddress = address;
            this.state.walletUnlocked = true;
            
            // Update UI
            this.updateWalletStatus();
            
            // Set mining address
            const miningAddressInput = document.getElementById('mining-address');
            if (miningAddressInput) {
                miningAddressInput.value = address;
            }
            
            console.log('✅ Wallet created:', address);
            
            // Show mnemonic to user for backup
            const backupMsg = `🔐 SAVE YOUR MNEMONIC (24 words):\n\n${mnemonic}\n\n` +
                            `⚠️ IMPORTANT: Also backup your wallet.dat file!\n` +
                            `Location: ~/.knotcoin/mainnet/wallet.dat\n\n` +
                            `You need BOTH the mnemonic AND wallet.dat for full recovery.\n\n` +
                            `Write down the mnemonic and keep it safe!`;
            alert(backupMsg);
            
            // Start refreshing balance
            await this.refreshBalance();
            
            return { mnemonic, address };
        } catch (error) {
            console.error('❌ Failed to create wallet:', error);
            alert('Failed to create wallet: ' + error.message);
            throw error;
        }
    }

    async unlockWallet(password) {
        try {
            const { address, mnemonic_hint } = await this.walletManager.unlockWallet(password);
            this.state.walletAddress = address;
            this.state.walletUnlocked = true;
            
            // Update UI
            this.updateWalletStatus();
            
            // Set mining address
            const miningAddressInput = document.getElementById('mining-address');
            if (miningAddressInput) {
                miningAddressInput.value = address;
            }
            
            console.log('✅ Wallet unlocked:', address);
            if (mnemonic_hint) {
                console.log('💡 Mnemonic hint:', mnemonic_hint);
            }
            
            // Start refreshing balance
            await this.refreshBalance();
            
            return address;
        } catch (error) {
            console.error('❌ Failed to unlock wallet:', error);
            alert('Failed to unlock wallet: ' + error.message);
            throw error;
        }
    }

    async importWallet(mnemonic, password) {
        try {
            const { address } = await this.walletManager.importWallet(mnemonic, password);
            this.state.walletAddress = address;
            this.state.walletUnlocked = true;
            
            // Update UI
            this.updateWalletStatus();
            
            // Set mining address
            const miningAddressInput = document.getElementById('mining-address');
            if (miningAddressInput) {
                miningAddressInput.value = address;
            }
            
            console.log('✅ Wallet imported:', address);
            
            // Show important info
            alert(`✅ Wallet imported successfully!\n\n` +
                  `Address: ${address}\n\n` +
                  `⚠️ IMPORTANT: Your wallet.dat file has been created at:\n` +
                  `~/.knotcoin/mainnet/wallet.dat\n\n` +
                  `Backup both your mnemonic AND wallet.dat file!`);
            
            // Start refreshing balance
            await this.refreshBalance();
            
            return address;
        } catch (error) {
            console.error('❌ Failed to import wallet:', error);
            alert('Failed to import wallet: ' + error.message);
            throw error;
        }
    }

    logout() {
        console.log('🚪 KnotcoinApp.logout() - Locking wallet...');
        
        // Stop auto-refresh first
        if (this.walletManager) {
            this.walletManager.stopAutoRefresh();
            console.log('🔄 Stopped auto-refresh');
        }
        
        // Stop data refresh
        this.stopDataRefresh();
        
        // Clear in-memory state only (keep localStorage)
        this.state.walletAddress = null;
        this.state.walletUnlocked = false;
        this.state.balance = 0;
        this.state.referral = {
            privacyCode: '',
            totalReferred: 0,
            totalBonus: 0
        };
        console.log('🔒 Wallet locked');
        
        // Update UI
        this.updateWalletStatus();
        
        console.log('✅ Logout complete - wallet locked');
        
        // Show unlock screen
        this.showWalletUnlockScreen();
    }
}

// Initialize the app when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    window.knotcoinApp = new KnotcoinApp();
});

// Global functions for compatibility
function showPage(pageName) {
    if (window.knotcoinApp) {
        window.knotcoinApp.showPage(pageName);
    }
}