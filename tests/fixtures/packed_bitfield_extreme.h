#pragma pack(push, 1)
struct packed_registers {
    unsigned ready : 1;
    unsigned mode : 3;
    unsigned error : 1;
    unsigned reserved : 3;
    unsigned short count;
    unsigned long long timestamp;
};
#pragma pack(pop)

typedef struct packed_registers packed_registers_t;
