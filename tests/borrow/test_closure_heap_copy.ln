fn execute<T>(f: T) -> int_32 {
    return f();
}

fn make_closure_and_execute() -> int_32 {
    dec x = 10;
    // Closure captures x by value and moves into 'execute', 
    // which outlives the local scope if execute returns it or stores it?
    // Wait, if it's passed to execute by value, does it escape?
    // Actually, to make it escape, it needs to be returned or stored.
    dec my_closure = move | | -> int_32 { return x; };
    return execute(my_closure);
}

fn main() -> int_32 {
    return make_closure_and_execute();
}
