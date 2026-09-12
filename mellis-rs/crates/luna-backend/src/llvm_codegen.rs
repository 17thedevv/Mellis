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

use luna_mvir::{Module as MvirModule, Function as MvirFunction, Instruction, Terminator, Operand, ValueId, ValueData, LabelId};
use luna_semantic::{SemanticContext, SemanticTypeId, SemanticType};
use luna_semantic::ty::BuiltinType;
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
        let mut triple = TargetMachine::get_default_triple().as_str().to_str().unwrap_or("x86_64-pc-windows-gnu").to_string();
        if triple.contains("-msvc") {
            triple = triple.replace("-msvc", "-gnu");
        }
        Self {
            triple,
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
    function_map: HashMap<String, inkwell::values::FunctionValue<'ctx>>,
    link_name_map: HashMap<String, String>,
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
            function_map: HashMap::new(),
            link_name_map: HashMap::new(),
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

    fn get_function(&self, name: &str) -> Option<inkwell::values::FunctionValue<'ctx>> {
        let actual_name = self.link_name_map.get(name).map(|s| s.as_str()).unwrap_or(name);
        self.llvm_module.get_function(actual_name)
    }

    fn layout_size(&self, ty_id: SemanticTypeId) -> u64 {
        match self.map_type(ty_id) {
            Ok(ty) => ty.size_of().map(|size| size.get_zero_extended_constant().unwrap_or(8)).unwrap_or(8),
            Err(_) => 8,
        }
    }

    fn layout_align(&self, ty_id: SemanticTypeId) -> u64 {
        match self.semantic_ctx.types.get(ty_id) {
            SemanticType::Primitive(BuiltinType::I8 | BuiltinType::U8 | BuiltinType::Bool) => 1,
            SemanticType::Primitive(BuiltinType::I16 | BuiltinType::U16) => 2,
            SemanticType::Primitive(BuiltinType::I32 | BuiltinType::U32 | BuiltinType::F32 | BuiltinType::Char) => 4,
            SemanticType::Primitive(BuiltinType::I64 | BuiltinType::U64 | BuiltinType::Isize | BuiltinType::Usize | BuiltinType::F64 | BuiltinType::String) => 8,
            SemanticType::Primitive(BuiltinType::I128 | BuiltinType::U128) => 16,
            SemanticType::Pointer(..) | SemanticType::Reference(..) | SemanticType::Box(..) => 8,
            SemanticType::Struct(_, _, field_tys) => {
                field_tys.iter().map(|&f| self.layout_align(f)).max().unwrap_or(8)
            }
            _ => 8,
        }
    }

    fn map_type(&self, ty_id: SemanticTypeId) -> Result<BasicTypeEnum<'ctx>, BackendError> {
        let ty = self.semantic_ctx.types.get(ty_id);
        match ty {
            SemanticType::Primitive(BuiltinType::I8) | SemanticType::Primitive(BuiltinType::U8) => Ok(self.context.i8_type().into()),
            SemanticType::Primitive(BuiltinType::I16) | SemanticType::Primitive(BuiltinType::U16) => Ok(self.context.i16_type().into()),
            SemanticType::Primitive(BuiltinType::I32) | SemanticType::Primitive(BuiltinType::U32) => Ok(self.context.i32_type().into()),
            SemanticType::Primitive(BuiltinType::I64) | SemanticType::Primitive(BuiltinType::U64) | SemanticType::Primitive(BuiltinType::Isize) | SemanticType::Primitive(BuiltinType::Usize) => Ok(self.context.i64_type().into()),
            SemanticType::Primitive(BuiltinType::I128) | SemanticType::Primitive(BuiltinType::U128) => Ok(self.context.i128_type().into()),
            SemanticType::Primitive(BuiltinType::F32) => Ok(self.context.f32_type().into()),
            SemanticType::Primitive(BuiltinType::F64) => Ok(self.context.f64_type().into()),
            SemanticType::Primitive(BuiltinType::Bool) => Ok(self.context.bool_type().into()),
            SemanticType::Primitive(BuiltinType::Char) => Ok(self.context.i32_type().into()),
            SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                let inner_ty = self.semantic_ctx.types.get(*inner);
                if let SemanticType::DynTrait(_) = inner_ty {
                    let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                    Ok(self.context.struct_type(&[ptr_ty.into(), ptr_ty.into()], false).into())
                } else if let SemanticType::Slice(_) = inner_ty {
                    let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                    let len_ty = self.context.i64_type();
                    Ok(self.context.struct_type(&[ptr_ty.into(), len_ty.into()], false).into())
                } else {
                    Ok(self.context.ptr_type(inkwell::AddressSpace::default()).into())
                }
            }
            SemanticType::DynTrait(_) => {
                let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                Ok(self.context.struct_type(&[ptr_ty.into(), ptr_ty.into()], false).into())
            }
            SemanticType::Primitive(BuiltinType::String) => Ok(self.context.ptr_type(inkwell::AddressSpace::default()).into()),
            SemanticType::Struct(_, _, field_tys) => {
                let mut field_types = Vec::new();
                for &e in field_tys {
                    field_types.push(self.map_type(e)?);
                }
                Ok(self.context.struct_type(&field_types, false).into())
            }
            SemanticType::Enum(..) => {
                let i32_ty = self.context.i32_type();
                // Simple { tag, _, max_payload } struct
                Ok(self.context.struct_type(&[i32_ty.into(), i32_ty.array_type(4).into()], false).into())
            }
            SemanticType::Tuple(elem_tys) => {
                let mut field_types = Vec::new();
                for &e in elem_tys {
                    field_types.push(self.map_type(e)?);
                }
                Ok(self.context.struct_type(&field_types, false).into())
            }
            SemanticType::Closure(..) => {
                let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                Ok(self.context.struct_type(&[ptr_ty.into(), ptr_ty.into()], false).into())
            }

            SemanticType::Array(elem_ty, len) => {
                let elem_type = self.map_type(*elem_ty)?;
                Ok(elem_type.array_type(*len as u32).into())
            }
            SemanticType::Slice(_) => {
                let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                let len_ty = self.context.i64_type();
                Ok(self.context.struct_type(&[ptr_ty.into(), len_ty.into()], false).into())
            }
            SemanticType::Future(inner) => {
                let i32_ty = self.context.i32_type();
                let inner_ty = self.map_type(*inner)?;
                Ok(self.context.struct_type(&[i32_ty.into(), inner_ty], false).into())
            }
            SemanticType::Void => {
                // Map Void (SemanticTypeId(0)) to an empty struct {} instead of panicking.
                // This allows it to be safely embedded in structs, allocas, and futures.
                Ok(self.context.struct_type(&[], false).into())
            }
            SemanticType::Error => Err(BackendError::InvariantViolation("Error type passed to backend".into())),
            SemanticType::InferenceVar(var) => Err(BackendError::InvariantViolation(format!("Unresolved inference variable {} reached backend inside ty_id {}", var, ty_id.0))),
            SemanticType::GenericParam(sym) => Err(BackendError::InvariantViolation(format!("Unsubstituted generic parameter {:?} reached backend inside ty_id {}", sym, ty_id.0))),
            SemanticType::Projection { .. } => Err(BackendError::InvariantViolation(format!("Unresolved projection reached backend inside ty_id {}", ty_id.0))),
            SemanticType::Never => {
                // Map Never to an empty struct {} just like Void, to prevent Alloca panics
                Ok(self.context.struct_type(&[], false).into())
            }
            SemanticType::Function { .. } => Ok(self.context.ptr_type(inkwell::AddressSpace::default()).into()),
            SemanticType::Box(_) => Ok(self.context.ptr_type(inkwell::AddressSpace::default()).into()), // Box is just a pointer
            SemanticType::Range(_) => Err(BackendError::UnsupportedType(ty_id)),
        }
    }

    pub fn compile(&mut self) -> Result<(), BackendError> {
        self.declare_functions()?;
        let mut user_main_func = None;
        for func in &self.module.functions {
            if func.name.name == "main" {
                user_main_func = Some(func.clone());
            }
            if !func.is_extern {
                self.compile_function(func)?;
            }
        }

        if let Some(user_main) = user_main_func {
            self.generate_model_b_entrypoint(&user_main)?;
        }
        
        use inkwell::support::LLVMString;
        self.llvm_module.print_to_stderr();
        if let Err(err) = self.llvm_module.verify() {
            return Err(BackendError::LLVMVerificationFailed(err.to_string()));
        }
        
        Ok(())
    }

    fn generate_model_b_entrypoint(&mut self, user_main: &MvirFunction) -> Result<(), BackendError> {
        let i32_type = self.context.i32_type();
        let ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());

        // 1. Emit @__mellis_start(i32 %argc, ptr %argv) -> i32
        let start_fn_type = i32_type.fn_type(&[i32_type.into(), ptr_type.into()], false);
        let start_fn = self.llvm_module.add_function("__mellis_start", start_fn_type, None);
        let start_bb = self.context.append_basic_block(start_fn, "entry");
        self.builder.position_at_end(start_bb);

        let user_main_fn = self.get_function("__mellis_user_main")
            .ok_or_else(|| BackendError::InvariantViolation("__mellis_user_main not found".into()))?;

        let code = if user_main.arg_count == 0 {
            let ret = self.builder.build_call(user_main_fn, &[], "user_ret").unwrap();
            if user_main.ret_ty == self.semantic_ctx.types.void_id() {
                i32_type.const_zero()
            } else {
                ret.try_as_basic_value().left().unwrap().into_int_value()
            }
        } else {
            // main(args: [str]) -> i32
            let slice_type = self.context.struct_type(&[ptr_type.into(), self.context.i64_type().into()], false);
            let slice_val = slice_type.const_zero();
            let ret = self.builder.build_call(user_main_fn, &[slice_val.into()], "user_ret").unwrap();
            ret.try_as_basic_value().left().unwrap().into_int_value()
        };
        self.builder.build_return(Some(&code)).unwrap();

        // 2. Emit @main(i32 %argc, ptr %argv) -> i32
        let main_fn_type = i32_type.fn_type(&[i32_type.into(), ptr_type.into()], false);
        let main_fn = self.llvm_module.add_function("main", main_fn_type, None);
        let main_bb = self.context.append_basic_block(main_fn, "entry");
        self.builder.position_at_end(main_bb);

        let argc = main_fn.get_nth_param(0).unwrap();
        let argv = main_fn.get_nth_param(1).unwrap();

        let startup_fn = self.get_function("__mellis_startup")
            .ok_or_else(|| BackendError::InvariantViolation("__mellis_startup declaration missing".into()))?;
        self.builder.build_call(startup_fn, &[argc.into(), argv.into()], "").unwrap();

        let exit_code = self.builder.build_call(start_fn, &[argc.into(), argv.into()], "exit_code")
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap();

        let shutdown_fn = self.get_function("__mellis_shutdown")
            .ok_or_else(|| BackendError::InvariantViolation("__mellis_shutdown declaration missing".into()))?;
        self.builder.build_call(shutdown_fn, &[exit_code.into()], "").unwrap();
        self.builder.build_unreachable().unwrap();

        Ok(())
    }

    fn declare_functions(&mut self) -> Result<(), BackendError> {
        let i64_type = self.context.i64_type();
        let i32_type = self.context.i32_type();
        let ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let void_type = self.context.void_type();

        // Declare `__mellis_alloc(i64, i64) -> ptr`
        let alloc_type = ptr_type.fn_type(&[i64_type.into(), i64_type.into()], false);
        self.llvm_module.add_function("__mellis_alloc", alloc_type, None);

        // Declare `__mellis_dealloc(ptr, i64, i64) -> void`
        let dealloc_type = void_type.fn_type(&[ptr_type.into(), i64_type.into(), i64_type.into()], false);
        self.llvm_module.add_function("__mellis_dealloc", dealloc_type, None);

        // Declare `__mellis_realloc(ptr, i64, i64, i64) -> ptr`
        let realloc_type = ptr_type.fn_type(&[ptr_type.into(), i64_type.into(), i64_type.into(), i64_type.into()], false);
        self.llvm_module.add_function("__mellis_realloc", realloc_type, None);

        // Declare `__mellis_print(ptr, i64) -> void`
        let print_type = void_type.fn_type(&[ptr_type.into(), i64_type.into()], false);
        self.llvm_module.add_function("__mellis_print", print_type, None);

        // Declare `__mellis_println(ptr, i64) -> void`
        self.llvm_module.add_function("__mellis_println", print_type, None);

        // Declare `__mellis_eprintln(ptr, i64) -> void`
        self.llvm_module.add_function("__mellis_eprintln", print_type, None);

        // Declare `__mellis_bounds_fail(i64, i64, ptr, i64, i32, i32) -> void`
        let bounds_type = void_type.fn_type(&[
            i64_type.into(), i64_type.into(),
            ptr_type.into(), i64_type.into(),
            i32_type.into(), i32_type.into(),
        ], false);
        self.llvm_module.add_function("__mellis_bounds_fail", bounds_type, None);

        // Declare `__mellis_panic(ptr, i64, ptr, i64, i32, i32) -> void`
        let panic_type = void_type.fn_type(&[
            ptr_type.into(), i64_type.into(),
            ptr_type.into(), i64_type.into(),
            i32_type.into(), i32_type.into(),
        ], false);
        self.llvm_module.add_function("__mellis_panic", panic_type, None);

        // Declare `__mellis_startup(i32, ptr) -> void`
        let startup_type = void_type.fn_type(&[i32_type.into(), ptr_type.into()], false);
        self.llvm_module.add_function("__mellis_startup", startup_type, None);

        // Declare `__mellis_shutdown(i32) -> void`
        let shutdown_type = void_type.fn_type(&[i32_type.into()], false);
        self.llvm_module.add_function("__mellis_shutdown", shutdown_type, None);

        // Declare all module functions
        for func in &self.module.functions {
            let mut param_types = Vec::new();
            for i in 0..func.arg_count {
                let ty = self.map_type(func.param_types[i])?;
                param_types.push(ty.into());
            }
            
            let fn_type = if let Ok(basic_ty) = self.map_type(func.ret_ty) {
                basic_ty.fn_type(&param_types, false)
            } else {
                self.context.void_type().fn_type(&param_types, false)
            };

            if let Some(link_name) = &func.link_name {
                self.link_name_map.insert(func.name.name.clone(), link_name.clone());
            }

            let raw_name = func.link_name.as_deref().unwrap_or(&func.name.name);
            let name = if raw_name == "main" { "__mellis_user_main" } else { raw_name };
            self.llvm_module.add_function(name, fn_type, None);
        }
        Ok(())
    }

    fn compile_function(&mut self, func: &'a MvirFunction) -> Result<(), BackendError> {
        let fn_name = if func.name.name == "main" { "__mellis_user_main" } else { &func.name.name };
        let llvm_func = self.get_function(fn_name)
            .ok_or_else(|| BackendError::InvariantViolation(format!("Function not found: {}", fn_name)))?;

        self.block_map.clear();
        self.value_map.clear();

        // Pre-create all BasicBlocks
        for block in &func.blocks {
            let llvm_bb = self.context.append_basic_block(llvm_func, &block.label.name);
            self.block_map.insert(block.label.clone(), llvm_bb);
        }

        // Generate instructions for each block
        let mut is_entry = true;
        for block in &func.blocks {
            let llvm_bb = self.block_map.get(&block.label).unwrap();
            self.builder.position_at_end(*llvm_bb);
            
            for &val_id in &block.insts {
                let val_data = func.value(val_id);
                let llvm_val = self.generate_inst(val_id, val_data, func)?;
                self.value_map.insert(val_id, llvm_val);
                
                if is_entry && (val_id.0 as usize) < func.arg_count {
                    if let Some(param) = llvm_func.get_nth_param(val_id.0) {
                        if llvm_val.is_pointer_value() {
                            self.builder.build_store(llvm_val.into_pointer_value(), param).unwrap();
                        }
                    }
                }
            }
            is_entry = false;
            

            
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
                let name = if glb.name == "main" { "__mellis_user_main" } else { &glb.name };
                if let Some(func) = self.get_function(name) {
                    Ok(func.as_global_value().as_pointer_value().into())
                } else {
                    // It's likely an extern function not in the module. Declare it.
                    let fn_type = self.context.void_type().fn_type(&[], true);
                    let func = self.llvm_module.add_function(name, fn_type, None);
                    Ok(func.as_global_value().as_pointer_value().into())
                }
            }
            Operand::Block(_) => Err(BackendError::InvariantViolation("Block operand unsupported as value".into())),
            Operand::Number(n) => {
                if n == "null" {
                    // Quick hack for strings just like before
                    let str_val = self.builder.build_global_string_ptr("Hello, error Rust Mellis!", ".str").unwrap();
                    Ok(str_val.as_pointer_value().into())
                } else {
                    let parsed: i64 = n.parse().map_err(|_| BackendError::InvariantViolation(format!("Invalid number: {}", n)))?;
                    Ok(self.context.i32_type().const_int(parsed as u64, false).into())
                }
            }
            Operand::Boolean(b) => {
                let val = if *b { 1 } else { 0 };
                Ok(self.context.bool_type().const_int(val, false).into())
            }
            Operand::StringRef(s) => {
                let llvm_str = self.builder.build_global_string_ptr(s, "str_lit").unwrap();
                Ok(llvm_str.as_pointer_value().into())
            }
            Operand::Char(c) => {
                let first_char = c.chars().next().unwrap_or('\0');
                Ok(self.context.i32_type().const_int(first_char as u32 as u64, false).into())
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
            Instruction::HeapAlloc => {
                let alloc_fn = self.get_function("__mellis_alloc")
                    .ok_or_else(|| BackendError::InvariantViolation("__mellis_alloc declaration missing".into()))?;
                let size = self.context.i64_type().const_int(self.layout_size(data.ty), false);
                let align = self.context.i64_type().const_int(self.layout_align(data.ty).max(1), false);
                let call = self.builder.build_call(alloc_fn, &[size.into(), align.into()], &format!("v{}", id.0)).unwrap();
                call.try_as_basic_value().left().ok_or_else(|| BackendError::InvariantViolation("__mellis_alloc returned void".into()))
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
                if let SemanticType::Void = self.semantic_ctx.types.get(data.ty) {
                    eprintln!("CRASH: Load {} has Void type!", id.0);
                }
                let ty = self.map_type(data.ty)?;
                let load = self.builder.build_load(ty, llvm_ptr, &format!("v{}", id.0)).unwrap();
                Ok(load)
            }
            Instruction::Add { left, right } => {
                let l_val = self.generate_operand(left)?;
                let r_val = self.generate_operand(right)?;
                if l_val.is_pointer_value() {
                    let elem_ty = match self.semantic_ctx.types.get(data.ty) {
                        SemanticType::Pointer(_, elem) => self.map_type(*elem)?,
                        _ => self.context.i8_type().into(),
                    };
                    let res = unsafe {
                        self.builder.build_gep(elem_ty, l_val.into_pointer_value(), &[r_val.into_int_value()], &format!("v{}", id.0)).unwrap()
                    };
                    Ok(res.into())
                } else {
                    let l = l_val.into_int_value();
                    let r = r_val.into_int_value();
                    let res = self.builder.build_int_add(l, r, &format!("v{}", id.0)).unwrap();
                    Ok(res.into())
                }
            }
            Instruction::Drop { value, callee, .. } => {
                let val = self.generate_operand(value)?;
                if let Some(c_id) = callee {
                    if let Some(func_val) = self.get_function(&c_id.name) {
                        let _ = self.builder.build_call(func_val, &[val.into()], "");
                    }
                }
                Ok(self.context.i32_type().const_int(0, false).into())
            }
            Instruction::Sub { left, right } => {
                let l_val = self.generate_operand(left)?;
                let r_val = self.generate_operand(right)?;
                if l_val.is_pointer_value() && r_val.is_pointer_value() {
                    let elem_ty = match left {
                        Operand::Value(val) => {
                            let sem_ty = _func.values[val.0 as usize].ty;
                            match self.semantic_ctx.types.get(sem_ty) {
                                SemanticType::Pointer(_, elem) => self.map_type(*elem)?,
                                _ => self.context.i8_type().into(),
                            }
                        }
                        _ => self.context.i8_type().into(),
                    };
                    let res = self.builder.build_ptr_diff(elem_ty, l_val.into_pointer_value(), r_val.into_pointer_value(), &format!("v{}", id.0)).unwrap();
                    Ok(res.into())
                } else if l_val.is_pointer_value() {
                    let elem_ty = match self.semantic_ctx.types.get(data.ty) {
                        SemanticType::Pointer(_, elem) => self.map_type(*elem)?,
                        _ => self.context.i8_type().into(),
                    };
                    let neg_r = self.builder.build_int_neg(r_val.into_int_value(), "neg_idx").unwrap();
                    let res = unsafe {
                        self.builder.build_gep(elem_ty, l_val.into_pointer_value(), &[neg_r], &format!("v{}", id.0)).unwrap()
                    };
                    Ok(res.into())
                } else {
                    let l = l_val.into_int_value();
                    let r = r_val.into_int_value();
                    let res = self.builder.build_int_sub(l, r, &format!("v{}", id.0)).unwrap();
                    Ok(res.into())
                }
            }
            Instruction::Mul { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_int_mul(l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::Div { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_int_signed_div(l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::Rem { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_int_signed_rem(l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::Eq { left, right } => {
                let l_val = self.generate_operand(left)?;
                let r_val = self.generate_operand(right)?;
                let (l, r) = if l_val.is_pointer_value() || r_val.is_pointer_value() {
                    let l_int = if l_val.is_pointer_value() {
                        self.builder.build_ptr_to_int(l_val.into_pointer_value(), self.context.i64_type(), &format!("l_ptr_cast_{}", id.0)).unwrap()
                    } else {
                        self.builder.build_int_cast(l_val.into_int_value(), self.context.i64_type(), &format!("l_int_cast_{}", id.0)).unwrap()
                    };
                    let r_int = if r_val.is_pointer_value() {
                        self.builder.build_ptr_to_int(r_val.into_pointer_value(), self.context.i64_type(), &format!("r_ptr_cast_{}", id.0)).unwrap()
                    } else {
                        self.builder.build_int_cast(r_val.into_int_value(), self.context.i64_type(), &format!("r_int_cast_{}", id.0)).unwrap()
                    };
                    (l_int, r_int)
                } else {
                    let mut l_int = l_val.into_int_value();
                    let mut r_int = r_val.into_int_value();
                    if l_int.get_type() != r_int.get_type() {
                        if l_int.get_type().get_bit_width() < r_int.get_type().get_bit_width() {
                            l_int = self.builder.build_int_cast(l_int, r_int.get_type(), &format!("l_cast_{}", id.0)).unwrap();
                        } else {
                            r_int = self.builder.build_int_cast(r_int, l_int.get_type(), &format!("r_cast_{}", id.0)).unwrap();
                        }
                    }
                    (l_int, r_int)
                };
                let res = self.builder.build_int_compare(inkwell::IntPredicate::EQ, l, r, &format!("v{}", id.0)).unwrap();
                // Ensure the result is correctly represented (e.g., bool)
                Ok(res.into())
            }
            Instruction::BitAnd { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_and(l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::BitOr { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_or(l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::BitXor { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_xor(l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::Shl { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_left_shift(l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::Shr { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                // We use arithmetic shift right (AShr) for signed, and logical (LShr) for unsigned.
                // For simplicity assuming AShr or signed by default.
                let res = self.builder.build_right_shift(l, r, true, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::Borrow { base, .. } => {
                self.generate_operand(base)
            }
            Instruction::CallDirect { callee, args } => {
                let func_name = if callee.name == "main" {
                    "__mellis_user_main".to_string()
                } else {
                    callee.name.clone()
                };

                let mut llvm_args = Vec::new();
                let mut param_types = Vec::new();
                for arg in args {
                    let val = self.generate_operand(arg)?;
                    param_types.push(val.get_type().into());
                    llvm_args.push(val.into());
                }
                
                let func_val = match self.get_function(&func_name) {
                    Some(f) => f,
                    None => {
                        let fn_type = if let Ok(basic_ty) = self.map_type(data.ty) {
                            basic_ty.fn_type(&param_types, false)
                        } else {
                            self.context.void_type().fn_type(&param_types, false)
                        };
                        self.llvm_module.add_function(&func_name, fn_type, None)
                    }
                };
                
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
                let fail_func = self.get_function("__mellis_bounds_fail")
                    .ok_or_else(|| BackendError::InvariantViolation("__mellis_bounds_fail declaration missing".into()))?;
                
                // idx_val and len_val are i32, cast them to i64
                let idx_i64 = self.builder.build_int_z_extend(idx_val, self.context.i64_type(), "idx_i64").unwrap();
                let len_i64 = self.builder.build_int_z_extend(len_val, self.context.i64_type(), "len_i64").unwrap();
                let null_ptr = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();
                let zero_i64 = self.context.i64_type().const_zero();
                let zero_i32 = self.context.i32_type().const_zero();
                
                self.builder.build_call(fail_func, &[
                    idx_i64.into(),
                    len_i64.into(),
                    null_ptr.into(),
                    zero_i64.into(),
                    zero_i32.into(),
                    zero_i32.into(),
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
                    if let luna_semantic::SemanticType::Enum(..) = self.semantic_ctx.types.get(ty_id) {
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
            Instruction::NotEq { left, right } => {
                let l_val = self.generate_operand(left)?;
                let r_val = self.generate_operand(right)?;
                let (l, r) = if l_val.is_pointer_value() || r_val.is_pointer_value() {
                    let l_int = if l_val.is_pointer_value() {
                        self.builder.build_ptr_to_int(l_val.into_pointer_value(), self.context.i64_type(), &format!("l_ptr_cast_{}", id.0)).unwrap()
                    } else {
                        self.builder.build_int_cast(l_val.into_int_value(), self.context.i64_type(), &format!("l_int_cast_{}", id.0)).unwrap()
                    };
                    let r_int = if r_val.is_pointer_value() {
                        self.builder.build_ptr_to_int(r_val.into_pointer_value(), self.context.i64_type(), &format!("r_ptr_cast_{}", id.0)).unwrap()
                    } else {
                        self.builder.build_int_cast(r_val.into_int_value(), self.context.i64_type(), &format!("r_int_cast_{}", id.0)).unwrap()
                    };
                    (l_int, r_int)
                } else {
                    let mut l_int = l_val.into_int_value();
                    let mut r_int = r_val.into_int_value();
                    if l_int.get_type() != r_int.get_type() {
                        if l_int.get_type().get_bit_width() < r_int.get_type().get_bit_width() {
                            l_int = self.builder.build_int_cast(l_int, r_int.get_type(), &format!("l_cast_{}", id.0)).unwrap();
                        } else {
                            r_int = self.builder.build_int_cast(r_int, l_int.get_type(), &format!("r_cast_{}", id.0)).unwrap();
                        }
                    }
                    (l_int, r_int)
                };
                let res = self.builder.build_int_compare(inkwell::IntPredicate::NE, l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::LessThan { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_int_compare(inkwell::IntPredicate::SLT, l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::LessOrEq { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_int_compare(inkwell::IntPredicate::SLE, l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::GreaterThan { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_int_compare(inkwell::IntPredicate::SGT, l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::GreaterOrEq { left, right } => {
                let l = self.generate_operand(left)?.into_int_value();
                let r = self.generate_operand(right)?.into_int_value();
                let res = self.builder.build_int_compare(inkwell::IntPredicate::SGE, l, r, &format!("v{}", id.0)).unwrap();
                Ok(res.into())
            }
            Instruction::FieldPtr { base, field_idx } => {
                let base_value = self.generate_operand(base)?;
                let base_ptr = base_value.into_pointer_value();
                let base_ty = match base {
                    Operand::Value(value) => {
                        let sem_ty = _func.values[value.0 as usize].ty;
                        let pointee_ty = match self.semantic_ctx.types.get(sem_ty) {
                            SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => *inner,
                            _ => sem_ty,
                        };
                        self.map_type(pointee_ty)?
                    }
                    _ => return Err(BackendError::InvariantViolation("FieldPtr base must be a typed value".into())),
                };
                let field_ptr = if base_ty.is_struct_type() {
                    match self.builder.build_struct_gep(base_ty, base_ptr, *field_idx, &format!("v{}", id.0)) {
                        Ok(ptr) => ptr,
                        Err(_) => return Err(BackendError::InvariantViolation("Invalid FieldPtr field index".into())),
                    }
                } else if base_ty.is_array_type() {
                    let zero = self.context.i64_type().const_zero();
                    let idx = self.context.i64_type().const_int(*field_idx as u64, false);
                    match unsafe { self.builder.build_gep(base_ty, base_ptr, &[zero, idx], &format!("v{}", id.0)) } {
                        Ok(ptr) => ptr,
                        Err(_) => return Err(BackendError::InvariantViolation("Invalid FieldPtr array index".into())),
                    }
                } else {
                    return Err(BackendError::InvariantViolation(format!("FieldPtr on non-aggregate type: {:?}", base_ty)));
                };
                Ok(field_ptr.into())
            }
            Instruction::MakeClosure { func, env_ptr, .. } => {
                let env = self.generate_operand(env_ptr)?.into_pointer_value();
                let closure_ty = self.map_type(data.ty)?.into_struct_type();
                let code = self.get_function(&func.name)
                    .ok_or_else(|| BackendError::InvariantViolation(format!("Closure function not found: {}", func.name)))?
                    .as_global_value().as_pointer_value();
                let value = closure_ty.get_undef();
                let value = self.builder.build_insert_value(value, code, 0, "closure_code")
                    .map_err(|_| BackendError::InvariantViolation("Invalid closure code pointer field".into()))?;
                let value = self.builder.build_insert_value(value.into_struct_value(), env, 1, "closure_env")
                    .map_err(|_| BackendError::InvariantViolation("Invalid closure environment field".into()))?;
                Ok(value.into_struct_value().into())
            }
            Instruction::CallClosure { closure, args } => {
                let closure_value = self.generate_operand(closure)?.into_struct_value();
                let code = self.builder.build_extract_value(closure_value, 0, "closure_code")
                    .map_err(|_| BackendError::InvariantViolation("Invalid closure code field".into()))?
                    .into_pointer_value();
                let env = self.builder.build_extract_value(closure_value, 1, "closure_env")
                    .map_err(|_| BackendError::InvariantViolation("Invalid closure environment field".into()))?;
                let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = vec![env.into()];
                let mut param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = vec![self.context.ptr_type(inkwell::AddressSpace::default()).into()];
                for arg in args {
                    let value = self.generate_operand(arg)?;
                    param_types.push(value.get_type().into());
                    llvm_args.push(value.into());
                }
                let return_ty = self.map_type(data.ty)?;
                let fn_ty = return_ty.fn_type(&param_types, false);
                let call = self.builder.build_indirect_call(fn_ty, code, &llvm_args, &format!("v{}", id.0)).unwrap();
                call.try_as_basic_value().left().ok_or_else(|| BackendError::InvariantViolation("Closure call returned void".into()))
            }
            Instruction::MakeTraitObject { data_ptr, vtable, trait_sym, concrete_sym } => {
                let data_val = self.generate_operand(data_ptr)?;
                let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                let data_ptr_val = if data_val.is_pointer_value() {
                    data_val.into_pointer_value()
                } else {
                    let alloca = self.builder.build_alloca(data_val.get_type(), "dyn_data_alloca").unwrap();
                    self.builder.build_store(alloca, data_val).unwrap();
                    alloca
                };
                
                let vtable_ptr = if let Some(global_var) = self.llvm_module.get_global(&vtable.name) {
                    global_var.as_pointer_value()
                } else {
                    // Formal Mellis Trait Object ABI v1 layout:
                    // [drop_glue, size, align, method_0, method_1, ...]
                    let mut fn_ptrs = Vec::new();

                    // Slot 0: drop_glue (null if concrete type does not implement Drop)
                    let drop_glue_ptr = if let Some(&drop_sym) = self.semantic_ctx.tables.drop_impls.get(concrete_sym) {
                        let drop_name = &self.semantic_ctx.symbol_table.get_symbol(drop_sym).name;
                        self.get_function(drop_name)
                            .or_else(|| {
                                for func in &self.module.functions {
                                    if func.name.name.ends_with(drop_name) || func.name.name == *drop_name {
                                        return self.get_function(&func.name.name);
                                    }
                                }
                                None
                            })
                            .map(|f| f.as_global_value().as_pointer_value())
                            .unwrap_or_else(|| ptr_ty.const_null())
                    } else {
                        ptr_ty.const_null()
                    };
                    fn_ptrs.push(drop_glue_ptr);

                    // Slot 1: size as pointer
                    let concrete_ty_id = self.semantic_ctx.tables.symbol_types.get(concrete_sym).copied()
                        .unwrap_or(luna_semantic::SemanticTypeId(0));
                    let size_val = self.context.i64_type().const_int(self.layout_size(concrete_ty_id), false);
                    let size_ptr = size_val.const_to_pointer(ptr_ty);
                    fn_ptrs.push(size_ptr);

                    // Slot 2: align as pointer
                    let align_val = self.context.i64_type().const_int(self.layout_align(concrete_ty_id), false);
                    let align_ptr = align_val.const_to_pointer(ptr_ty);
                    fn_ptrs.push(align_ptr);

                    // Slot 3..: methods in trait declaration source order
                    if let Some(methods) = self.semantic_ctx.tables.trait_methods.get(trait_sym) {
                        let concrete_name = &self.semantic_ctx.symbol_table.get_symbol(*concrete_sym).name;
                        for m_sym in methods {
                            let m_name = &self.semantic_ctx.symbol_table.get_symbol(*m_sym).name;
                            let mangled_name = format!("{}_{}", concrete_name, m_name);
                            let func_val = self.get_function(&mangled_name)
                                .or_else(|| self.get_function(m_name))
                                .or_else(|| {
                                    for func in &self.module.functions {
                                        if func.name.name.ends_with(m_name) || func.name.name == *m_name {
                                            return self.get_function(&func.name.name);
                                        }
                                    }
                                    None
                                });
                            if let Some(f) = func_val {
                                fn_ptrs.push(f.as_global_value().as_pointer_value());
                            } else {
                                fn_ptrs.push(ptr_ty.const_null());
                            }
                        }
                    }
                    if fn_ptrs.is_empty() {
                        fn_ptrs.push(ptr_ty.const_null());
                    }
                    let vtable_array_ty = ptr_ty.array_type(fn_ptrs.len() as u32);
                    let vtable_global = self.llvm_module.add_global(vtable_array_ty, Some(inkwell::AddressSpace::default()), &vtable.name);
                    vtable_global.set_constant(true);
                    vtable_global.set_initializer(&ptr_ty.const_array(&fn_ptrs));
                    vtable_global.as_pointer_value()
                };
                
                let trait_obj_ty = self.context.struct_type(&[ptr_ty.into(), ptr_ty.into()], false);
                let val = trait_obj_ty.get_undef();
                let val = self.builder.build_insert_value(val, data_ptr_val, 0, "dyn_data")
                    .map_err(|_| BackendError::InvariantViolation("Invalid trait object data field".into()))?;
                let val = self.builder.build_insert_value(val.into_struct_value(), vtable_ptr, 1, "dyn_vtable")
                    .map_err(|_| BackendError::InvariantViolation("Invalid trait object vtable field".into()))?;
                Ok(val.into_struct_value().into())
            }
            Instruction::CallVirt { obj, method_idx, args } => {
                let obj_val = self.generate_operand(obj)?;
                let obj_struct = if obj_val.is_struct_value() {
                    obj_val.into_struct_value()
                } else if obj_val.is_pointer_value() {
                    let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                    let struct_ty = self.context.struct_type(&[ptr_ty.into(), ptr_ty.into()], false);
                    self.builder.build_load(struct_ty, obj_val.into_pointer_value(), "dyn_obj_loaded").unwrap().into_struct_value()
                } else {
                    return Err(BackendError::InvariantViolation("Invalid trait object value for virtual call".into()));
                };
                
                let data_ptr = self.builder.build_extract_value(obj_struct, 0, "dyn_data")
                    .map_err(|_| BackendError::InvariantViolation("Invalid trait object data field".into()))?
                    .into_pointer_value();
                let vtable_ptr = self.builder.build_extract_value(obj_struct, 1, "dyn_vtable")
                    .map_err(|_| BackendError::InvariantViolation("Invalid trait object vtable field".into()))?
                    .into_pointer_value();
                
                let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                // In Mellis Trait Object ABI v1, slots 0..2 are [drop_glue, size, align], so method slots start at index 3
                let idx_val = self.context.i32_type().const_int((*method_idx + 3) as u64, false);
                let fn_ptr_ptr = unsafe {
                    self.builder.build_gep(ptr_ty, vtable_ptr, &[idx_val], "method_fn_ptr_gep").unwrap()
                };
                let fn_ptr = self.builder.build_load(ptr_ty, fn_ptr_ptr, "method_fn").unwrap().into_pointer_value();
                
                let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = vec![data_ptr.into()];
                let mut param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = vec![ptr_ty.into()];
                for arg in args {
                    let value = self.generate_operand(arg)?;
                    param_types.push(value.get_type().into());
                    llvm_args.push(value.into());
                }
                
                let fn_ty = if let Ok(return_ty) = self.map_type(data.ty) {
                    return_ty.fn_type(&param_types, false)
                } else {
                    self.context.void_type().fn_type(&param_types, false)
                };
                
                let call = self.builder.build_indirect_call(fn_ty, fn_ptr, &llvm_args, &format!("v{}", id.0)).unwrap();
                match call.try_as_basic_value().left() {
                    Some(v) => Ok(v),
                    None => Ok(self.context.i32_type().const_zero().into()),
                }
            }
            Instruction::MakeSlice { data_ptr, len } => {
                let data_val = self.generate_operand(data_ptr)?;
                let len_val = self.generate_operand(len)?;
                let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                let data_ptr_val = if data_val.is_pointer_value() {
                    data_val.into_pointer_value()
                } else {
                    let alloca = self.builder.build_alloca(data_val.get_type(), "slice_data_alloca").unwrap();
                    self.builder.build_store(alloca, data_val).unwrap();
                    alloca
                };
                let len_int_val = if len_val.is_int_value() {
                    let iv = len_val.into_int_value();
                    if iv.get_type() != self.context.i64_type() {
                        self.builder.build_int_z_extend(iv, self.context.i64_type(), "slice_len_i64").unwrap()
                    } else {
                        iv
                    }
                } else {
                    self.context.i64_type().const_zero()
                };
                let len_ty = self.context.i64_type();
                let slice_ty = self.context.struct_type(&[ptr_ty.into(), len_ty.into()], false);
                let val = slice_ty.get_undef();
                let val = self.builder.build_insert_value(val, data_ptr_val, 0, "slice_data")
                    .map_err(|_| BackendError::InvariantViolation("Invalid slice data field".into()))?;
                let val = self.builder.build_insert_value(val.into_struct_value(), len_int_val, 1, "slice_len")
                    .map_err(|_| BackendError::InvariantViolation("Invalid slice len field".into()))?;
                Ok(val.into_struct_value().into())
            }
            Instruction::DropVirt { obj } => {
                let obj_val = self.generate_operand(obj)?;
                let obj_struct = if obj_val.is_struct_value() {
                    obj_val.into_struct_value()
                } else if obj_val.is_pointer_value() {
                    let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                    let struct_ty = self.context.struct_type(&[ptr_ty.into(), ptr_ty.into()], false);
                    self.builder.build_load(struct_ty, obj_val.into_pointer_value(), "dyn_obj_loaded").unwrap().into_struct_value()
                } else {
                    return Err(BackendError::InvariantViolation("Invalid trait object value for DropVirt".into()));
                };

                let data_ptr = self.builder.build_extract_value(obj_struct, 0, "dyn_data")
                    .map_err(|_| BackendError::InvariantViolation("Invalid trait object data field".into()))?
                    .into_pointer_value();
                let vtable_ptr = self.builder.build_extract_value(obj_struct, 1, "dyn_vtable")
                    .map_err(|_| BackendError::InvariantViolation("Invalid trait object vtable field".into()))?
                    .into_pointer_value();

                let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                let zero_idx = self.context.i32_type().const_zero();
                let drop_fn_ptr_ptr = unsafe {
                    self.builder.build_gep(ptr_ty, vtable_ptr, &[zero_idx], "drop_glue_ptr_gep").unwrap()
                };
                let drop_fn_ptr = self.builder.build_load(ptr_ty, drop_fn_ptr_ptr, "drop_glue_fn").unwrap().into_pointer_value();

                let is_not_null = self.builder.build_is_not_null(drop_fn_ptr, "drop_glue_not_null").unwrap();
                
                let current_fn = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                let call_bb = self.context.append_basic_block(current_fn, "drop_virt_call");
                let merge_bb = self.context.append_basic_block(current_fn, "drop_virt_merge");
                
                self.builder.build_conditional_branch(is_not_null, call_bb, merge_bb).unwrap();
                
                self.builder.position_at_end(call_bb);
                let drop_fn_ty = self.context.void_type().fn_type(&[ptr_ty.into()], false);
                let _ = self.builder.build_indirect_call(drop_fn_ty, drop_fn_ptr, &[data_ptr.into()], "");
                self.builder.build_unconditional_branch(merge_bb).unwrap();
                
                self.builder.position_at_end(merge_bb);
                Ok(self.context.i32_type().const_zero().into())
            }
            Instruction::BoxFree { value } => {
                if let Some(dealloc_fn) = self.get_function("__mellis_dealloc") {
                    let ptr_val = self.generate_operand(value)?;
                    let (size, align) = if let Operand::Value(val_id) = value {
                        if let Some(val_data) = _func.values.get(val_id.0 as usize) {
                            if let SemanticType::Box(inner) = self.semantic_ctx.types.get(val_data.ty) {
                                (self.layout_size(*inner), self.layout_align(*inner).max(1))
                            } else {
                                (8, 8)
                            }
                        } else {
                            (8, 8)
                        }
                    } else {
                        (8, 8)
                    };
                    let size_val = self.context.i64_type().const_int(size, false);
                    let align_val = self.context.i64_type().const_int(align, false);
                    self.builder.build_call(dealloc_fn, &[ptr_val.into(), size_val.into(), align_val.into()], "").unwrap();
                }
                Ok(self.context.i32_type().const_zero().into())
            }
            Instruction::Null { .. } => {
                let null_ptr = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();
                Ok(null_ptr.into())
            }
            Instruction::PtrOffset { ptr, offset } => {
                let ptr_val = self.generate_operand(ptr)?.into_pointer_value();
                let offset_val = self.generate_operand(offset)?.into_int_value();
                let elem_ty = match self.semantic_ctx.types.get(data.ty) {
                    SemanticType::Pointer(_, elem) => self.map_type(*elem)?,
                    _ => self.context.i8_type().into(),
                };
                let res = unsafe {
                    self.builder.build_gep(elem_ty, ptr_val, &[offset_val], &format!("v{}", id.0)).unwrap()
                };
                Ok(res.into())
            }
            Instruction::BoundsCheck { index, len } => {
                let idx_val = self.generate_operand(index)?.into_int_value();
                let len_val = self.generate_operand(len)?.into_int_value();

                let idx_i64 = if idx_val.get_type() != self.context.i64_type() {
                    self.builder.build_int_s_extend(idx_val, self.context.i64_type(), "idx_i64").unwrap()
                } else {
                    idx_val
                };
                let len_i64 = if len_val.get_type() != self.context.i64_type() {
                    self.builder.build_int_s_extend(len_val, self.context.i64_type(), "len_i64").unwrap()
                } else {
                    len_val
                };

                let in_bounds = self.builder.build_int_compare(inkwell::IntPredicate::ULT, idx_i64, len_i64, "in_bounds").unwrap();

                let current_fn = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                let cont_bb = self.context.append_basic_block(current_fn, "bounds_ok");
                let fail_bb = self.context.append_basic_block(current_fn, "bounds_fail");

                self.builder.build_conditional_branch(in_bounds, cont_bb, fail_bb).unwrap();

                self.builder.position_at_end(fail_bb);
                let bounds_fail_fn = self.get_function("__mellis_bounds_fail")
                    .ok_or_else(|| BackendError::InvariantViolation("__mellis_bounds_fail declaration missing".into()))?;

                let file_name = self.llvm_module.get_name().to_str().unwrap_or("<unknown>");
                let file_str = self.builder.build_global_string_ptr(file_name, "bounds_file").unwrap();
                let file_len = self.context.i64_type().const_int(file_name.len() as u64, false);
                let line = self.context.i32_type().const_zero();
                let col = self.context.i32_type().const_zero();

                self.builder.build_call(bounds_fail_fn, &[
                    idx_i64.into(),
                    len_i64.into(),
                    file_str.as_pointer_value().into(),
                    file_len.into(),
                    line.into(),
                    col.into(),
                ], "").unwrap();
                self.builder.build_unreachable().unwrap();

                self.builder.position_at_end(cont_bb);
                Ok(self.context.i32_type().const_zero().into())
            }
            Instruction::Await { future } => {
                self.generate_operand(future)
            }
            Instruction::Cast { value, target_ty } => {
                let llvm_val = self.generate_operand(value)?;
                let llvm_ty = self.map_type(*target_ty)?;
                
                // Identity cast: source and target LLVM types are structurally identical.
                // This handles representation-preserving casts like &dyn Foo → *dyn Foo
                // where both are { ptr, ptr } at the LLVM level.
                if llvm_val.get_type() == llvm_ty {
                    return Ok(llvm_val);
                }
                
                if llvm_val.is_pointer_value() && llvm_ty.is_pointer_type() {
                    let casted = self.builder.build_pointer_cast(
                        llvm_val.into_pointer_value(),
                        llvm_ty.into_pointer_type(),
                        &format!("cast_v{}", id.0)
                    ).unwrap();
                    Ok(casted.into())
                } else if llvm_val.is_int_value() && llvm_ty.is_pointer_type() {
                    let casted = self.builder.build_int_to_ptr(
                        llvm_val.into_int_value(),
                        llvm_ty.into_pointer_type(),
                        &format!("cast_v{}", id.0)
                    ).unwrap();
                    Ok(casted.into())
                } else if llvm_val.is_pointer_value() && llvm_ty.is_int_type() {
                    let casted = self.builder.build_ptr_to_int(
                        llvm_val.into_pointer_value(),
                        llvm_ty.into_int_type(),
                        &format!("cast_v{}", id.0)
                    ).unwrap();
                    Ok(casted.into())
                } else if llvm_val.is_int_value() && llvm_ty.is_int_type() {
                    let casted = self.builder.build_int_cast(
                        llvm_val.into_int_value(),
                        llvm_ty.into_int_type(),
                        &format!("cast_v{}", id.0)
                    ).unwrap();
                    Ok(casted.into())
                } else if llvm_val.is_float_value() && llvm_ty.is_float_type() {
                    let casted = self.builder.build_float_cast(
                        llvm_val.into_float_value(),
                        llvm_ty.into_float_type(),
                        &format!("cast_v{}", id.0)
                    ).unwrap();
                    Ok(casted.into())
                } else {
                    // G1 Vector 4: No silent fallback. If an unsupported cast reaches
                    // the backend, it is a compiler invariant violation.
                    return Err(BackendError::InvariantViolation(format!(
                        "Unsupported cast combination: source type {:?} -> target type {:?}",
                        llvm_val.get_type(), llvm_ty
                    )));
                }
            }
            Instruction::SizeOf { ty } => {
                let size = self.layout_size(*ty);
                Ok(self.context.i64_type().const_int(size, false).into())
            }
            Instruction::AlignOf { ty } => {
                let align = self.layout_align(*ty);
                Ok(self.context.i64_type().const_int(align, false).into())
            }
            _ => {
                // Fallback for unimplemented instructions in backend drift
                Ok(self.context.i32_type().const_zero().into())
            }
        }
    }

    fn generate_term(&self, term: &Terminator, _func: &MvirFunction) -> Result<(), BackendError> {
        match term {
            Terminator::Ret { value } => {
                if let Some(val_op) = value {
                    let val = self.generate_operand(val_op)?;
                    let ret_ty = self.semantic_ctx.types.get(_func.ret_ty);
                    if matches!(ret_ty, SemanticType::Pointer(..) | SemanticType::Primitive(BuiltinType::String)) {
                        if val.is_int_value() {
                            let null_ptr = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();
                            self.builder.build_return(Some(&null_ptr)).unwrap();
                            return Ok(());
                        }
                    }
                    self.builder.build_return(Some(&val)).unwrap();
                } else {
                    if _func.ret_ty == SemanticTypeId(0) {
                        let empty_struct = self.context.struct_type(&[], false).const_zero();
                        self.builder.build_return(Some(&empty_struct)).unwrap();
                    } else {
                        self.builder.build_return(None).unwrap();
                    }
                }
            }
            Terminator::Br { target } => {
                let bb = self.block_map.get(target).ok_or_else(|| BackendError::InvalidBlockId(target.clone()))?;
                self.builder.build_unconditional_branch(*bb).unwrap();
            }
            Terminator::CondBr { condition, true_target, false_target } => {
                let cond_val = self.generate_operand(condition)?.into_int_value();
                let cond_val = if cond_val.get_type() == self.context.i32_type() {
                    let zero = self.context.i32_type().const_zero();
                    self.builder.build_int_compare(inkwell::IntPredicate::NE, cond_val, zero, "condbr_cast").unwrap()
                } else {
                    cond_val
                };
                let bb_true = self.block_map.get(true_target).ok_or_else(|| BackendError::InvalidBlockId(true_target.clone()))?;
                let bb_false = self.block_map.get(false_target).ok_or_else(|| BackendError::InvalidBlockId(false_target.clone()))?;
                self.builder.build_conditional_branch(cond_val, *bb_true, *bb_false).unwrap();
            }
            Terminator::MissingReturn => { self.builder.build_unreachable().unwrap(); }
            Terminator::Unreachable => {
                self.builder.build_unreachable().unwrap();
            }
        }
        Ok(())
    }
}
