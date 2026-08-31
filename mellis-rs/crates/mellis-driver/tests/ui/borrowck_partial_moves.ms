struct String {
    len: i32,
}

struct Point {
    x: String,
    y: String,
}

fn consume_string(val: String) {}
fn consume_point(p: Point) {}

fn test_partial_move_ok() {
    dec p = Point { x: String { len: 10 }, y: String { len: 20 } };
    consume_string(p.x); // Moves p.x
    consume_string(p.y); // Should be OK
}

fn test_partial_move_err() {
    dec p = Point { x: String { len: 10 }, y: String { len: 20 } };
    consume_string(p.x); // Moves p.x
    consume_point(p); //~ ERROR Use of partially moved value
}

fn test_partial_move_err2() {
    dec p = Point { x: String { len: 10 }, y: String { len: 20 } };
    consume_point(p); // Moves p
    consume_string(p.x); //~ ERROR Use of moved value
}

// NOTE: Piecewise initialization (dec p: T; p.x = ...) is a known limitation.
// The typechecker does not yet support member access on uninitialized struct variables.
// These tests are commented out until piecewise init is implemented.
//
// fn test_piecewise_init() {
//     dec p: Point;
//     p.x = String { len: 10 };
//     consume_string(p.x);
// }
//
// fn test_piecewise_init_err() {
//     dec p: Point;
//     p.x = String { len: 10 };
//     consume_point(p); // Error: Use of uninitialized value
// }
