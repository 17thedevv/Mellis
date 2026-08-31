import os
import subprocess

def create_file(path, content):
    with open(path, 'w') as f:
        f.write(content)

os.makedirs('scratch/modules', exist_ok=True)

# 1. alloc.mlib exports std namespace with Vec
create_file('scratch/modules/alloc.ms', '''
module std {
    export struct Vec<T> {
        ptr: *rw T,
        len: i32,
        cap: i32,
    }
}
''')

# 2. core.mlib exports std namespace with Option
create_file('scratch/modules/core.ms', '''
module std {
    export enum Option<T> {
        Some(T),
        None,
    }
    
    export module collections {
        export struct Map<K, V> {
            k: K,
            v: V,
        }
    }
}
''')

# 3. main.ms that imports both and accesses std::Vec and std::Option
create_file('scratch/modules/main.ms', '''
import <alloc>;
import <core>;

fn main() -> i32 {
    dec v: std::Vec<i32>;
    dec o: std::Option<i32>;
    dec m: std::collections::Map<i32, i32>;
    return 0;
}
''')

# 4. Duplicate symbols test
create_file('scratch/modules/dup_core.ms', '''
module std {
    export enum Vec<T> {
        Duplicate(T),
    }
}
''')
create_file('scratch/modules/dup_main.ms', '''
import <alloc>;
import "dup_core"; 

fn main() -> i32 {
    dec v: std::Vec<i32>;
    return 0;
}
''')
