package main

import (
	"fmt"
	"time"
)

func main() {
	start := time.Now()
	sum := 0
	for i := 0; i < 10000000; i++ {
		sum += i
	}
	duration := time.Since(start)

	fmt.Printf("Go (Loop Sum 10M): %d\n", sum)
	fmt.Printf("Time: %.2fms\n", float64(duration.Microseconds())/1000.0)
}
