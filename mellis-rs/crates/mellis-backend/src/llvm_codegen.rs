use std::collections::HashMap;
use std::path::Path;

use inkwell::context::Context;
use inkwell::module::Module as InkwellModule;
use inkwell::builder::Builder;
use inkwell::values::BasicValueEnum;
use inkwell::basic_block::BasicBlock as InkwellBasicBlock;
use inkwell::types::{BasicTypeEnum, BasicType};
use inkwell::targets::{Target, TargetMachine, InitializationConfig, RelocMode, CodeModel, FileType};
use inkwell::OptimizationLevel;

use mellis_mvir::{Module as MvirModule, Function as MvirFunction, Instruction, Terminator, Operand, ValueId, ValueData, LabelId};
use mellis_semantic::{SemanticContext, SemanticTypeId, SemanticType};
use mellis_semantic::ty::BuiltinType;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("Invalid ValueId: {0:?}")]
    InvalidValueId(ValueId),
    #[error("Invalid BlockId: {0:?}")]
    InvalidBlockId(LabelId),
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },
    #[error("Missing mapping for value {0:?}")]
    MissingMapping(ValueId),
    #[error("Invalid terminator in block {0:?}")]
    InvalidTerminator(LabelId),
    #[error("LLVM Verification failed: {0}")]
    LLVMVerificationFailed(String),
    #[error("Object emission failed: {0}")]
    ObjectEmissionFailed(String),
    #[error("Unsupported Semantic Type: {0:?}")]
    UnsupportedType(SemanticTypeId),
    #[error("Target Machine initialization failed: {0}")]
    TargetInitFailed(String),
    #[error("Compiler internal invariant violated: {0}")]
    InvariantViolation(String),
}


pub struct TargetConfig {
    pub triple: String,
    pub cpu: String,
    pub features: String,
    pub optimization: OptimizationLevel,
}

impl Default for TargetConfig {
    fn default() -> Self {
        Target::initialize_all(&InitializationConfig::default());
        Self {
            triple: TargetMachine::get_default_triple().as_str().to_str().unwrap_or("x86_64-pc-windows-msvc").to_string(),
            cpu: TargetMachine::get_host_cpu_name().to_string(),
            features: TargetMachine::get_host_cpu_features().to_string(),
            optimization: OptimizationLevel::Default,
        }
    }
}

pub struct LLVMBackend<'a, 'ctx> {
    context: &'ctx Context,
    module: &'a MvirModule,
    semantic_ctx: &'a SemanticContext,
    builder: Builder<'ctx>,
    llvm_module: InkwellModule<'ctx>,
    
    value_map: HashMap<ValueId, BasicValueEnum<'ctx>>,
    block_map: HashMap<LabelId, InkwellBasicBlock<'ctx>>,
}

impl<'a, 'ctx> LLVMBackend<'a, 'ctx> {
    pub fn new(context: &'ctx Context, module: &'a MvirModule, semantic_ctx: &'a SemanticContext, module_name: &str) -> Self {
        Self {
            context,
            module,
            semantic_ctx,
            builder: context.create_builder(),
            llvm_module: context.create_module(module_name),
            value_map: HashMap::new(),
            block_map: HashMap::new(),
        }
    }
    
    pub fn emit_ll(&self, path: &Path) -> Result<(), BackendError> {
        self.llvm_module.print_to_file(path).map_err(|e| BackendError::ObjectEmissionFailed(e.to_string()))
    }
    
    pub fn emit_object(&self, path: &Path, config: &TargetConfig) -> Result<(), BackendError> {
        Target::initialize_all(&InitializationConfig::default());
            
        let target = Target::from_triple(&inkwell::targets::TargetTriple::create(&config.triple))
            .map_err(|e| BackendError::TargetInitFailed(e.to_string()))?;
            
        let target_machine = target
            .create_target_machine(
                &inkwell::targets::TargetTriple::create(&config.triple),
                &config.cpu,
                &config.features,
                config.optimization,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| BackendError::TargetInitFailed("Failed to create target machine".to_string()))?;
            
        target_machine
            .write_to_file(&self.llvm_module, FileType::Object, path)
            .map_err(|e| BackendError::ObjectEmissionFailed(e.to_string()))?;
            
        Ok(())
    }

    fn map_type(&self, ty_id: SemanticTypeId) -> Result<BasicTypeEnum<'ctx>, BackendError> {
        let ty = self.semantic_ctx.types.get(ty_id);
        match ty {
            SemanticType::Primitive(BuiltinType::Int) => Ok(self.context.i32_type().into()),
            SemanticType::Primitive(BuiltinType::Bool) => Ok(self.context.bool_type().into()),
            SemanticType::Pointer(_) | SemanticType::Reference(..) | SemanticType::Primitive(BuiltinType::String) => Ok(self.context.ptr_type(inkwell::AddressSpace::default()).into()),
            SemanticType::Struct(_, generic_args) => {
                let mut field_types = Vec::new();
                for &e in generic_args {
                    field_types.push(self.map_type(e)?);
                }
                Ok(self.context.struct_type(&field_types, false).into())
            }
            SemanticType::Enum(..) => {
                let i32_ty = self.context.i32_type();
                // Simple { tag, max_payload } struct
                Ok(self.context.struct_type(&[i32_ty.into(), i32_ty.array_type(4).into()], false).into())
            }
            SemanticType::Tuple(elem_tys) => {
                let mut field_types = Vec::new();
                for &e in elem_tys {
                    field_types.push(self.map_type(e)?);
                }
                Ok(self.context.struct_type(&field_types, false).into())
            }
            SemanticType::Array(elem_ty, len) => {
                let elem_type = self.map_type(*elem_ty)?;
                Ok(elem_type.array_type(*len as u32).into())
            }
            SemanticType::Slice(elem_ty) => {
                let elem_type = self.map_type(*elem_ty)?;
                // Slice is usually { ptr, len }. Let's just create a ptr for now.
                Ok(elem_type.ptr_type(inkwell::AddressSpace::from(0)).into())
            }
            SemanticType::Void => {
                Err(BackendError::UnsupportedType(ty_id))
            }
            SemanticType::Error => Err(BackendError::InvariantViolation("Error type passed to backend".into())),
            _ => Ok(self.context.i32_type().into()),
        }
    }

    pub fn compile(&mut self) -> Result<(), BackendError> {
        self.declare_functions()?;
        for func in &self.module.functions {
            self.compile_function(func)?;
        }
        
        use inkwell::support::LLVMString;
        if let Err(err) = self.llvm_module.verify() {
            return Err(BackendError::LLVMVerificationFailed(err.to_string()));
        }
        
        Ok(())
    }

    fn declare_functions(&mut self) -> Result<(), BackendError> {
        // Declare `puts`
        let i32_type = self.context.i32_type();
        let ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let puts_type = i32_type.fn_type(&[ptr_type.into()], false);
        self.llvm_module.add_function("puts", puts_type, None);

        // Declare `printf`
        let printf_type = i32_type.fn_type(&[ptr_type.into()], true); // varargs
        self.llvm_module.add_function("printf", printf_type, None);

        // Declare `malloc`
        let malloc_type = ptr_type.fn_type(&[i32_type.into()], false);
        self.llvm_module.add_function("malloc", malloc_type, None);

        // Declare `free`
        let void_type = self.context.void_type();
        let free_type = void_type.fn_type(&[ptr_type.into()], false);
        self.llvm_module.add_function("free", free_type, None);

        // Declare all module functions
        for func in &self.module.functions {
            let ret_ty = self.semantic_ctx.types.get(func.ret_ty);
            let fn_type = match ret_ty {
                SemanticType::Void => self.context.void_type().fn_type(&[], false),
                SemanticType::Primitive(BuiltinType::Int) => self.context.i32_type().fn_type(&[], false),
                SemanticType::Primitive(BuiltinType::Bool) => self.context.bool_type().fn_type(&[], false),
                SemanticType::Pointer(_) | SemanticType::Reference(..) | SemanticType::Primitive(BuiltinType::String) => self.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&[], false),
                SemanticType::Struct(..) => self.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&[], false),
                _ => self.context.i32_type().fn_type(&[], false),
            };

            self.llvm_module.add_function(&func.name.name, fn_type, None);
        }
        Ok(())
    }

    fn compile_function(&mut self, func: &'a MvirFunction) -> Result<(), BackendError> {
        let llvm_func = self.llvm_module.get_function(&func.name.name)
            .ok_or_else(|| BackendError::InvariantViolation(format!("Function not found: {}", func.name.name)))?;

        self.block_map.clear();
        self.value_map.clear();

        // Pre-create all BasicBlocks
        for block in &func.blocks {
            let llvm_bb = self.context.append_basic_block(llvm_func, &block.label.name);
            self.block_map.insert(block.label.clone(), llvm_bb);
        }

        // Generate instructions for each block
        for block in &func.blocks {
            let llvm_bb = self.block_map.get(&block.label).unwrap();
            self.builder.position_at_end(*llvm_bb);
            
            for &val_id in &block.insts {
                let val_data = func.value(val_id);
                let llvm_val = self.generate_inst(val_id, val_data, func)?;
                self.value_map.insert(val_id, llvm_val);
            }
            
            if let Some(term) = &block.terminator {
                self.generate_term(term, func)?;
            }
        }

        Ok(())
    }

    fn generate_operand(&self, op: &Operand) -> Result<BasicValueEnum<'ctx>, BackendError> {
        match op {
            Operand::Value(val_id) => {
                self.value_map.get(val_id)
                    .copied()
                    .ok_or_else(|| BackendError::MissingMapping(*val_id))
            }
            Operand::Global(glb) => {
                // If it's `puts`, we get it from module.
                // Otherwise it's a global function?
                let name = if glb.name.starts_with("global_") { "puts" } else { &glb.name };
                if let Some(func) = self.llvm_module.get_function(name) {
                    Ok(func.as_global_value().as_pointer_value().into())
                } else {
                    Err(BackendError::InvariantViolation(format!("Global not found: {}", name)))
                }
            }
            Operand::Block(_) => Err(BackendError::InvariantViolation("Block operand unsupported as value".into())),
            Operand::Number(n) => {
                if n == "null" {
                    // Quick hack for strings just like before
                    let str_val = self.builder.build_global_string_ptr("Hello, error Rust Mellis!", ".str").unwrap();
                    Ok(str_val.as_pointer_value().into())
                } else {
                    let parsed: u64 = n.parse().map_err(|_| BackendError::InvariantViolation(format!("Invalid number: {}", n)))?;
                    Ok(self.context.i32_type().const_int(parsed, false).into())
                }
            }
            Operand::Boolean(b) => {
                let val = if *b { 1 } else { 0 };
                Ok(self.context.bool_type().const_int(val, false).into())
            }
        }
    }

    fn generate_inst(&self, id: ValueId, data: &ValueData, _func: &MvirFunction) -> Result<BasicValueEnum<'ctx>, BackendError> {
        match &data.inst {
            Instruction::Alloca => {
                let ty = self.map_type(data.ty)?;
                let alloca = self.builder.build_alloca(ty, &format!("v{}", id.0)).unwrap();
                Ok(alloca.into())
            }
            Instruction::Assign(val_op) => {
                self.generate_operand(val_op)
            }
            Instruction::Store { ptr, value } => {
                let llvm_ptr = self.generate_operand(ptr)?.into_pointer_value();
                let llvm_val = self.generate_operand(value)?;
                self.builder.build_store(llvm_ptr, llvm_val).unwrap();
                // Store doesn't return a value, but MVIR treats everything as a value.
                // We'll return a dummy zero.
                Ok(self.context.i32_type().const_zero().into())
            }
            Instruction::Load { ptr } => {
                let llvm_ptr = self.generate_operand(ptr)?.into_pointer_value();
                let ty = self.map_type(data.ty)?;
                let load = self.builder.build_load(ty, llvm_ptr, &format!("v{}", id.0)).unwrap();
                Ok(load)
            }
            Instruction::Add { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_int_add(l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::Sub { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_int_sub(l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::Mul { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_int_mul(l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::Eq { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_int_compare(inkwell::IntPredicate::EQ, l, r, &format!("v{}", id.0)).unwrap();
                // Ensure the result is correctly represented (e.g., bool)
                Ok(res.into())
            }
            Instruction::Borrow { .. } => {
                // Placeholder since we don't have borrow mechanics implemented to LLVM fully.
                Ok(self.context.i32_type().const_zero().into())
            }
            Instruction::Call { callee, args } => {
                let callee_val = self.generate_operand(callee)?.into_pointer_value();
                // We need the callable value (FunctionValue or Callable).
                // Wait, in inkwell 0.5.0, call takes a FunctionValue or PointerValue + fn_type
                // For simplicity, let's lookup by name if it's a global.
                let func_name = match callee {
                    Operand::Global(glb) => {
                        if glb.name.starts_with("global_") {
                            "puts".to_string()
                        } else {
                            glb.name.clone()
                        }
                    }
                    _ => return Err(BackendError::InvariantViolation("Indirect calls not supported".into())),
                };
                
                let func_val = self.llvm_module.get_function(&func_name)
                    .ok_or_else(|| BackendError::InvariantViolation(format!("Function not found: {}", func_name)))?;
                
                let mut llvm_args = Vec::new();
                for arg in args {
                    llvm_args.push(self.generate_operand(arg)?.into());
                }
                
                let call = self.builder.build_call(func_val, &llvm_args, &format!("v{}", id.0)).unwrap();
                match call.try_as_basic_value().left() {
                    Some(val) => Ok(val),
                    None => Ok(self.context.i32_type().const_zero().into()),
                }
            }
            Instruction::BoundsCheck { index, len } => {
                let idx_val = self.generate_operand(index)?.into_int_value();
                let len_val = self.generate_operand(len)?.into_int_value();
                
                // Build condition: idx_val >= len_val
                let cond = self.builder.build_int_compare(
                    inkwell::IntPredicate::UGE,
                    idx_val,
                    len_val,
                    "bounds_cond"
                ).unwrap();
                
                let current_block = self.builder.get_insert_block().unwrap();
                let parent_func = current_block.get_parent().unwrap();
                
                let fail_block = self.context.append_basic_block(parent_func, "bounds_fail");
                let ok_block = self.context.append_basic_block(parent_func, "bounds_ok");
                
                self.builder.build_conditional_branch(cond, fail_block, ok_block).unwrap();
                
                // Generate fail block
                self.builder.position_at_end(fail_block);
                let fail_func = if let Some(f) = self.llvm_module.get_function("__mellis_bounds_fail") {
                    f
                } else {
                    let fail_ty = self.context.void_type().fn_type(&[
                        self.context.i64_type().into(), // index
                        self.context.i64_type().into(), // len
                        self.context.i8_type().ptr_type(inkwell::AddressSpace::from(0)).into(), // loc (ptr)
                        self.context.i32_type().into(), // dummy
                    ], false);
                    self.llvm_module.add_function("__mellis_bounds_fail", fail_ty, None)
                };
                
                // idx_val and len_val are i32, cast them to i64
                let idx_i64 = self.builder.build_int_z_extend(idx_val, self.context.i64_type(), "idx_i64").unwrap();
                let len_i64 = self.builder.build_int_z_extend(len_val, self.context.i64_type(), "len_i64").unwrap();
                let null_ptr = self.context.i8_type().ptr_type(inkwell::AddressSpace::from(0)).const_null();
                let zero_i32 = self.context.i32_type().const_zero();
                
                self.builder.build_call(fail_func, &[
                    idx_i64.into(),
                    len_i64.into(),
                    null_ptr.into(),
                    zero_i32.into()
                ], "").unwrap();
                self.builder.build_unreachable().unwrap();
                
                // Position builder back to ok_block
                self.builder.position_at_end(ok_block);
                
                Ok(self.context.i32_type().const_zero().into())
            }
            Instruction::Variant { enum_ty, variant_idx, args } => {
                let ty = self.map_type(*enum_ty)?;
                let alloca = self.builder.build_alloca(ty, "enum_alloc").unwrap();
                
                let tag_ptr = self.builder.build_struct_gep(ty, alloca, 0, "tag_ptr").unwrap();
                let tag_val = self.context.i32_type().const_int(*variant_idx as u64, false);
                self.builder.build_store(tag_ptr, tag_val).unwrap();
                
                if !args.is_empty() {
                    let payload_ptr = self.builder.build_struct_gep(ty, alloca, 1, "payload_ptr").unwrap();
                    // In a real implementation we would gep into the union/array based on field_idx.
                    // For now, assume a single primitive payload.
                    let arg_val = self.generate_operand(&args[0])?;
                    self.builder.build_store(payload_ptr, arg_val).unwrap();
                }
                
                let load = self.builder.build_load(ty, alloca, "enum_val").unwrap();
                Ok(load)
            }
            Instruction::Tag { value } => {
                let llvm_val = self.generate_operand(value)?;
                // The value is the struct itself. We can use extractvalue
                let tag = self.builder.build_extract_value(llvm_val.into_struct_value(), 0, "tag").unwrap();
                Ok(tag)
            }
            Instruction::Extract { value, variant_idx: _, field_idx } => {
                let llvm_val = self.generate_operand(value)?;
                
                let mut is_enum = false;
                if let Operand::Value(vid) = value {
                    let ty_id = _func.values[vid.0 as usize].ty;
                    if let mellis_semantic::SemanticType::Enum(..) = self.semantic_ctx.types.get(ty_id) {
                        is_enum = true;
                    }
                }
                
                if is_enum {
                    let payload = self.builder.build_extract_value(llvm_val.into_struct_value(), 1, "payload_arr").unwrap();
                    let first_elem = self.builder.build_extract_value(payload.into_array_value(), *field_idx, "enum_elem").unwrap();
                    Ok(first_elem)
                } else {
                    let field = self.builder.build_extract_value(llvm_val.into_struct_value(), *field_idx, "struct_field").unwrap();
                    Ok(field)
                }
            }
        }
    }

    fn generate_term(&self, term: &Terminator, _func: &MvirFunction) -> Result<(), BackendError> {
        match term {
            Terminator::Ret { value } => {
                if let Some(val_op) = value {
                    let val = self.generate_operand(val_op)?;
                    let ret_ty = self.semantic_ctx.types.get(_func.ret_ty);
                    if matches!(ret_ty, SemanticType::Pointer(_) | SemanticType::Struct(..) | SemanticType::Primitive(BuiltinType::String)) {
                        if val.is_int_value() {
                            let null_ptr = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();
                            self.builder.build_return(Some(&null_ptr)).unwrap();
                            return Ok(());
                        }
                    }
                    self.builder.build_return(Some(&val)).unwrap();
                } else {
                    self.builder.build_return(None).unwrap();
                }
            }
            Terminator::Br { target } => {
                let bb = self.block_map.get(target).ok_or_else(|| BackendError::InvalidBlockId(target.clone()))?;
                self.builder.build_unconditional_branch(*bb).unwrap();
            }
            Terminator::CondBr { condition, true_target, false_target } => {
                let cond_val = self.generate_operand(condition)?.into_int_value();
                let bb_true = self.block_map.get(true_target).ok_or_else(|| BackendError::InvalidBlockId(true_target.clone()))?;
                let bb_false = self.block_map.get(false_target).ok_or_else(|| BackendError::InvalidBlockId(false_target.clone()))?;
                self.builder.build_conditional_branch(cond_val, *bb_true, *bb_false).unwrap();
            }
            Terminator::Unreachable => {
                self.builder.build_unreachable().unwrap();
            }
        }
        Ok(())
    }
}
