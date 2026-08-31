// Test: nested using with deep path alias
// Expected: no errors, proto::Request resolves

module application {
    export module network {
        export module protocol {
            export struct Request {
                id: i32,
            }
            export struct Response {
                code: i32,
            }
        }
    }
}

using application::network::protocol as proto;

fn main() -> i32 {
    dec r: proto::Request;
    dec s: proto::Response;
    return 0;
}
