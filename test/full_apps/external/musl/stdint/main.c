#include <stdint.h>

struct CounterWindow {
    int_fast16_t current;
    uint_least32_t limit;
    uintptr_t cookie;
};

static struct CounterWindow make_window(void) {
    struct CounterWindow window = {
        INT_FAST16_MIN,
        UINT_LEAST32_MAX,
        (uintptr_t)0
    };
    return window;
}

int main(void) {
    struct CounterWindow window = make_window();
    return window.current == INT_FAST16_MIN ? 0 : 1;
}
