import os
filepath = "lib/core/mod.mlib"
with open(filepath, "rb") as f:
    data = f.read()

idx = data.find(b"rt mod iter")
if idx != -1:
    print("Found at", idx)
    print(data[idx-10:idx+50])
else:
    print("Not found in file!")
