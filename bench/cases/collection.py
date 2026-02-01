import time
from functools import reduce

start = time.time_ns()

# Collection operations benchmark
arr = list(range(10000))

# Map
doubled = list(map(lambda x: x * 2, arr))

# Filter
evens = list(filter(lambda x: x % 4 == 0, doubled))

# Reduce
total = reduce(lambda acc, x: acc + x, evens, 0)

end = time.time_ns()
print(f"Original length: {len(arr)}")
print(f"Doubled length: {len(doubled)}")
print(f"Evens length: {len(evens)}")
print(f"Sum: {total}")
print(f"Time: {(end - start) / 1000000}ms")
