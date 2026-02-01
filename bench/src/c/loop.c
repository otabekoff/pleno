#include <stdio.h>
#include <time.h>

int main() {
    clock_t start = clock();
    volatile long long sum = 0;
    for (long long i = 0; i < 10000000; i++) {
        sum += i;
    }
    clock_t end = clock();
    
    double duration_ms = ((double)(end - start)) / CLOCKS_PER_SEC * 1000.0;
    
    printf("C (Loop Sum 10M): %lld\n", sum);
    printf("Time: %.2fms\n", duration_ms);
    return 0;
}
