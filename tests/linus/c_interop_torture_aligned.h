#ifndef C_INTEROP_TORTURE_ALIGNED_H
#define C_INTEROP_TORTURE_ALIGNED_H

#include <stddef.h>
#include <stdint.h>

#define TORTURE_ALIGNED_LEVEL 4
#define TORTURE_ALIGN16 __attribute__((aligned(16)))

typedef struct TORTURE_ALIGN16 torture_aligned_packet {
    uint32_t code;
    uint64_t payload_len;
} torture_aligned_packet;

extern size_t torture_aligned_size(const torture_aligned_packet *packet);

#endif
