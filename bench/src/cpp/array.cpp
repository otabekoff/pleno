#include <iostream>
#include <vector>
#include <algorithm>
#include <numeric>
#include <chrono>

int main() {
    auto start = std::chrono::high_resolution_clock::now();

    // Create 1M vector
    std::vector<int> arr(1000000);
    std::iota(arr.begin(), arr.end(), 0);

    // Map * 2
    std::vector<int> mapped;
    mapped.reserve(arr.size());
    std::transform(arr.begin(), arr.end(), std::back_inserter(mapped), [](int x) { return x * 2; });

    // Filter % 3
    std::vector<int> filtered;
    filtered.reserve(mapped.size());
    std::copy_if(mapped.begin(), mapped.end(), std::back_inserter(filtered), [](int x) { return x % 3 == 0; });

    // Reduce
    long long sum = std::accumulate(filtered.begin(), filtered.end(), 0LL);

    auto end = std::chrono::high_resolution_clock::now();
    std::chrono::duration<double, std::milli> duration = end - start;
    
    std::cout << "C++ (Array Ops 1M): " << sum << std::endl;
    std::cout << "Time: " << duration.count() << "ms" << std::endl;
    return 0;
}
