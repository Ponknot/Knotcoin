#include "sha3.h"

#include <cstring>

static const uint64_t RC[24] = {
    0x0000000000000001ULL, 0x0000000000008082ULL, 0x800000000000808aULL,
    0x8000000080008000ULL, 0x000000000000808bULL, 0x0000000080000001ULL,
    0x8000000080008081ULL, 0x8000000000008009ULL, 0x000000000000008aULL,
    0x0000000000000088ULL, 0x0000000080008009ULL, 0x000000008000000aULL,
    0x000000008000808bULL, 0x800000000000008bULL, 0x8000000000008089ULL,
    0x8000000000008003ULL, 0x8000000000008002ULL, 0x8000000000000080ULL,
    0x000000000000800aULL, 0x800000008000000aULL, 0x8000000080008081ULL,
    0x8000000000008080ULL, 0x0000000080000001ULL, 0x8000000080008008ULL
};

static const int ROTC[24] = {
    1,  3,  6,  10, 15, 21, 28, 36, 45, 55, 2,  14,
    27, 41, 56, 8,  25, 43, 62, 18, 39, 61, 20, 44
};

static const int PILN[24] = {
    10, 7,  11, 17, 18, 3,  5,  16, 8,  21, 24, 4,
    15, 23, 19, 13, 12, 2,  20, 14, 22, 9,  6,  1
};

#define ROL64(a, offset) ((((uint64_t)a) << offset) ^ (((uint64_t)a) >> (64 - offset)))

static void keccak_f1600(uint64_t state[25]) {
    uint64_t bc[5];
    for (int round = 0; round < 24; round++) {
        for (int i = 0; i < 5; i++) {
            bc[i] = state[i] ^ state[i + 5] ^ state[i + 10] ^ state[i + 15] ^ state[i + 20];
        }

        for (int i = 0; i < 5; i++) {
            uint64_t t = bc[(i + 4) % 5] ^ ROL64(bc[(i + 1) % 5], 1);
            for (int j = 0; j < 25; j += 5) {
                state[j + i] ^= t;
            }
        }

        uint64_t t = state[1];
        for (int i = 0; i < 24; i++) {
            int j = PILN[i];
            uint64_t tmp = state[j];
            state[j] = ROL64(t, ROTC[i]);
            t = tmp;
        }

        for (int j = 0; j < 25; j += 5) {
            for (int i = 0; i < 5; i++) {
                bc[i] = state[j + i];
            }
            for (int i = 0; i < 5; i++) {
                state[j + i] ^= (~bc[(i + 1) % 5]) & bc[(i + 2) % 5];
            }
        }

        state[0] ^= RC[round];
    }
}

std::array<uint8_t, 32> sha3_256(const uint8_t* data, size_t len) {
    static constexpr size_t RATE = 136; // SHA3-256 rate
    uint64_t state[25] = {};
    uint8_t buffer[RATE] = {};
    size_t buffer_len = 0;

    while (len > 0) {
        size_t take = RATE - buffer_len;
        if (take > len) {
            take = len;
        }

        std::memcpy(buffer + buffer_len, data, take);
        buffer_len += take;
        data += take;
        len -= take;

        if (buffer_len == RATE) {
            for (int i = 0; i < 17; i++) {
                uint64_t v = 0;
                for (int j = 0; j < 8; j++) {
                    v |= ((uint64_t)buffer[i * 8 + j]) << (8 * j);
                }
                state[i] ^= v;
            }
            keccak_f1600(state);
            buffer_len = 0;
            std::memset(buffer, 0, RATE);
        }
    }

    // SHA3-256 domain separator: 0x06 (NOT Keccak's 0x01).
    buffer[buffer_len] = 0x06;
    std::memset(buffer + buffer_len + 1, 0, RATE - buffer_len - 1);
    buffer[RATE - 1] |= 0x80;

    for (int i = 0; i < 17; i++) {
        uint64_t v = 0;
        for (int j = 0; j < 8; j++) {
            v |= ((uint64_t)buffer[i * 8 + j]) << (8 * j);
        }
        state[i] ^= v;
    }
    keccak_f1600(state);

    std::array<uint8_t, 32> out{};
    for (int i = 0; i < 4; i++) {
        for (int j = 0; j < 8; j++) {
            out[i * 8 + j] = (state[i] >> (8 * j)) & 0xFF;
        }
    }
    return out;
}
