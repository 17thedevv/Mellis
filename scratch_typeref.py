import struct
import sys

def main():
    try:
        with open('lib/alloc.mlib', 'rb') as f:
            data = f.read()
    except Exception as e:
        return
        
    sectionTableOffset = struct.unpack('<Q', data[114:122])[0]
    sectionCount = struct.unpack('<I', data[110:114])[0]
    offset = sectionTableOffset
    
    typeref_sec = None
    for i in range(sectionCount):
        sec_id, sec_type, sec_offset, sec_size = struct.unpack('<IIQQ', data[offset:offset+24])
        if sec_type == 14: # TypeRefTable
            typeref_sec = (sec_offset, sec_size)
        offset += 40
        
    if not typeref_sec:
        print("No TypeRefTable found!")
        return
        
    t_off, t_size = typeref_sec
    print(f"TypeRefTable at offset {t_off}, size {t_size}")
    
    count = struct.unpack('<I', data[t_off:t_off+4])[0]
    print(f"Count: {count}")
    
    p = t_off + 4
    for i in range(count):
        kind, flags, payloadSize = struct.unpack('<BBH', data[p:p+4])
        p += 4
        
        numWords = payloadSize // 4
        payload = struct.unpack(f'<{numWords}I', data[p:p+numWords*4])
        p += numWords * 4
        
        if kind == 1: # Named
            if len(payload) >= 2:
                print(f"[{i}] Named: nameStringId={payload[0]}, numArgs={payload[1]}, rest={payload[2:]}")
            else:
                print(f"[{i}] Named: MALFORMED (len {len(payload)})")
        else:
            print(f"[{i}] Kind={kind} payload={payload}")

if __name__ == '__main__':
    main()
