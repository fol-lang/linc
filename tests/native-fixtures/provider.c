extern int dependency_api(int);

int provider_api(int value) {
    return dependency_api(value) + 1;
}

int exported_data = 42;
__thread int exported_tls = 3;
