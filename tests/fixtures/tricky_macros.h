#define API_LEVEL 7
#define API_NAME "demo"
#define HAVE_WIDGETS 1
#define WIDGET_PACK __attribute__((packed))
#define DECLARE_WIDGET(name) int widget_##name(void)
