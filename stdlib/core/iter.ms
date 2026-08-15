import option;

export trait Iterator<T> {
    fn next(mut self: *Self) -> option::Option<T>;
}

export trait Iterable<T> {
    fn iter(self: *Self) -> *dyn Iterator<T>;
}
