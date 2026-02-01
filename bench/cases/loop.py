import time
sum = 0
start = time.time_ns()
for i in range(1000000):
    sum += i
end = time.time_ns()
print(f"Loop Sum = {sum}")
print(f"Time: {(end - start) // 1000000}ms")
