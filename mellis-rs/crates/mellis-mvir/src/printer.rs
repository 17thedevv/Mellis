use crate::mvir::*;

pub fn print_module(module: &Module) -> String {
    let mut out = String::new();
    for func in &module.functions {
        out.push_str(&format!("fn {}() {{\n", func.name.name));
        for block in &func.blocks {
            out.push_str(&format!("{}:\n", block.label.name));
            for &val_id in &block.insts {
                let val_data = func.value(val_id);
                out.push_str(&format!("  %v{} = {}\n", val_id.0, print_instruction(&val_data.inst)));
            }
            if let Some(term) = &block.terminator {
                out.push_str(&format!("  {}\n", print_terminator(term)));
            } else {
                out.push_str("  <missing terminator>\n");
            }
        }
        out.push_str("}\n\n");
    }
    out
}

fn print_instruction(inst: &Instruction) -> String {
    match inst {
        Instruction::Alloca => "alloca".to_string(),
        Instruction::HeapAlloc => "heap_alloc".to_string(),
        Instruction::Assign(val) => format!("assign {}", print_operand(val)),
        Instruction::Store { ptr, value } => format!("store {} -> {}", print_operand(value), print_operand(ptr)),
        Instruction::Load { ptr } => format!("load {}", print_operand(ptr)),
        Instruction::Add { left, right } => format!("add {}, {}", print_operand(left), print_operand(right)),
        Instruction::Sub { left, right } => format!("sub {}, {}", print_operand(left), print_operand(right)),
        Instruction::Mul { left, right } => format!("mul {}, {}", print_operand(left), print_operand(right)),
        Instruction::Div { left, right } => format!("div {}, {}", print_operand(left), print_operand(right)),
        Instruction::Rem { left, right } => format!("rem {}, {}", print_operand(left), print_operand(right)),
        Instruction::Eq { left, right } => format!("eq {}, {}", print_operand(left), print_operand(right)),
        Instruction::NotEq { left, right } => format!("neq {}, {}", print_operand(left), print_operand(right)),
        Instruction::LessThan { left, right } => format!("lt {}, {}", print_operand(left), print_operand(right)),
        Instruction::LessOrEq { left, right } => format!("le {}, {}", print_operand(left), print_operand(right)),
        Instruction::GreaterThan { left, right } => format!("gt {}, {}", print_operand(left), print_operand(right)),
        Instruction::GreaterOrEq { left, right } => format!("ge {}, {}", print_operand(left), print_operand(right)),
        Instruction::BitAnd { left, right } => format!("bitand {}, {}", print_operand(left), print_operand(right)),
        Instruction::BitOr { left, right } => format!("bitor {}, {}", print_operand(left), print_operand(right)),
        Instruction::BitXor { left, right } => format!("bitxor {}, {}", print_operand(left), print_operand(right)),
        Instruction::Shl { left, right } => format!("shl {}, {}", print_operand(left), print_operand(right)),
        Instruction::Shr { left, right } => format!("shr {}, {}", print_operand(left), print_operand(right)),

        Instruction::CallDirect { callee, args } => {
            let mut arg_strs = Vec::new();
            for arg in args { arg_strs.push(print_operand(arg)); }
            format!("call_direct {}({})", callee.name, arg_strs.join(", "))
        }
        Instruction::CallIndirect { callee, args } => {
            let mut arg_strs = Vec::new();
            for arg in args { arg_strs.push(print_operand(arg)); }
            format!("call_indirect {}({})", print_operand(callee), arg_strs.join(", "))
        }
        Instruction::CallClosure { closure, args } => {
            let mut arg_strs = Vec::new();
            for arg in args { arg_strs.push(print_operand(arg)); }
            format!("call_closure {}({})", print_operand(closure), arg_strs.join(", "))
        }
        Instruction::CallIntrinsic { kind, args } => {
            let mut arg_strs = Vec::new();
            for arg in args { arg_strs.push(print_operand(arg)); }
            format!("call_intrinsic {:?}({})", kind, arg_strs.join(", "))
        }
        Instruction::MakeClosure { func, env_ptr, captures } => {
            let capture_text = captures.iter()
                .map(|capture| format!("sym{}:{}:{:?}", capture.symbol.0, capture.env_field, capture.mode))
                .collect::<Vec<_>>()
                .join(", ");
            format!("make_closure {}({}) [{}]", func.name, print_operand(env_ptr), capture_text)
        }
        Instruction::CallVirt { obj, method_idx, args } => {
            let mut arg_strs = Vec::new();
            for arg in args { arg_strs.push(print_operand(arg)); }
            format!("call_virt {}.{}({})", print_operand(obj), method_idx, arg_strs.join(", "))
        }
        Instruction::MakeTraitObject { data_ptr, vtable, .. } => {
            format!("make_trait_object {{ data: {}, vtable: @{} }}", print_operand(data_ptr), vtable.name)
        }
        Instruction::MakeSlice { data_ptr, len } => {
            format!("make_slice {{ data: {}, len: {} }}", print_operand(data_ptr), print_operand(len))
        }
        Instruction::DropVirt { obj } => {
            format!("drop_virt {}", print_operand(obj))
        }
        Instruction::BoundsCheck { index, len } => {
            format!("bounds_check {} < {}", print_operand(index), print_operand(len))
        }
        Instruction::Borrow { is_rw, base } => {
            let kw = if *is_rw { "rw " } else { "" };
            format!("borrow &{}{}", kw, print_operand(base))
        }
        Instruction::Variant { enum_ty, variant_idx, args } => {
            let mut arg_strs = Vec::new();
            for arg in args { arg_strs.push(print_operand(arg)); }
            format!("variant {}::{}({})", enum_ty.0, variant_idx, arg_strs.join(", "))
        }
        Instruction::Tag { value } => format!("tag {}", print_operand(value)),
        Instruction::Extract { value, variant_idx, field_idx } => {
            format!("extract {}.{}.{}", print_operand(value), variant_idx, field_idx)
        }
        Instruction::FieldPtr { base, field_idx } => {
            format!("field_ptr {}, {}", print_operand(base), field_idx)
        }
        Instruction::BoxNew { value } => format!("box_new {}", print_operand(value)),
        Instruction::MarkInit { value } => format!("mark_init {}", print_operand(value)),
        Instruction::BoxFree { value } => format!("box_free {}", print_operand(value)),
        Instruction::Drop { value, callee, .. } => {
            if let Some(c) = callee {
                format!("drop {} ({})", print_operand(value), c.name)
            } else {
                format!("drop {}", print_operand(value))
            }
        },
        Instruction::PtrOffset { ptr, offset } => format!("ptr_offset {}, {}", print_operand(ptr), print_operand(offset)),
        Instruction::Cast { value, target_ty } => format!("cast {}, {:?}", print_operand(value), target_ty),
        Instruction::Null { ty } => format!("null {:?}", ty),
        Instruction::SizeOf { ty } => format!("size_of {:?}", ty),
        Instruction::AlignOf { ty } => format!("align_of {:?}", ty),
        Instruction::Await { future } => format!("await {}", print_operand(future)),
        Instruction::Nop => "nop".to_string(),
    }
}

fn print_terminator(term: &Terminator) -> String {
    match term {
        Terminator::Ret { value } => {
            if let Some(val) = value {
                format!("ret {}", print_operand(val))
            } else {
                "ret void".to_string()
            }
        }
        Terminator::Br { target } => format!("br {}", target.name),
        Terminator::CondBr { condition, true_target, false_target } => {
            format!("condbr {}, {}, {}", print_operand(condition), true_target.name, false_target.name)
        }
        Terminator::Unreachable => "unreachable".to_string(),
        Terminator::MissingReturn => "missing_return".to_string(),
    }
}

fn print_operand(op: &Operand) -> String {
    match op {
        Operand::Value(val_id) => format!("%v{}", val_id.0),
        Operand::Global(glb) => glb.name.clone(),
        Operand::Block(blk_id) => format!("block_{}", blk_id.0),
        Operand::Number(n) => n.clone(),
        Operand::Boolean(b) => b.to_string(),
        Operand::StringRef(s) => format!("\"{}\"", s),
        Operand::Char(c) => format!("'{}'", c),
    }
}
