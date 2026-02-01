import time

start = time.time_ns()

# String operations benchmark
result = ""
for i in range(10000):
    result = result + "x"

upper = result.upper()
replaced = upper.replace("X", "Y")
parts = replaced.split("Y")

end = time.time_ns()
print(f"String length: {len(result)}")
print(f"Parts count: {len(parts)}")
print(f"Time: {(end - start) / 1000000}ms")
