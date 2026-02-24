#include "ponc.h"
#include "sha3.h"

#include <algorithm>
#include <array>
#include <cstring>
#include <stdexcept>
#include <vector>

// Knotcoin Proof-of-Network-Contribution (PONC)
//
// 2 MB memory-hard PoW. Each candidate nonce requires 512 rounds of:
//   SHA3-256(state || scratchpad[index]) → next state
//
// The scratchpad is seeded from the previous block hash and the miner's
// address, making it unique per miner per block template.
//
// CONSENSUS CRITICAL: We use SHA3-256 (NIST FIPS 202, domain 0x06),
// NOT Keccak-256 (domain 0x01). Any external verifier must match exactly.

static constexpr size_t SCRATCHPAD_CHUNKS = 65536;  // 65536 × 32 = 2 MB
static constexpr size_t CHUNK_BYTES       = 32;
static constexpr size_t ROUNDS            = 512;
static constexpr size_t HEADER_PREFIX_LEN = 140; // 148 - 8 (nonce)

PoncEngine::PoncEngine()
    : scratchpad_(SCRATCHPAD_CHUNKS * CHUNK_BYTES, 0)
    , is_initialized_(false)
    , rounds_(ROUNDS)
{}

void PoncEngine::set_rounds(size_t rounds) {
    if (rounds < 256 || rounds > 2048) {
        throw std::invalid_argument("rounds must be 256-2048");
    }
    rounds_ = rounds;
}

void PoncEngine::initialize_scratchpad(
    rust::Slice<const uint8_t> prev_hash,
    rust::Slice<const uint8_t> miner_addr
) {
    if (prev_hash.size() < 32) throw std::invalid_argument("prev_hash too short");
    if (miner_addr.size() < 32) throw std::invalid_argument("miner_addr too short");

    // Clear any prior state so reuse is safe
    std::fill(scratchpad_.begin(), scratchpad_.end(), 0);
    is_initialized_ = false;

    // Stage 1: SHA3 counter-mode expansion from a per-template seed.
    std::array<uint8_t, 64> seed_material{};
    std::memcpy(seed_material.data(), prev_hash.data(), 32);
    std::memcpy(seed_material.data() + 32, miner_addr.data(), 32);
    const std::array<uint8_t, 32> seed = sha3_256(seed_material.data(), seed_material.size());

    for (size_t i = 0; i < SCRATCHPAD_CHUNKS; i++) {
        const uint64_t counter = static_cast<uint64_t>(i);
        std::array<uint8_t, 40> in{}; // 32 bytes seed + 8 bytes counter
        std::memcpy(in.data(), seed.data(), 32);
        
        // Encode counter as 8 bytes little-endian
        for (int b = 0; b < 8; b++) {
            in[32 + b] = static_cast<uint8_t>((counter >> (8 * b)) & 0xFF);
        }

        const std::array<uint8_t, 32> chunk = sha3_256(in.data(), in.size());
        std::memcpy(scratchpad_.data() + i * CHUNK_BYTES, chunk.data(), CHUNK_BYTES);
    }

    is_initialized_ = true;
}

bool PoncEngine::compute_and_verify(
    rust::Slice<const uint8_t> header_prefix,
    uint64_t nonce,
    rust::Slice<const uint8_t> target_bytes,
    rust::Slice<uint8_t> out_hash
) const {
    if (!is_initialized_)
        throw std::runtime_error("scratchpad not initialized");
    if (header_prefix.size() != HEADER_PREFIX_LEN)
        throw std::invalid_argument("header_prefix must be exactly 140 bytes");
    if (target_bytes.size() < 32 || out_hash.size() < 32)
        throw std::invalid_argument("target/out buffers too short");

    // Stage 2: initialize running state = sha3_256(header_prefix || nonce.le)
    std::array<uint8_t, 148> mix_buf{};
    std::memcpy(mix_buf.data(), header_prefix.data(), HEADER_PREFIX_LEN);
    
    // Nonce in little-endian as per checklist [FATAL]
    mix_buf[140] = static_cast<uint8_t>(nonce & 0xFF);
    mix_buf[141] = static_cast<uint8_t>((nonce >> 8) & 0xFF);
    mix_buf[142] = static_cast<uint8_t>((nonce >> 16) & 0xFF);
    mix_buf[143] = static_cast<uint8_t>((nonce >> 24) & 0xFF);
    mix_buf[144] = static_cast<uint8_t>((nonce >> 32) & 0xFF);
    mix_buf[145] = static_cast<uint8_t>((nonce >> 40) & 0xFF);
    mix_buf[146] = static_cast<uint8_t>((nonce >> 48) & 0xFF);
    mix_buf[147] = static_cast<uint8_t>((nonce >> 56) & 0xFF);

    std::array<uint8_t, 32> state = sha3_256(mix_buf.data(), mix_buf.size());

    for (size_t r = 0; r < rounds_; r++) {
        // Derive scratchpad index from the first 4 state bytes (little-endian).
        uint32_t idx =
            static_cast<uint32_t>(state[0]) |
            (static_cast<uint32_t>(state[1]) << 8) |
            (static_cast<uint32_t>(state[2]) << 16) |
            (static_cast<uint32_t>(state[3]) << 24);
        idx &= (SCRATCHPAD_CHUNKS - 1);

        // Hash state || scratchpad_chunk
        const uint8_t* chunk = scratchpad_.data() + idx * CHUNK_BYTES;
        std::array<uint8_t, 64> round_mix{};
        std::memcpy(round_mix.data(), state.data(), 32);
        std::memcpy(round_mix.data() + 32, chunk, 32);
        state = sha3_256(round_mix.data(), round_mix.size());
    }

    // Stage 3: final hash and target check.
    const std::array<uint8_t, 32> final_hash = sha3_256(state.data(), state.size());
    std::memcpy(out_hash.data(), final_hash.data(), 32);

    // Valid if hash ≤ target (big-endian comparison)
    for (size_t i = 0; i < 32; i++) {
        if (final_hash[i] < target_bytes[i]) return true;
        if (final_hash[i] > target_bytes[i]) return false;
    }
    return true; // exactly equal → valid
}

std::unique_ptr<PoncEngine> new_ponc_engine() {
    return std::make_unique<PoncEngine>();
}
