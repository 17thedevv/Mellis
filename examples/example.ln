use "std/net.mlib";
use "std/io.mlib";

// ==========================================
// 1. ANNOTATIONS (Siêu dữ liệu - @[...])
// Đánh dấu hàm này cần được Compiler tối ưu hóa (inline) 
// để gọi nhanh nhất có thể mà không tốn overhead chuyển ngữ cảnh.
// ==========================================
@[inline]
@[export(name = "log_system")]
// ==========================================
// 2. VARIADIC ARGUMENTS (Tham số biến số - ...any)
// Hàm log có thể nhận 1 prefix và VÔ SỐ tham số phía sau (giống printf)
// ==========================================
fn log_info(prefix: str, args: ...any) {
    io::print(prefix);
    
    // Trong thân hàm, `args` được đối xử như một Slice (Mảng)
    for (item in args) {
        io::print(" ");
        io::print(item);
    }
    io::print("\n");
}

// ==========================================
// 3. ASYNC / AWAIT (Hàm bất đồng bộ)
// Khai báo hàm với từ khóa `async`
// ==========================================
async fn fetch_player_data(id: int_32) -> Result<Player, str> {
    
    // Gọi hàm có variadic args: truyền 2 tham số hay 4 tham số đều hợp lệ!
    log_info("[FETCH]", "Đang lấy dữ liệu cho Player ID:", id, "từ server...");

    // Toán tử .await (đã có trong grammar của Sếp) kết hợp với toán tử ?
    // Đợi tải xong mạng, nếu lỗi mạng thì thoát hàm ném lỗi luôn.
    dec response = net::http_get("https://api.game.com/player").await?;

    return Result::Ok(Player::from_json(response));
}

// Hàm main của một hệ thống bất đồng bộ
async fn main() {
    log_info("[MAIN]", "Khởi động máy chủ", "Version:", 1.0, "Trạng thái:", "Sẵn sàng");
    
    dec player_req = fetch_player_data(101).await;
    
    match player_req {
        Result::Ok(p) -> log_info("[SUCCESS]", "Tên người chơi:", p.name),
        Result::Err(e) -> log_info("[ERROR]", "Tải thất bại. Mã lỗi:", e)
    }
}