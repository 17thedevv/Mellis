module std {
    struct Hidden {}
    export struct Top {}
    export module io {
        export struct File {}
    }
}

fn main() {
    dec t: std::Top = std::Top {};
    dec f: std::io::File = std::io::File {};
}
