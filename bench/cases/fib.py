import time

def fib(n):
    if n <= 1: return n
    return fib(n - 1) + fib(n - 2)

start = time.time_ns()
result = fib(30)
end = time.time_ns()

print(f"Fibonacci(30) = {result}")
print(f"Time: {(end - start) // 1000000}ms")
