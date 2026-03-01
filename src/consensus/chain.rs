use crate::primitives::transaction::KNOTS_PER_KOT;
use primitive_types::U256;

const PHASE_1_END: u64 = 262_800;
const PHASE_2_END: u64 = 525_600;
const RETARGET_WINDOW: u64 = 60;
const RETARGET_SECS: u64 = RETARGET_WINDOW * 60;
pub const REFERRAL_WINDOW: u64 = 2_880;
const REFERRAL_BONUS_PCT: u64 = 5;

// Governance parameters
pub const GOVERNANCE_BASE_BPS: u64 = 100; // 1% base
pub const GOVERNANCE_BPS_SCALE: u64 = 10_000; // 100% = 10000 bps

// Governance cap range (tunable via governance vote)
pub const GOVERNANCE_CAP_MIN_BPS: u64 = 500;   // 5% minimum
pub const GOVERNANCE_CAP_MAX_BPS: u64 = 2000;  // 20% maximum  
pub const GOVERNANCE_CAP_DEFAULT_BPS: u64 = 1000; // 10% default (genesis)

// PONC round count range (tunable via governance vote)
pub const PONC_ROUNDS_MIN: u64 = 256;
pub const PONC_ROUNDS_MAX: u64 = 2048;
pub const PONC_ROUNDS_DEFAULT: u64 = 512;

// Mining thread count range (tunable via governance vote)
// FAIRNESS: Start at 4 threads to level playing field
// Community can vote to increase if network matures
pub const MINING_THREADS_MIN: u64 = 1;
pub const MINING_THREADS_MAX: u64 = 8;   // Hard cap for fairness
pub const MINING_THREADS_DEFAULT: u64 = 4;  // Fair for laptops

// Phase 1: linear ramp from 0.1 KOT to 1.0 KOT over 262,800 blocks.
// Formula: reward = 0.1 + (0.9 * height / 262,800) KOT
// In knots: 10M + (90M * height / 262,800)
fn phase1_reward(height: u64) -> u64 {
    let start_knots = 10_000_000;
    let delta_knots = 90_000_000;
    start_knots + (delta_knots * height / PHASE_1_END)
}

// Actually, let's use a simpler fixed-point log2.
// log2(x) ~= ilog2(x) + (x - 2^ilog2) / 2^ilog2
// This is linear interpolation, but we want more precision.

pub fn calculate_governance_weight(total_contributions: u64) -> u64 {
    if total_contributions == 0 { return 100; }
    // Approximation: 100 + 100 * log10(n)
    // SECURITY FIX: Use ilog10() instead of to_string().len() to avoid heap allocation DoS
    // This prevents memory exhaustion attacks during governance tally validation
    let digits = total_contributions.ilog10() as u64 + 1;
    100 + 100 * (digits - 1)
}

fn phase3_reward(height: u64) -> u64 {
    let adjusted = height - (PHASE_2_END + 1);
    let x = adjusted + 2;
    if x == 2 { return KNOTS_PER_KOT; } // Exact match for continuity

    let ilog = x.ilog2();
    let mut val = (ilog as u64) << 16;

    // SECURITY FIX: Use saturating_sub to prevent underflow panic
    // While practically impossible (requires 4.6 quintillion blocks),
    // this ensures formal safety against edge cases and testnet manipulation
    let shift_amount = 62u32.saturating_sub(ilog);
    let mut f = x << shift_amount;

    for i in (0..16).rev() {
        let f128 = f as u128;
        f = ((f128 * f128) >> 62) as u64;
        if f >= (1u64 << 63) {
            val |= 1 << i;
            f >>= 1;
        }
    }

    (KNOTS_PER_KOT << 16) / val
}

pub fn calculate_block_reward(height: u64) -> u64 {
    if height <= PHASE_1_END {
        phase1_reward(height)
    } else if height <= PHASE_2_END {
        KNOTS_PER_KOT // 1.0 KOT
    } else {
        phase3_reward(height)
    }
}

// Referrer gets 5% of the miner's base reward, but only if they mined
// within the last 2,880 blocks (~48 hours). Bonus is protocol-minted,
// not deducted from the miner.
pub fn calculate_referral_bonus(
    base_reward: u64,
    referrer_total_mined: u64,
    referrer_last_mined: u64,
    current_height: u64,
) -> u64 {
    // Referrer must be an active miner (mined at least one block)
    if referrer_total_mined == 0 {
        return 0;
    }

    // Referrer must be RECENTLY active (mined a block in the last 2880 blocks)
    let activity_window = 2880;
    let too_old = current_height > referrer_last_mined + activity_window && current_height > 0;
    
    if too_old {
        return 0;
    }

    // 5% bonus
    (base_reward * REFERRAL_BONUS_PCT) / 100
}

// Governance weight for an address, in basis points (10000 = 100%).
// Base: 1% (100 bps). Boost: +5% per active downstream referral.
// Hard cap: 10% (1000 bps) regardless of referral count. This prevents


pub fn calculate_new_difficulty(old_target: &[u8; 32], actual_secs: u64) -> [u8; 32] {
    // Clamp to 4x adjustment window to resist timestamp manipulation.
    let clamped = actual_secs.clamp(RETARGET_SECS / 4, RETARGET_SECS * 4);

    let old = U256::from_big_endian(old_target);
    let actual = U256::from(clamped);
    let expected = U256::from(RETARGET_SECS);

    let new = if U256::MAX / actual < old {
        U256::MAX
    } else {
        (old * actual / expected).max(U256::one())
    };

    let mut out = [0u8; 32];
    let words = new.0;
    for i in 0..4 {
        out[i * 8..(i + 1) * 8].copy_from_slice(&words[3 - i].to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== PHASE 1 TESTS ==========
    #[test]
    fn test_phase1() {
        assert_eq!(calculate_block_reward(0), 10_000_000); // 0.1 KOT
        assert_eq!(calculate_block_reward(PHASE_1_END), 100_000_000); // 1.0 KOT
    }

    #[test]
    fn test_phase1_midpoint() {
        let mid = PHASE_1_END / 2;
        let reward = calculate_block_reward(mid);
        // At midpoint, should be ~0.55 KOT
        assert!(reward > 50_000_000 && reward < 60_000_000);
    }

    #[test]
    fn test_phase1_quarter_points() {
        let q1 = PHASE_1_END / 4;
        let q3 = (PHASE_1_END * 3) / 4;
        assert!(calculate_block_reward(q1) < calculate_block_reward(q3));
    }

    #[test]
    fn test_phase1_monotonic_increase() {
        for i in 0..100 {
            let h1 = (PHASE_1_END * i) / 100;
            let h2 = (PHASE_1_END * (i + 1)) / 100;
            assert!(calculate_block_reward(h1) <= calculate_block_reward(h2));
        }
    }

    // ========== PHASE 2 TESTS ==========
    #[test]
    fn test_phase2() {
        assert_eq!(calculate_block_reward(PHASE_1_END + 1), 100_000_000);
        assert_eq!(calculate_block_reward(PHASE_2_END), 100_000_000);
    }

    #[test]
    fn test_phase2_constant() {
        for i in 0..100 {
            let h = PHASE_1_END + 1 + (i * 1000);
            if h <= PHASE_2_END {
                assert_eq!(calculate_block_reward(h), 100_000_000);
            }
        }
    }

    // ========== PHASE 3 TESTS ==========
    #[test]
    fn test_phase3_continuity() {
        let r = calculate_block_reward(PHASE_2_END + 1);
        assert_eq!(r, 100_000_000); // 1.0 KOT exactly
    }

    #[test]
    fn test_phase3_decay() {
        let r1 = calculate_block_reward(PHASE_2_END + 1);
        let r2 = calculate_block_reward(PHASE_2_END + 100_000);
        let r3 = calculate_block_reward(PHASE_2_END + 1_000_000);
        assert!(r1 > r2);
        assert!(r2 > r3);
    }

    #[test]
    fn test_phase3_never_zero() {
        // Even at very high block heights, reward should never be zero
        let r = calculate_block_reward(PHASE_2_END + 100_000_000);
        assert!(r > 0);
    }

    #[test]
    fn test_phase3_long_term() {
        // Test rewards at various future points
        let year_2 = PHASE_2_END + 525_600;
        let year_10 = PHASE_2_END + 5_256_000;
        let year_50 = PHASE_2_END + 26_280_000;
        
        let r2 = calculate_block_reward(year_2);
        let r10 = calculate_block_reward(year_10);
        let r50 = calculate_block_reward(year_50);
        
        assert!(r2 > r10);
        assert!(r10 > r50);
        assert!(r50 > 0);
    }

    // ========== REFERRAL BONUS TESTS ==========
    #[test]
    fn test_referral_bonus() {
        // total_mined=100, last_mined=1000, current=2000 => OK
        assert_eq!(calculate_referral_bonus(100_000_000, 100, 1000, 2000), 5_000_000);
        // last_mined=1000, current=5000 => Too old
        assert_eq!(calculate_referral_bonus(100_000_000, 100, 1000, 5000), 0);
    }

    #[test]
    fn test_referral_bonus_no_threshold() {
        assert_eq!(calculate_referral_bonus(1_000, 10, 1000, 2000), 50);
        assert_eq!(calculate_referral_bonus(100, 10, 1000, 2000), 5);
        assert_eq!(calculate_referral_bonus(10, 10, 1000, 2000), 0);
    }

    #[test]
    fn test_referral_bonus_genesis_miner() {
        // Genesis miner (total=1, last=0) IS active for first 2880 blocks
        assert_eq!(calculate_referral_bonus(100_000_000, 1, 0, 1000), 5_000_000);
        // Non-miner (total=0) get no bonus
        assert_eq!(calculate_referral_bonus(100_000_000, 0, 0, 1000), 0);
    }

    #[test]
    fn test_referral_bonus_window_boundary() {
        let base = 100_000_000;
        let referrer_height = 1000;
        
        // Just inside window
        assert_eq!(calculate_referral_bonus(base, 1, referrer_height, referrer_height + REFERRAL_WINDOW), 5_000_000);
        
        // Just outside window
        assert_eq!(calculate_referral_bonus(base, 1, referrer_height, referrer_height + REFERRAL_WINDOW + 1), 0);
    }

    #[test]
    fn test_referral_bonus_percentage() {
        // Verify 5% calculation is exact
        for reward in [1_000_000, 10_000_000, 100_000_000, 1_000_000_000] {
            let bonus = calculate_referral_bonus(reward, 1, 100, 200);
            assert_eq!(bonus, reward / 20); // 5% = 1/20
        }
    }

    // ========== GOVERNANCE WEIGHT TESTS ==========
    #[test]
    fn test_governance_weight() {
        assert_eq!(calculate_governance_weight(0), 100); 
        assert_eq!(calculate_governance_weight(10), 200); 
        assert_eq!(calculate_governance_weight(100), 300); 
    }

    #[test]
    fn test_governance_weight_scaling() {
        assert_eq!(calculate_governance_weight(1), 100);
        assert_eq!(calculate_governance_weight(9), 100);
        assert_eq!(calculate_governance_weight(10), 200);
        assert_eq!(calculate_governance_weight(99), 200);
        assert_eq!(calculate_governance_weight(100), 300);
        assert_eq!(calculate_governance_weight(999), 300);
        assert_eq!(calculate_governance_weight(1000), 400);
    }

    #[test]
    fn test_governance_weight_large_numbers() {
        assert_eq!(calculate_governance_weight(1_000_000), 700);
        assert_eq!(calculate_governance_weight(1_000_000_000), 1000);
    }

    // ========== DIFFICULTY RETARGET TESTS ==========
    #[test]
    fn test_difficulty_retarget() {
        let mut target = [0u8; 32];
        target[31] = 100;
        assert_eq!(calculate_new_difficulty(&target, 3600)[31], 100);
        assert_eq!(calculate_new_difficulty(&target, 1800)[31], 50);
        assert_eq!(calculate_new_difficulty(&target, 7200)[31], 200);
        // Clamp floor: 10s → treated as 900s → 100 * 900 / 3600 = 25
        assert_eq!(calculate_new_difficulty(&target, 10)[31], 25);
    }

    #[test]
    fn test_difficulty_clamp_ceiling() {
        let mut target = [0u8; 32];
        target[31] = 100;
        // 20000s should be clamped to 14400s (4x max)
        // 100 * 14400 / 3600 = 400
        let result = calculate_new_difficulty(&target, 20000);
        assert_eq!(result[31], 144); // Clamped to 4x
    }

    #[test]
    fn test_difficulty_never_zero() {
        let mut target = [0u8; 32];
        target[31] = 1;
        let result = calculate_new_difficulty(&target, 1);
        // Should never produce zero difficulty
        assert!(result.iter().any(|&b| b > 0));
    }

    #[test]
    fn test_difficulty_symmetry() {
        let mut target = [0u8; 32];
        target[31] = 100;
        
        // Double time should halve difficulty (double target)
        let doubled = calculate_new_difficulty(&target, 7200);
        // Half time should double difficulty (halve target)
        let halved = calculate_new_difficulty(&target, 1800);
        
        assert!(doubled[31] > target[31]);
        assert!(halved[31] < target[31]);
    }

    // ========== CONSTANTS TESTS ==========
    #[test]
    fn test_phase_boundaries() {
        assert_eq!(PHASE_1_END, 262_800);
        assert_eq!(PHASE_2_END, 525_600);
        assert!(PHASE_2_END > PHASE_1_END);
    }

    #[test]
    fn test_referral_constants() {
        assert_eq!(REFERRAL_BONUS_PCT, 5);
        assert_eq!(REFERRAL_WINDOW, 2_880);
    }

    #[test]
    fn test_governance_cap_constants() {
        assert_eq!(GOVERNANCE_CAP_MIN_BPS, 500);
        assert_eq!(GOVERNANCE_CAP_MAX_BPS, 2000);
        assert_eq!(GOVERNANCE_CAP_DEFAULT_BPS, 1000);
        assert!(GOVERNANCE_CAP_MIN_BPS < GOVERNANCE_CAP_DEFAULT_BPS);
        assert!(GOVERNANCE_CAP_DEFAULT_BPS < GOVERNANCE_CAP_MAX_BPS);
    }

    #[test]
    fn test_ponc_rounds_constants() {
        assert_eq!(PONC_ROUNDS_MIN, 256);
        assert_eq!(PONC_ROUNDS_MAX, 2048);
        assert_eq!(PONC_ROUNDS_DEFAULT, 512);
        assert!(PONC_ROUNDS_MIN < PONC_ROUNDS_DEFAULT);
        assert!(PONC_ROUNDS_DEFAULT < PONC_ROUNDS_MAX);
    }
}
