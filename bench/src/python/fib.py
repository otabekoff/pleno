import time

def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)

start = time.time() * 1000
res = fib(30)
duration = (time.time() * 1000) - start

print(f"Python (Recursive Fib 30): {res}")
print(f"Time: {duration:.2f}ms")
