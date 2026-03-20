#include "max_pain.h"

struct bic_daemon {
    bic_daemon_config config;
    bic_daemon_hooks hooks;
};

bic_daemon *bic_daemon_create(const bic_daemon_config *config, const bic_daemon_hooks *hooks) {
    (void)config;
    (void)hooks;
    return (bic_daemon *)0;
}

void bic_daemon_destroy(bic_daemon *daemon) {
    (void)daemon;
}

int bic_daemon_submit_packet(bic_daemon *daemon, const bic_daemon_packet *packet) {
    (void)daemon;
    (void)packet;
    return 0;
}

int bic_daemon_enable_socketcan(bic_daemon *daemon, int can_socket_fd) {
    (void)daemon;
    (void)can_socket_fd;
    return 0;
}

int bic_daemon_enable_pcap(bic_daemon *daemon, void *pcap_handle) {
    (void)daemon;
    (void)pcap_handle;
    return 0;
}

int bic_daemon_enable_tls(bic_daemon *daemon, bic_tls_client *client) {
    (void)daemon;
    (void)client;
    return 0;
}
