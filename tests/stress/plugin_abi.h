#ifndef BIC_PLUGIN_ABI_H
#define BIC_PLUGIN_ABI_H

#include <stddef.h>
#include <stdint.h>

#define BIC_PLUGIN_ABI_VERSION 1

typedef struct bic_plugin_host bic_plugin_host;
typedef struct bic_plugin_instance bic_plugin_instance;

typedef struct bic_plugin_message {
    const uint8_t *data;
    size_t len;
} bic_plugin_message;

typedef void (*bic_plugin_log_fn)(bic_plugin_host *host, const char *message, void *user_data);
typedef int (*bic_plugin_emit_fn)(bic_plugin_host *host, const bic_plugin_message *message, void *user_data);

typedef struct bic_plugin_host_vtable {
    bic_plugin_log_fn log;
    bic_plugin_emit_fn emit;
    void *user_data;
} bic_plugin_host_vtable;

typedef struct bic_plugin_descriptor {
    const char *name;
    uint32_t abi_version;
    bic_plugin_instance *(*create)(const bic_plugin_host_vtable *host);
    void (*destroy)(bic_plugin_instance *instance);
    int (*submit)(bic_plugin_instance *instance, const bic_plugin_message *message);
} bic_plugin_descriptor;

extern const bic_plugin_descriptor *bic_plugin_descriptor_v1(void);

#endif
