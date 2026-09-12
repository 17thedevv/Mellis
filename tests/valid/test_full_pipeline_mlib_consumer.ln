use test_full_pipeline_mlib_producer::{Result, Process, handle_result};

struct SuccessData { val: int_32; }
struct ErrorData { code: int_32; }

impl Process for SuccessData {
    fn process(self) -> int_32 { return self.val; }
}

impl Process for ErrorData {
    fn process(self) -> int_32 { return self.code * -1; }
}

fn main() -> int_32 {
    dec ok_res: Result<SuccessData, ErrorData> = Result::Ok(SuccessData { val: 42 });
    dec err_res: Result<SuccessData, ErrorData> = Result::Err(ErrorData { code: 10 });
    
    dec a = handle_result(ok_res);
    dec b = handle_result(err_res);
    
    if a != 42 { return 1; }
    if b != -10 { return 2; }
    
    return 0;
}
