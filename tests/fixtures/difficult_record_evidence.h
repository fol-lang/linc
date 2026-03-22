typedef struct nested_header {
    unsigned short tag;
    unsigned char kind;
    unsigned char flags;
} nested_header_t;

typedef struct message_payload {
    double weight;
    nested_header_t header;
    long code;
} message_payload_t;

struct packed_window {
    unsigned int state : 3;
    unsigned int mode : 5;
    unsigned short span;
    message_payload_t payload;
};

extern message_payload_t current_payload;
extern struct packed_window current_window;
