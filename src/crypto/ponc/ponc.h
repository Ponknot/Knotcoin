#ifndef KNOTCOIN_PONC_H
#define KNOTCOIN_PONC_H

#include <cstdint>
#include <memory>
#include <vector>

#include "rust/cxx.h"

class PoncEngine {
public:
    PoncEngine();

    void initialize_scratchpad(
        rust::Slice<const uint8_t> prev_hash,
        rust::Slice<const uint8_t> miner_address
    );

    bool compute_and_verify(
        rust::Slice<const uint8_t> header_prefix,
        uint64_t nonce,
        rust::Slice<const uint8_t> target_bytes,
        rust::Slice<uint8_t> out_hash
    ) const;

    void set_rounds(size_t rounds);

private:
    std::vector<uint8_t> scratchpad_;
    bool is_initialized_;
    size_t rounds_;
};

std::unique_ptr<PoncEngine> new_ponc_engine();

#endif  // KNOTCOIN_PONC_H
