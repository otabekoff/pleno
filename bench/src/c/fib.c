#include <stdio.h>
#include <time.h>

long long fib(int n) {
    if (n < 2) return n;
    return fib(n - 1) + fib(n - 2);
}

int main() {
    clock_t start = clock();
    long long res = fib(30);
    clock_t end = clock();
    
    double duration_ms = ((double)(end - start)) / CLOCKS_PER_SEC * 1000.0;
    
    printf("C (Recursive Fib 30): %lld\n", res);
    printf("Time: %.2fms\n", duration_ms);
    return 0;
}
