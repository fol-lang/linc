#include "png.h"

static png_color make_color(png_byte red, png_byte green, png_byte blue)
{
    png_color color = { red, green, blue };
    return color;
}

int libpng_header_fixture(void)
{
    png_color color = make_color(1u, 2u, 3u);
    png_uint_32 mask = PNG_COLOR_MASK_COLOR | PNG_COLOR_MASK_ALPHA;
    return (int)color.red + (int)mask;
}
