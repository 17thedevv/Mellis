import os
import glob
import time

def find_file():
    dirs = [
        os.path.expandvars(r"%APPDATA%\Code\User\History"),
        os.path.expandvars(r"%APPDATA%\Cursor\User\History")
    ]
    cutoff = time.time() - (24 * 3600)  # last 24 hours
    
    matches = []
    for history_dir in dirs:
        if not os.path.exists(history_dir):
            continue
        for root, _, files in os.walk(history_dir):
            for file in files:
                path = os.path.join(root, file)
                try:
                    if os.path.getmtime(path) > cutoff:
                        size = os.path.getsize(path)
                        # We are looking for a file around 140KB (130k - 150k)
                        if 130000 < size < 150000:
                            matches.append((path, size, os.path.getmtime(path)))
                except Exception as e:
                    pass
                
    matches.sort(key=lambda x: x[2], reverse=True) # newest first
    for m in matches:
        print(f"Path: {m[0]}, Size: {m[1]}, Time: {m[2]}")

find_file()
