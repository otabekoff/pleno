import time

start = time.time() * 1000

# Create 1M array
arr = list(range(1000000))

# Map * 2
mapped = list(map(lambda x: x * 2, arr))

# Filter divisible by 3
filtered = list(filter(lambda x: x % 3 == 0, mapped))

# Reduce sum (standard loop or functools)
from functools import reduce
sum_val = reduce(lambda acc, x: acc + x, filtered, 0)

duration = (time.time() * 1000) - start

print(f"Python (Array Ops 1M): {sum_val}")
print(f"Time: {duration:.2f}ms")
