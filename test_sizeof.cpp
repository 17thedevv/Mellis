#include <iostream>
#include <cstddef>
#pragma pack(push, 1)
struct MLibHeader {
    char magic[4];             
    uint16_t formatVersion;    
    uint16_t compilerVersion;  
    uint16_t mvirVersion;      
    char targetTriple[64];     
    uint8_t moduleUUID[16];    
    uint64_t moduleHash;       
    uint64_t timestamp;        
    uint32_t flags;            
    uint32_t sectionCount;     
    uint64_t sectionTableOffset;
};
#pragma pack(pop)
int main() { 
    std::cout << "size: " << sizeof(MLibHeader) << "\n"; 
    std::cout << "uuid: " << offsetof(MLibHeader, moduleUUID) << "\n";
    std::cout << "count: " << offsetof(MLibHeader, sectionCount) << "\n";
    return 0; 
}
