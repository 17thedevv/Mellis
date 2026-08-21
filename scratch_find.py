import struct
import sys

def main():
    try:
        with open('lib/alloc.mlib', 'rb') as f:
            data = f.read()
    except Exception as e:
        print(f"Error reading file: {e}")
        return
        
    pos = data.find(b'locator')
    if pos != -1:
        print(f"Found 'locator' at offset {pos} in file")
        
        # MLibHeader is 122 bytes packed
        sectionCount = struct.unpack('<I', data[110:114])[0]
        sectionTableOffset = struct.unpack('<Q', data[114:122])[0]
        print(f"Sections: {sectionCount}, TableOffset: {sectionTableOffset}")
        
        offset = sectionTableOffset
        # SectionEntry is also packed!
        # uint32_t id, uint32_t type, uint64_t off, uint64_t size, uint16_t ver, uint8_t comp, uint8_t res[5], uint64_t hash
        # 4 + 4 + 8 + 8 + 2 + 1 + 5 + 8 = 40 bytes
        for i in range(sectionCount):
            sec_id, sec_type, sec_offset, sec_size = struct.unpack('<IIQQ', data[offset:offset+24])
            print(f"[{i}] Type={sec_type} Offset={sec_offset} Size={sec_size}")
            
            if sec_type == 4: # StringTable
                print(f"StringTable starts at {sec_offset}")
                print(f"'locator' is at offset {pos - sec_offset} in StringTable")
                
                print("\nAll strings:")
                strs = data[sec_offset:sec_offset+sec_size].split(b'\0')
                current_off = 0
                for s in strs:
                    if s:
                        print(f"  {current_off}: {s.decode('utf-8', errors='replace')}")
                    current_off += len(s) + 1
            offset += 40

if __name__ == '__main__':
    main()
