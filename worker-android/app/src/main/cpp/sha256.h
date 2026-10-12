#ifndef SHA256_H
#define SHA256_H

#include <cstdint>
#include <cstddef>
#include <string>

class SHA256 {
public:
    SHA256();
    void update(const uint8_t* data, size_t length);
    void update(const void* data, size_t length) {
        update(static_cast<const uint8_t*>(data), length);
    }
    void finalize(uint8_t hash[32]);
    std::string finalizeHex();

private:
    void transform(const uint8_t block[64]);

    uint32_t m_state[8];
    uint64_t m_count;
    uint8_t m_buffer[64];
};

#endif // SHA256_H
