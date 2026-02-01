#include <iostream>
#include <chrono>

int main() {
    auto start = std::chrono::high_resolution_clock::now();
    volatile long long sum = 0;
    for (long long i = 0; i < 10000000; i++) {
        sum += i;
    }
    auto end = std::chrono::high_resolution_clock::now();
    
    std::chrono::duration<double, std::milli> duration = end - start;
    
    std::cout << "C++ (Loop Sum 10M): " << sum << std::endl;
    std::cout << "Time: " << duration.count() << "ms" << std::endl;
    return 0;
}
