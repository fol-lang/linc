#ifndef BIC_EPOLL_FIXTURE_H
#define BIC_EPOLL_FIXTURE_H

typedef union epoll_data {
    void *ptr;
    int fd;
    unsigned int u32;
    unsigned long long u64;
} epoll_data_t;

struct epoll_event {
    unsigned int events;
    epoll_data_t data;
};

#define EPOLLIN 0x001
#define EPOLLOUT 0x004

#endif
