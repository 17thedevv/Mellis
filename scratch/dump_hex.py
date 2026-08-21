import os
filepath = "tests/e2e/test_vec_out.txt"
with open(filepath, "rb") as f:
    lines = f.readlines()

print(lines[74])
