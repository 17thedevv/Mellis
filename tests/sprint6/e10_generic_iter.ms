enum Option {
    Some(int_32),
    None,
}

trait Iterator {
    type Item;
    fn next(self: &rw Self) -> Option;
}

struct Range {
    current: int_32;
    end: int_32;
}

impl Iterator for Range {
    type Item = int_32;

    fn next(self: &rw Range) -> Option {
        if self.current < self.end {
            dec val = self.current;
            self.current = self.current + 1;
            return Option::Some(val);
        }
        return Option::None;
    }
}

fn get_next<I: Iterator>(iter: &rw I) -> Option {
    return iter.next();
}

fn main() -> int_32 {
    dec iter = Range { current: 1, end: 3 };
    dec _dummy1 = (iter.current = 1);
    dec _dummy2 = (iter.end = 3);
    
    dec n1 = get_next@<Range>(&rw iter);
    
    dec _res = match n1 {
        Option::Some(val) -> {
            if val == 1 {
                return 0; // Success
            }
            return 1;
        },
        Option::None -> {
            return 1;
        }
    };


    
    return 1;
}
