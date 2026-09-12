// destructure_matrix.ms

module test {
    export struct Box {
        val: int,
    }

    export struct Wrapper {
        b1: Box,
        b2: Box,
    }

    export fn test_destructure(w: Wrapper) -> int {
        // partial move semantics check
        dec Wrapper { b1, b2: Box { val } } = w;
        
        dec sum = b1.val + val;
        return sum;
    }

    export fn test_tuple(tup: (Box, int, bool)) -> int {
        dec (b, i, ..) = tup;
        return b.val + i;
    }
}
