#ifndef KNOTCOIN_SHA3_H
#define KNOTCOIN_SHA3_H

#include <array>
#include <cstddef>
#include <cstdint>

// SHA3-256 (NIST FIPS 202). Domain separator is 0x06.
std::array<uint8_t, 32> sha3_256(const uint8_t* data, size_t len);

#endif  // KNOTCOIN_SHA3_H
