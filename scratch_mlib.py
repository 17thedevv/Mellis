import struct
import sys

def main():
    try:
        with open('lib/alloc.mlib', 'rb') as f:
            data = f.read()
    except Exception as e:
        print(f"Error reading file: {e}")
        return

    magic, version, num_sections = struct.unpack('<4sII', data[:12])
    print(f'magic={magic}, sections={num_sections}')
    
    offset = 12
    for i in range(num_sections):
        sec_type, sec_offset, sec_size = struct.unpack('<BQQ', data[offset:offset+17])
        print(f"[{i}] Type={sec_type} Offset={sec_offset} Size={sec_size}")
        
        if sec_type == 4: # StringTable
            print("\nStrings in StringTable:")
            str_data = data[sec_offset:sec_offset+sec_size]
            current_offset = 0
            while current_offset < len(str_data):
                end = str_data.find(b'\0', current_offset)
                if end == -1:
                    break
                s = str_data[current_offset:end].decode('utf-8', errors='replace')
                if s:
                    print(f"  Offset {current_offset}: {s}")
                current_offset = end + 1
        
        offset += 17

if __name__ == '__main__':
    main()
