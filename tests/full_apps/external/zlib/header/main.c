#include "zlib.h"

static z_stream make_stream(Bytef *input, Bytef *output)
{
    z_stream stream = {
        input,
        32u,
        0u,
        output,
        64u,
        0u,
        "fixture",
        (alloc_func)0,
        (free_func)0,
        (voidpf)0,
        0,
        0,
        0,
        Z_NULL,
        Z_NULL,
        Z_NULL,
        0,
        0
    };
    return stream;
}

int fixture_uses_zlib_types(Bytef *input, Bytef *output)
{
    z_stream stream = make_stream(input, output);
    return stream.zalloc == (alloc_func)0 && stream.zfree == (free_func)0;
}
