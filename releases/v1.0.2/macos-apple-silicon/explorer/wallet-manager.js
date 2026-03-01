/**
 * Knotcoin Wallet Manager
 * Fixes the wallet non-determinism issue by storing address with mnemonic
 * 
 * ISSUE: Dilithium keys are non-deterministic, so same mnemonic generates different addresses
 * SOLUTION: Store the address alongside the encrypted mnemonic in localStorage
 */

class WalletManager {
    constructor(rpcFunction) {
        this.rpc = rpcFunction;
        this.currentWallet = null;
        this.STORAGE_KEY = 'knotcoin_wallet_v1';
    }

    /**
     * Create a new wallet
     * @param {string} password - Password to encrypt the wallet
     * @returns {Promise<{mnemonic: string, address: string}>}
     */
    async createWallet(password) {
        try {
            // Generate new mnemonic via RPC
            const createResult = await this.rpc('wallet_create', []);
            const { mnemonic, address: tempAddress } = createResult;

            // Create wallet.dat file with mnemonic and password
            const result = await this.rpc('wallet_create_file', [
                mnemonic,
                password,
                '~/.knotcoin/mainnet/wallet.dat'
            ]);

            const { address, created, mnemonic_hint } = result;

            // Store wallet info in localStorage (for quick access)
            const walletInfo = {
                address: address,
                created: created,
                mnemonic_hint: mnemonic_hint,
                has_wallet_file: true
            };
            localStorage.setItem(this.STORAGE_KEY, JSON.stringify(walletInfo));
            
            this.currentWallet = { mnemonic, address };
            
            console.log('✅ Wallet created:', address);
            console.log('📁 Wallet file saved to ~/.knotcoin/mainnet/wallet.dat');
            return { mnemonic, address };
        } catch (error) {
            console.error('❌ Failed to create wallet:', error);
            throw error;
        }
    }

    /**
     * Unlock existing wallet with password
     * @param {string} password - Password to decrypt the wallet
     * @returns {Promise<{mnemonic: string, address: string}>}
     */
    async unlockWallet(password) {
        try {
            // Check if wallet file exists
            const stored = localStorage.getItem(this.STORAGE_KEY);
            if (!stored) {
                throw new Error('No wallet found. Please create or import a wallet.');
            }

            const walletInfo = JSON.parse(stored);
            
            // Unlock wallet.dat file via RPC
            const result = await this.rpc('wallet_unlock_file', [
                password,
                '~/.knotcoin/mainnet/wallet.dat'
            ]);

            const { address, mnemonic_hint } = result;

            // Verify address matches
            if (walletInfo.address !== address) {
                console.warn('⚠️ Address mismatch - updating stored address');
                walletInfo.address = address;
                localStorage.setItem(this.STORAGE_KEY, JSON.stringify(walletInfo));
            }

            // Note: We don't have the mnemonic after unlock (it's not stored)
            // User needs mnemonic only for backup/export
            this.currentWallet = {
                mnemonic: null, // Not available after unlock
                address: address
            };

            console.log('✅ Wallet unlocked:', address);
            return { address, mnemonic_hint };
        } catch (error) {
            if (error.message && error.message.includes('Invalid password')) {
                throw new Error('Incorrect password');
            }
            console.error('❌ Failed to unlock wallet:', error);
            throw error;
        }
    }

    /**
     * Import wallet from mnemonic
     * @param {string} mnemonic - 24-word mnemonic phrase
     * @param {string} password - Password to encrypt the wallet
     * @returns {Promise<{mnemonic: string, address: string}>}
     */
    async importWallet(mnemonic, password) {
        try {
            // Validate mnemonic format
            const words = mnemonic.trim().split(/\s+/);
            if (words.length !== 24) {
                throw new Error('Mnemonic must be exactly 24 words');
            }

            // Check if wallet already exists
            const existing = localStorage.getItem(this.STORAGE_KEY);
            if (existing) {
                if (!confirm('A wallet already exists. Importing will replace it. Continue?')) {
                    throw new Error('Import cancelled');
                }
            }

            // Create wallet.dat file from mnemonic
            const result = await this.rpc('wallet_create_file', [
                mnemonic,
                password,
                '~/.knotcoin/mainnet/wallet.dat'
            ]);

            const { address, created, mnemonic_hint } = result;

            // Store wallet info in localStorage
            const walletInfo = {
                address: address,
                created: created,
                mnemonic_hint: mnemonic_hint,
                has_wallet_file: true,
                imported: true
            };
            localStorage.setItem(this.STORAGE_KEY, JSON.stringify(walletInfo));
            
            this.currentWallet = { mnemonic, address };
            
            console.log('✅ Wallet imported:', address);
            console.log('📁 Wallet file saved to ~/.knotcoin/mainnet/wallet.dat');
            return { mnemonic, address };
        } catch (error) {
            console.error('❌ Failed to import wallet:', error);
            throw error;
        }
    }

    /**
     * Export wallet data (for backup)
     * @returns {{mnemonic: string, address: string, created: number, wallet_file_path: string}}
     */
    exportWallet() {
        if (!this.currentWallet) {
            throw new Error('No wallet unlocked');
        }

        const stored = localStorage.getItem(this.STORAGE_KEY);
        const walletInfo = JSON.parse(stored);

        if (!this.currentWallet.mnemonic) {
            throw new Error('Mnemonic not available. You need to have created or imported the wallet in this session to export the mnemonic.');
        }

        return {
            mnemonic: this.currentWallet.mnemonic,
            address: this.currentWallet.address,
            created: walletInfo.created,
            wallet_file_path: '~/.knotcoin/mainnet/wallet.dat',
            exported: Date.now(),
            warning: 'BACKUP BOTH: (1) This mnemonic AND (2) wallet.dat file from ~/.knotcoin/mainnet/'
        };
    }

    /**
     * Logout (clear session but keep wallet in storage)
     */
    logout() {
        this.currentWallet = null;
        console.log('✅ Logged out');
    }

    /**
     * Delete wallet permanently
     */
    deleteWallet() {
        if (confirm('Are you sure you want to delete your wallet? This cannot be undone!\n\nMake sure you have backed up your mnemonic AND wallet.dat file!')) {
            localStorage.removeItem(this.STORAGE_KEY);
            this.currentWallet = null;
            console.log('✅ Wallet deleted from localStorage');
            console.log('⚠️ Note: wallet.dat file still exists at ~/.knotcoin/mainnet/wallet.dat');
            console.log('⚠️ Delete it manually if you want to completely remove the wallet');
            return true;
        }
        return false;
    }

    /**
     * Check if wallet exists
     * @returns {boolean}
     */
    hasWallet() {
        return localStorage.getItem(this.STORAGE_KEY) !== null;
    }

    /**
     * Get current wallet info (without mnemonic)
     * @returns {{address: string, created: number, mnemonic_hint: string} | null}
     */
    getWalletInfo() {
        const stored = localStorage.getItem(this.STORAGE_KEY);
        if (!stored) return null;

        const walletInfo = JSON.parse(stored);
        return {
            address: walletInfo.address,
            created: walletInfo.created,
            imported: walletInfo.imported || false,
            mnemonic_hint: walletInfo.mnemonic_hint || null,
            has_wallet_file: walletInfo.has_wallet_file || false
        };
    }

    /**
     * Check if wallet is unlocked
     * @returns {boolean}
     */
    isUnlocked() {
        return this.currentWallet !== null;
    }

    /**
     * Get current address
     * @returns {string | null}
     */
    getCurrentAddress() {
        return this.currentWallet?.address || null;
    }

    /**
     * Get current mnemonic (only when unlocked during create/import)
     * @returns {string | null}
     */
    getCurrentMnemonic() {
        return this.currentWallet?.mnemonic || null;
    }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = WalletManager;
}

    /**
     * Update balance and referral info from blockchain
     * @returns {Promise<void>}
     */
    async updateBalance() {
        if (!this.currentWallet) {
            console.log('[Wallet] No wallet loaded, skipping balance update');
            return;
        }

        try {
            const address = this.currentWallet.address;
            console.log('[Wallet] Fetching balance for:', address);
            
            const balanceData = await this.rpc('getbalance', [address]);
            console.log('[Wallet] Balance data:', balanceData);
            
            if (balanceData) {
                this.currentWallet.balance = parseFloat(balanceData.balance_kot || '0');
                this.currentWallet.nonce = balanceData.nonce || 0;
                
                // Update UI
                const balanceEl = document.getElementById('wallet-balance');
                if (balanceEl) {
                    balanceEl.textContent = this.currentWallet.balance.toFixed(8);
                }
                
                const addressEl = document.getElementById('wallet-address');
                if (addressEl) {
                    addressEl.textContent = address;
                }
            }
            
            // Fetch referral info
            const referralData = await this.rpc('getreferralinfo', [address]);
            console.log('[Wallet] Referral data:', referralData);
            
            if (referralData) {
                this.currentWallet.privacyCode = referralData.privacy_code;
                this.currentWallet.totalReferredMiners = referralData.total_referred_miners || 0;
                this.currentWallet.totalReferralBonus = parseFloat(referralData.total_referral_bonus_kot || '0');
                
                // Update referral UI
                const privacyCodeEl = document.getElementById('privacy-code');
                if (privacyCodeEl) {
                    privacyCodeEl.textContent = referralData.privacy_code;
                }
                
                const referralLinkEl = document.getElementById('referral-link');
                if (referralLinkEl) {
                    referralLinkEl.value = `knotcoin:?ref=${referralData.privacy_code}`;
                }
                
                const referredCountEl = document.getElementById('referred-count');
                if (referredCountEl) {
                    referredCountEl.textContent = referralData.total_referred_miners || 0;
                }
                
                const referralBonusEl = document.getElementById('referral-bonus');
                if (referralBonusEl) {
                    referralBonusEl.textContent = (parseFloat(referralData.total_referral_bonus_kot || '0')).toFixed(8);
                }
            }
        } catch (error) {
            console.error('[Wallet] Error updating balance:', error);
        }
    }

    /**
     * Start auto-refresh of balance (every 5 seconds)
     */
    startAutoRefresh() {
        if (this.refreshInterval) {
            clearInterval(this.refreshInterval);
        }
        
        this.refreshInterval = setInterval(() => {
            if (this.isUnlocked()) {
                this.updateBalance();
            }
        }, 5000);
        
        // Initial update
        if (this.isUnlocked()) {
            this.updateBalance();
        }
    }

    /**
     * Stop auto-refresh
     */
    stopAutoRefresh() {
        if (this.refreshInterval) {
            clearInterval(this.refreshInterval);
            this.refreshInterval = null;
        }
    }
}
