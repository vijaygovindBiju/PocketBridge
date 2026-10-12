#include <jni.h>
#include <cstdint>
#include <vector>
#include <thread>
#include <string>
#include "sha256.h"

static inline uint64_t rotl64(uint64_t x, int k) {
    return (x << k) | (x >> (64 - k));
}

static const uint64_t MULTIPLIER_1 = 0x9E3779B97F4A7C15ULL;
static const uint64_t MULTIPLIER_2 = 0x2545F4914F6CDD1DULL;

static inline uint64_t transform_record(uint64_t index, uint64_t seed, int32_t rounds) {
    uint64_t state = seed + (index * MULTIPLIER_1);
    for (int32_t r = 0; r < rounds; ++r) {
        state ^= (state >> 12);
        state ^= (state << 25);
        state ^= (state >> 27);
        state *= MULTIPLIER_2;
        state = rotl64(state, 13);
    }
    return state;
}

extern "C" JNIEXPORT jstring JNICALL
Java_com_pocketbridge_worker_runtime_ChunkTransformEngine_nativeTransform(
    JNIEnv* env,
    jobject /* thiz */,
    jlong recordCount,
    jint rounds,
    jlong seed
) {
    if (recordCount <= 0) {
        SHA256 hasher;
        std::string emptyHex = hasher.finalizeHex();
        return env->NewStringUTF(emptyHex.c_str());
    }

    uint32_t num_threads = std::thread::hardware_concurrency();
    if (num_threads == 0) {
        num_threads = 8;
    }

    uint64_t total = static_cast<uint64_t>(recordCount);
    if (total < 10000 || num_threads <= 1) {
        // Sequential path
        std::vector<uint8_t> buf(total * 8);
        uint8_t* ptr = buf.data();
        for (uint64_t i = 0; i < total; ++i) {
            uint64_t v = transform_record(i, static_cast<uint64_t>(seed), rounds);
            ptr[0] = static_cast<uint8_t>((v >> 56) & 0xFF);
            ptr[1] = static_cast<uint8_t>((v >> 48) & 0xFF);
            ptr[2] = static_cast<uint8_t>((v >> 40) & 0xFF);
            ptr[3] = static_cast<uint8_t>((v >> 32) & 0xFF);
            ptr[4] = static_cast<uint8_t>((v >> 24) & 0xFF);
            ptr[5] = static_cast<uint8_t>((v >> 16) & 0xFF);
            ptr[6] = static_cast<uint8_t>((v >> 8) & 0xFF);
            ptr[7] = static_cast<uint8_t>(v & 0xFF);
            ptr += 8;
        }

        SHA256 hasher;
        hasher.update(buf.data(), buf.size());
        std::string hex = hasher.finalizeHex();
        return env->NewStringUTF(hex.c_str());
    }

    // Parallel multi-core execution across all Oryon CPU cores
    uint64_t chunk_size = (total + num_threads - 1) / num_threads;
    std::vector<std::vector<uint8_t>> chunk_results(num_threads);
    std::vector<std::thread> workers;
    workers.reserve(num_threads);

    for (uint32_t t = 0; t < num_threads; ++t) {
        uint64_t start = t * chunk_size;
        uint64_t end = (start + chunk_size < total) ? (start + chunk_size) : total;
        if (start >= end) {
            continue;
        }

        workers.emplace_back([t, start, end, seed, rounds, &chunk_results]() {
            uint64_t count = end - start;
            std::vector<uint8_t>& buf = chunk_results[t];
            buf.resize(count * 8);
            uint8_t* ptr = buf.data();

            for (uint64_t i = start; i < end; ++i) {
                uint64_t v = transform_record(i, static_cast<uint64_t>(seed), rounds);
                ptr[0] = static_cast<uint8_t>((v >> 56) & 0xFF);
                ptr[1] = static_cast<uint8_t>((v >> 48) & 0xFF);
                ptr[2] = static_cast<uint8_t>((v >> 40) & 0xFF);
                ptr[3] = static_cast<uint8_t>((v >> 32) & 0xFF);
                ptr[4] = static_cast<uint8_t>((v >> 24) & 0xFF);
                ptr[5] = static_cast<uint8_t>((v >> 16) & 0xFF);
                ptr[6] = static_cast<uint8_t>((v >> 8) & 0xFF);
                ptr[7] = static_cast<uint8_t>(v & 0xFF);
                ptr += 8;
            }
        });
    }

    for (auto& w : workers) {
        if (w.joinable()) {
            w.join();
        }
    }

    SHA256 hasher;
    for (uint32_t t = 0; t < num_threads; ++t) {
        if (!chunk_results[t].empty()) {
            hasher.update(chunk_results[t].data(), chunk_results[t].size());
        }
    }

    std::string hex = hasher.finalizeHex();
    return env->NewStringUTF(hex.c_str());
}
