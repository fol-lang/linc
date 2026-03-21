#include "../../header/include/zlib.h"

#if !defined(STDC)
#define STDC 1
#endif

#include <stddef.h>
#include <string.h>
#include <stdlib.h>
#include <limits.h>

#ifndef local
#define local static
#endif

typedef unsigned char uch;
typedef uch FAR uchf;
typedef unsigned short ush;
typedef ush FAR ushf;
typedef unsigned long ulg;

#if (ULONG_MAX == 0xffffffffffffffffUL)
#define Z_U8 unsigned long
#elif defined(ULLONG_MAX) && (ULLONG_MAX == 0xffffffffffffffffULL)
#define Z_U8 unsigned long long
#elif (UINT_MAX == 0xffffffffffffffffU)
#define Z_U8 unsigned
#endif

#define ERR_MSG(err) z_errmsg[(err) < -6 || (err) > 2 ? 9 : 2 - (err)]
#define ERR_RETURN(strm, err) return (strm->msg = ERR_MSG(err), (err))

#ifndef DEF_WBITS
#define DEF_WBITS MAX_WBITS
#endif

#if MAX_MEM_LEVEL >= 8
#define DEF_MEM_LEVEL 8
#else
#define DEF_MEM_LEVEL MAX_MEM_LEVEL
#endif

#define STORED_BLOCK 0
#define STATIC_TREES 1
#define DYN_TREES 2

#define MIN_MATCH 3
#define MAX_MATCH 258
#define PRESET_DICT 0x20

#ifndef OS_CODE
#define OS_CODE 3
#endif

#ifndef F_OPEN
#define F_OPEN(name, mode) fopen((name), (mode))
#endif

#define zmemcpy memcpy
#define zmemcmp memcmp
#define zmemzero(dest, len) memset(dest, 0, len)

extern z_const char * const z_errmsg[10];

#define ZALLOC(strm, items, size) \
    (*((strm)->zalloc))((strm)->opaque, (items), (size))
#define ZFREE(strm, addr) (*((strm)->zfree))((strm)->opaque, (voidpf)(addr))
#define TRY_FREE(s, p) { if (p) ZFREE(s, p); }

#define ZSWAP32(q) ((((q) >> 24) & 0xff) + (((q) >> 8) & 0xff00) + \
                    (((q) & 0xff00) << 8) + (((q) & 0xff) << 24))
