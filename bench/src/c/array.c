#include <stdio.h>
#include <stdlib.h>
#include <time.h>

int main() {
    clock_t start = clock();

    // Create 1M array
    int size = 1000000;
    int* arr = (int*)malloc(size * sizeof(int));
    for(int i=0; i<size; i++) arr[i] = i;

    // Map * 2
    int* mapped = (int*)malloc(size * sizeof(int));
    for(int i=0; i<size; i++) mapped[i] = arr[i] * 2;

    // Filter % 3 (resize later or count first, simplified: allocate max)
    int* filtered = (int*)malloc(size * sizeof(int));
    int count = 0;
    for(int i=0; i<size; i++) {
        if (mapped[i] % 3 == 0) {
            filtered[count++] = mapped[i];
        }
    }

    // Reduce
    long long sum = 0;
    for(int i=0; i<count; i++) sum += filtered[i];

    clock_t end = clock();
    
    double duration_ms = ((double)(end - start)) / CLOCKS_PER_SEC * 1000.0;
    
    printf("C (Array Ops 1M): %lld\n", sum);
    printf("Time: %.2fms\n", duration_ms);
    
    free(arr);
    free(mapped);
    free(filtered);
    return 0;
}
