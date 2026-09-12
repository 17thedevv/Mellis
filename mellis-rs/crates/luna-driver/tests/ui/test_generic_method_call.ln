trait Writer {
    fn write(self: &rw Self) -> i32;
}

struct FileWriter {}

impl Writer for FileWriter {
    fn write(self: &rw FileWriter) -> i32 {
        return 1;
    }
}

struct BadWriter {}

struct Box<T> {
    value: T
}

impl<T> Box<T> {
    fn write_to_box<W: Writer>(self: &rw Box<T>, writer: &rw W) -> i32 {
        return writer.write();
    }
}

fn main() {
    dec rw file = FileWriter {};
    dec rw bad = BadWriter {};
    dec rw b = Box<i32> { value: 42 };

    dec ok_sound = b.write_to_box(&rw file); // OK
    dec bad_sound = b.write_to_box(&rw bad); //~ ERROR The type `BadWriter` does not implement trait `Writer` (required by inferred generic parameter `W`)
}
