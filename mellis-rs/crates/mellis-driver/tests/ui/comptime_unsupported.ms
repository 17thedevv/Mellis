fn main() { //~ ERROR cannot evaluate comptime block: unsupported comptime operation: comptime expressions are not yet supported
    dec x = comptime { 1 };
}
