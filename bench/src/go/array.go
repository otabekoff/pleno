package main

import (
	"fmt"
	"time"
)

func main() {
	start := time.Now()

	// Create 1M array
	arr := make([]int, 1000000)
	for i := 0; i < 1000000; i++ {
		arr[i] = i
	}

	// Map * 2
	mapped := make([]int, len(arr))
	for i, v := range arr {
		mapped[i] = v * 2
	}

	// Filter % 3
	var filtered []int
	for _, v := range mapped {
		if v%3 == 0 {
			filtered = append(filtered, v)
		}
	}

	// Reduce sum
	sum := 0
	for _, v := range filtered {
		sum += v
	}

	duration := time.Since(start)

	fmt.Printf("Go (Array Ops 1M): %d\n", sum)
	fmt.Printf("Time: %.2fms\n", float64(duration.Microseconds())/1000.0)
}
