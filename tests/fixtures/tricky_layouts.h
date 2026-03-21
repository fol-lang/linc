typedef unsigned long size_t;
typedef size_t my_size_t;
typedef const my_size_t *my_size_ptr;

struct packed_flags {
    unsigned int value : 3;
    unsigned int mode : 5;
    unsigned int count;
};

enum widget_mode {
    WIDGET_MODE_A = 0,
    WIDGET_MODE_B = 7
};
