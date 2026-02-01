package main

import (
	"fmt"
	"time"
)

func fib(n int) int {
	if n < 2 {
		return n
	}
	return fib(n-1) + fib(n-2)
}

func main() {
	start := time.Now()
	res := fib(30)
	duration := time.Since(start)

	fmt.Printf("Go (Recursive Fib 30): %d\n", res)
	fmt.Printf("Time: %.2fms\n", float64(duration.Microseconds())/1000.0)
}
