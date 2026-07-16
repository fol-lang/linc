__attribute__((used))
static int parc_open(void *handle) {
    return handle != 0;
}

int local_anchor(void) {
    return parc_open(0);
}
