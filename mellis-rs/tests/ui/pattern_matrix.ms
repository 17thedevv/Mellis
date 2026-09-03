// pattern_matrix.ms

module test {
    export struct Point {
        x: int,
        y: int,
    }

    export struct Nested {
        p: Point,
        z: int,
    }

    export enum Result {
        Ok(Point),
        Err(int),
    }

    export fn test_patterns(p: Nested, res: Result, tup: (int, bool)) -> int {
        dec rw score = 0;

        match tup {
            (10, true) -> { score = score + 1; }
            (_, false) -> { score = score + 2; }
            _ -> { score = score + 3; }
        };

        match p {
            Nested { p: Point { x: 5, y: _ }, z: 10 } -> { score = score + 4; }
            Nested { p, .. } -> { score = score + p.y; }
        };

        match res {
            Result::Ok(Point { x: 0, y: 0 }) -> { score = score + 10; }
            Result::Ok(p) -> { score = score + p.x; }
            Result::Err(e) -> { score = score - e; }
        };

        return score;
    }
}
