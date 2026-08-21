use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::IntPredicate;

fn build_workload_a<'ctx>(context: &'ctx Context, module: &Module<'ctx>, builder: &Builder<'ctx>) {
    let i32_type = context.i32_type();
    let fn_type = i32_type.fn_type(&[i32_type.into(), i32_type.into()], false);
    let function = module.add_function("workload_a", fn_type, None);
    let basic_block = context.append_basic_block(function, "entry");

    builder.position_at_end(basic_block);
    let param_a = function.get_nth_param(0).unwrap().into_int_value();
    let param_b = function.get_nth_param(1).unwrap().into_int_value();

    let add = builder.build_int_add(param_a, param_b, "add_res").unwrap();
    let sub = builder.build_int_sub(param_a, param_b, "sub_res").unwrap();
    let mul = builder.build_int_mul(add, sub, "mul_res").unwrap();

    let zero = i32_type.const_zero();
    let cmp = builder.build_int_compare(IntPredicate::SGT, mul, zero, "cmp_res").unwrap();
    let zext = builder.build_int_z_extend(cmp, i32_type, "zext_res").unwrap();

    builder.build_return(Some(&zext)).unwrap();
}

fn build_workload_b<'ctx>(context: &'ctx Context, module: &Module<'ctx>, builder: &Builder<'ctx>) {
    let i32_type = context.i32_type();
    let fn_type = i32_type.fn_type(&[i32_type.into()], false);
    let function = module.add_function("workload_b", fn_type, None);

    let entry = context.append_basic_block(function, "entry");
    let then_bb = context.append_basic_block(function, "then");
    let else_bb = context.append_basic_block(function, "else");
    let merge_bb = context.append_basic_block(function, "merge");

    builder.position_at_end(entry);
    let param_cond = function.get_nth_param(0).unwrap().into_int_value();
    let zero = i32_type.const_zero();
    let cmp = builder.build_int_compare(IntPredicate::NE, param_cond, zero, "cond").unwrap();
    builder.build_conditional_branch(cmp, then_bb, else_bb).unwrap();

    builder.position_at_end(then_bb);
    let val_then = i32_type.const_int(42, false);
    builder.build_unconditional_branch(merge_bb).unwrap();

    builder.position_at_end(else_bb);
    let val_else = i32_type.const_int(24, false);
    builder.build_unconditional_branch(merge_bb).unwrap();

    builder.position_at_end(merge_bb);
    let phi = builder.build_phi(i32_type, "phi_res").unwrap();
    phi.add_incoming(&[(&val_then, then_bb), (&val_else, else_bb)]);
    builder.build_return(Some(&phi.as_basic_value())).unwrap();
}

fn build_workload_c<'ctx>(context: &'ctx Context, module: &Module<'ctx>, builder: &Builder<'ctx>) {
    let void_type = context.void_type();
    let i32_type = context.i32_type();
    let ptr_type = context.ptr_type(inkwell::AddressSpace::default());

    let puts_type = i32_type.fn_type(&[ptr_type.into()], false);
    let puts_func = module.add_function("puts", puts_type, None);

    let fn_type = void_type.fn_type(&[], false);
    let function = module.add_function("workload_c", fn_type, None);
    let entry = context.append_basic_block(function, "entry");

    builder.position_at_end(entry);
    let alloca = builder.build_alloca(i32_type, "var").unwrap();
    let val = i32_type.const_int(100, false);
    builder.build_store(alloca, val).unwrap();

    let _load = builder.build_load(i32_type, alloca, "load_val").unwrap();

    builder.build_call(puts_func, &[alloca.into()], "call_res").unwrap();

    builder.build_return(None).unwrap();
}

fn main() {
    let context = Context::create();
    let module = context.create_module("spike_module");
    let builder = context.create_builder();

    build_workload_a(&context, &module, &builder);
    build_workload_b(&context, &module, &builder);
    build_workload_c(&context, &module, &builder);

    if let Err(err) = module.verify() {
        println!("Verifier Error: {}", err.to_string());
    } else {
        println!("inkwell: Module verified successfully!");
    }

    module.print_to_stderr();
}
