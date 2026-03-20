#ifndef BIC_MAX_PAIN_H
#define BIC_MAX_PAIN_H

#include <stddef.h>
#include <stdint.h>

#include "../plugin_abi.h"

typedef struct bic_daemon bic_daemon;
typedef struct bic_tls_client bic_tls_client;

typedef struct bic_daemon_packet {
    uint64_t sequence;
    uint32_t source_kind;
    uint32_t flags;
    const uint8_t *payload;
    size_t payload_len;
} bic_daemon_packet;

typedef struct bic_daemon_config {
    int epoll_fd;
    int timer_fd;
    int signal_fd;
    const char *endpoint_url;
    const bic_plugin_descriptor *output_plugin;
} bic_daemon_config;

typedef void (*bic_daemon_packet_cb)(bic_daemon *daemon, const bic_daemon_packet *packet, void *user_data);

typedef struct bic_daemon_hooks {
    bic_daemon_packet_cb on_packet;
    void *user_data;
} bic_daemon_hooks;

extern bic_daemon *bic_daemon_create(const bic_daemon_config *config, const bic_daemon_hooks *hooks);
extern void bic_daemon_destroy(bic_daemon *daemon);
extern int bic_daemon_submit_packet(bic_daemon *daemon, const bic_daemon_packet *packet);
extern int bic_daemon_enable_socketcan(bic_daemon *daemon, int can_socket_fd);
extern int bic_daemon_enable_pcap(bic_daemon *daemon, void *pcap_handle);
extern int bic_daemon_enable_tls(bic_daemon *daemon, bic_tls_client *client);

#endif
