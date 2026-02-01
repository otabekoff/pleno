import time

start = time.time() * 1000
sum_val = 0
for i in range(10000000):
    sum_val += i
duration = (time.time() * 1000) - start

print(f"Python (Loop Sum 10M): {sum_val}")
print(f"Time: {duration:.2f}ms")
