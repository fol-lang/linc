#include <stdio.h>
#include <string.h>

int main(int argc, char **argv) {
    if (argc == 2 && strcmp(argv[1], "--version") == 0) {
        puts("gcc (LINC cross fixture) 1.0");
        return 0;
    }
    if (argc == 2 && strcmp(argv[1], "-dumpmachine") == 0) {
        puts("aarch64-unknown-linux-gnu");
        return 0;
    }
    if (argc == 2 && strcmp(argv[1], "-print-sysroot") == 0) {
        puts("/");
        return 0;
    }
    for (int index = 1; index + 1 < argc; ++index) {
        if (strcmp(argv[index], "-o") == 0) {
            FILE *output = fopen(argv[index + 1], "wb");
            if (output == NULL) {
                return 2;
            }
            fputs("deliberately non-executable foreign object", output);
            return fclose(output) == 0 ? 0 : 3;
        }
    }
    return 4;
}
