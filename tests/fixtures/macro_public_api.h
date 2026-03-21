#define WIDGET_API_LEVEL 12
#define WIDGET_ENABLE_FAST_PATH 1
#define WIDGET_POINTER_BITS 64
#define WIDGET_PACKED __attribute__((packed))
#define WIDGET_SLOT_COUNT (WIDGET_POINTER_BITS / 8)
#define WIDGET_TIMEOUT_MS 250
#define WIDGET_DECLARE_HANDLE(name) typedef struct name##_handle* name##_handle_t

WIDGET_DECLARE_HANDLE(widget);
