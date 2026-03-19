typedef struct widget {
    int code;
    double weight;
} widget_t;

typedef enum mode {
    MODE_A = 0,
    MODE_B = 1
} mode_t;

extern widget_t widget_global;
extern mode_t current_mode;
