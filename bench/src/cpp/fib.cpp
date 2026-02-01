#include <iostream>
#include <chrono>

long long fib(int n) {
    if (n < 2) return n;
    return fib(n - 1) + fib(n - 2);
}

int main() {
    auto start = std::chrono::high_resolution_clock::now();
    long long res = fib(30);
    auto end = std::chrono::high_resolution_clock::now();
    
    std::chrono::duration<double, std::milli> duration = end - start;
    
    std::cout << "C++ (Recursive Fib 30): " << res << std::endl;
    std::cout << "Time: " << duration.count() << "ms" << std::endl;
    return 0;
}
