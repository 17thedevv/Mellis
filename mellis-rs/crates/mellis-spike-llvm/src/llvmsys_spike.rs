use std::ffi::CString;
use llvm_sys::core::*;
use llvm_sys::prelude::*;
use llvm_sys::LLVMIntPredicate;
use std::ptr;

fn c_str(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn build_workload_a(module: LLVMModuleRef, builder: LLVMBuilderRef, context: LLVMContextRef) {
    unsafe {
        let i32_type = LLVMInt32TypeInContext(context);
        let param_types = &mut [i32_type, i32_type];
        let func_type = LLVMFunctionType(i32_type, param_types.as_mut_ptr(), 2, 0);
        let func_name = c_str("workload_a");
        let func = LLVMAddFunction(module, func_name.as_ptr(), func_type);

        let entry = LLVMAppendBasicBlockInContext(context, func, c_str("entry").as_ptr());
        LLVMPositionBuilderAtEnd(builder, entry);

        let param_a = LLVMGetParam(func, 0);
        let param_b = LLVMGetParam(func, 1);

        let add = LLVMBuildAdd(builder, param_a, param_b, c_str("add_res").as_ptr());
        let sub = LLVMBuildSub(builder, param_a, param_b, c_str("sub_res").as_ptr());
        let mul = LLVMBuildMul(builder, add, sub, c_str("mul_res").as_ptr());

        // compare
        let cmp = LLVMBuildICmp(builder, LLVMIntPredicate::LLVMIntSGT, mul, LLVMConstInt(i32_type, 0, 0), c_str("cmp_res").as_ptr());
        // zext cmp to i32
        let zext = LLVMBuildZExt(builder, cmp, i32_type, c_str("zext_res").as_ptr());

        LLVMBuildRet(builder, zext);
    }
}

fn build_workload_b(module: LLVMModuleRef, builder: LLVMBuilderRef, context: LLVMContextRef) {
    unsafe {
        let i32_type = LLVMInt32TypeInContext(context);
        let param_types = &mut [i32_type];
        let func_type = LLVMFunctionType(i32_type, param_types.as_mut_ptr(), 1, 0);
        let func_name = c_str("workload_b");
        let func = LLVMAddFunction(module, func_name.as_ptr(), func_type);

        let entry = LLVMAppendBasicBlockInContext(context, func, c_str("entry").as_ptr());
        let then_bb = LLVMAppendBasicBlockInContext(context, func, c_str("then").as_ptr());
        let else_bb = LLVMAppendBasicBlockInContext(context, func, c_str("else").as_ptr());
        let merge_bb = LLVMAppendBasicBlockInContext(context, func, c_str("merge").as_ptr());

        LLVMPositionBuilderAtEnd(builder, entry);
        let param_cond = LLVMGetParam(func, 0);
        let cmp = LLVMBuildICmp(builder, LLVMIntPredicate::LLVMIntNE, param_cond, LLVMConstInt(i32_type, 0, 0), c_str("cond").as_ptr());
        LLVMBuildCondBr(builder, cmp, then_bb, else_bb);

        LLVMPositionBuilderAtEnd(builder, then_bb);
        let val_then = LLVMConstInt(i32_type, 42, 0);
        LLVMBuildBr(builder, merge_bb);

        LLVMPositionBuilderAtEnd(builder, else_bb);
        let val_else = LLVMConstInt(i32_type, 24, 0);
        LLVMBuildBr(builder, merge_bb);

        LLVMPositionBuilderAtEnd(builder, merge_bb);
        let phi = LLVMBuildPhi(builder, i32_type, c_str("phi_res").as_ptr());
        let mut phi_vals = [val_then, val_else];
        let mut phi_blocks = [then_bb, else_bb];
        LLVMAddIncoming(phi, phi_vals.as_mut_ptr(), phi_blocks.as_mut_ptr(), 2);

        LLVMBuildRet(builder, phi);
    }
}

fn build_workload_c(module: LLVMModuleRef, builder: LLVMBuilderRef, context: LLVMContextRef) {
    unsafe {
        let void_type = LLVMVoidTypeInContext(context);
        let i32_type = LLVMInt32TypeInContext(context);
        let ptr_type = LLVMPointerType(i32_type, 0); // Opaque pointer in LLVM 15+

        // declare puts
        let puts_param_types = &mut [ptr_type];
        let puts_type = LLVMFunctionType(i32_type, puts_param_types.as_mut_ptr(), 1, 0);
        let puts_func = LLVMAddFunction(module, c_str("puts").as_ptr(), puts_type);

        let func_type = LLVMFunctionType(void_type, ptr::null_mut(), 0, 0);
        let func = LLVMAddFunction(module, c_str("workload_c").as_ptr(), func_type);

        let entry = LLVMAppendBasicBlockInContext(context, func, c_str("entry").as_ptr());
        LLVMPositionBuilderAtEnd(builder, entry);

        let alloca = LLVMBuildAlloca(builder, i32_type, c_str("var").as_ptr());
        let val = LLVMConstInt(i32_type, 100, 0);
        LLVMBuildStore(builder, val, alloca);

        let load = LLVMBuildLoad2(builder, i32_type, alloca, c_str("load_val").as_ptr());

        // For simplicity, we just pass alloca (ptr) to puts
        let mut args = [alloca];
        LLVMBuildCall2(builder, puts_type, puts_func, args.as_mut_ptr(), 1, c_str("call_res").as_ptr());

        LLVMBuildRetVoid(builder);
    }
}

fn main() {
    unsafe {
        let context = LLVMContextCreate();
        let module = LLVMModuleCreateWithNameInContext(c_str("spike_module").as_ptr(), context);
        let builder = LLVMCreateBuilderInContext(context);

        build_workload_a(module, builder, context);
        build_workload_b(module, builder, context);
        build_workload_c(module, builder, context);

        use llvm_sys::analysis::{LLVMVerifyModule, LLVMVerifierFailureAction};
        let mut error_msg: *mut libc::c_char = ptr::null_mut();
        
        let has_error = LLVMVerifyModule(module, LLVMVerifierFailureAction::LLVMReturnStatusAction, &mut error_msg);
        if has_error == 1 {
            let c_str = std::ffi::CStr::from_ptr(error_msg);
            println!("Verifier Error: {:?}", c_str);
            LLVMDisposeMessage(error_msg);
        } else {
            println!("llvm-sys: Module verified successfully!");
        }

        LLVMDumpModule(module);

        LLVMDisposeBuilder(builder);
        LLVMDisposeModule(module);
        LLVMContextDispose(context);
    }
}
