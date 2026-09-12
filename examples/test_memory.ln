struct Vector3 {
    x: int_32;
    y: int_32;
    z: int_32;
}

export fn main() -> int_32 {
    dec v: Vector3;
    v.x = 10;
    v.y = 20;
    
    {
        dec ref_z: &rw int_32 = &rw v.z;
        *ref_z = 30; // v.z = 30
    }
    
    dec sum: int_32 = 0;
    dec i: int_32 = 0;
    
    while i < 3 {
        if i == 0 {
            sum = sum + v.x;
        }
        if i == 1 {
            sum = sum + v.y;
        }
        if i == 2 {
            sum = sum + v.z;
        }
        i = i + 1;
    }
    
    return sum; // 10 + 20 + 30 = 60
}
