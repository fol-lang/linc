#ifndef C_INTEROP_TORTURE_H
#define C_INTEROP_TORTURE_H

#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>

#define TORTURE_API_LEVEL 3
#define TORTURE_FLAG_A (1u << 0)
#define TORTURE_FLAG_B (1u << 1)
#define TORTURE_ABI_PACKED 1
#define TORTURE_PACKED __attribute__((packed))

typedef uint32_t torture_u32;
typedef struct torture_handle torture_handle;

typedef enum torture_mode {
    TORTURE_MODE_IDLE = 0,
    TORTURE_MODE_ACTIVE = 7,
    TORTURE_MODE_ERROR = 255,
} torture_mode;

typedef struct torture_config {
    torture_u32 flags;
    struct {
        unsigned short enabled : 1;
        unsigned short mode : 3;
        unsigned short reserved : 12;
    } state_bits;
    union {
        long raw_value;
        struct {
            short low;
            short high;
        } pair;
    } payload;
    const char *name;
} torture_config;

typedef struct TORTURE_PACKED torture_packet {
    uint8_t tag;
    uint16_t length;
    uint32_t checksum;
} torture_packet;

typedef struct torture_buffer {
    torture_u32 len;
    unsigned char data[];
} torture_buffer;

typedef void (*torture_callback)(torture_handle *handle, const torture_packet *packet, void *user_data);

struct torture_handle;

extern torture_handle *torture_open(const torture_config *config, torture_callback callback, void *user_data);
extern int torture_send(torture_handle *handle, const torture_buffer *buffer);
extern int torture_logf(torture_handle *handle, const char *fmt, ...);
extern size_t torture_packet_size(const torture_packet *packet);

static inline int torture_inline_only(int value) {
    return value + 1;
}

#endif
