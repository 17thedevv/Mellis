use std::io::{Write, Cursor};
use crate::format::{MlibHeader, SectionEntry, SectionType};
use crate::ir::{MlibModule, MlibFunction, MlibValue, MlibBlock, MlibInstruction, MlibTerminator, MlibOperand, MlibTypeEntry};
use luna_mvir::{Module, Function, ValueData, BasicBlock, CaptureInfo, Instruction, Terminator, Operand};
use luna_semantic::CaptureMode;

pub struct MlibWriter;
pub type LlibWriter = MlibWriter;

impl MlibWriter {
    pub fn write_module<W: Write>(module: &Module, arena: &luna_ast::AstArena, items: &[luna_ast::Item], source: &str, mut manifest: crate::format::Manifest, semantic_metadata: Option<&crate::metadata::SemanticMetadata>, obj_bytes: Option<&[u8]>, writer: &mut W) -> std::io::Result<()> {
        let mlib_module = Self::convert_module(module);
        
        let mut mvir_payload = Vec::new();
        Self::serialize_module_internal(&mut mvir_payload, &mlib_module)?;

        let mut ast_payload = Vec::new();
        let mut public_arena = arena.clone();
        
        let mut generic_impl_methods = std::collections::HashSet::new();
        for decl in &public_arena.decls {
            if let luna_ast::Decl::Impl { generic_params, methods, .. } = decl {
                if !generic_params.is_empty() {
                    for method_id in methods {
                        generic_impl_methods.insert(*method_id);
                    }
                }
            }
        }

        for (i, decl) in public_arena.decls.iter_mut().enumerate() {
            if let luna_ast::Decl::Function { generic_params, body, .. } = decl {
                if generic_params.is_empty() && !generic_impl_methods.contains(&luna_ast::DeclId(i as u32)) {
                    *body = None;
                }
            }
        }
        bincode::serialize_into(&mut ast_payload, &(public_arena, items.to_vec(), source.to_string()))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        use sha2::{Sha256, Digest};
        let mut source_hasher = Sha256::new();
        source_hasher.update(source.as_bytes());
        manifest.provenance.source_fingerprint = source_hasher.finalize().into();

        let mut interface_hasher = Sha256::new();
        interface_hasher.update(&ast_payload);
        manifest.provenance.interface_hash = interface_hasher.finalize().into();

        if let Some(obj) = obj_bytes {
            let mut obj_hasher = Sha256::new();
            obj_hasher.update(obj);
            if let Some(ref mut metadata) = manifest.object_metadata {
                metadata.hash = obj_hasher.finalize().into();
            }
        }

        let mut manifest_payload = Vec::new();
        bincode::serialize_into(&mut manifest_payload, &manifest)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let mut semantic_payload = Vec::new();
        if let Some(meta) = semantic_metadata {
            bincode::serialize_into(&mut semantic_payload, meta)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }

        let mut section_count = 3;
        if !semantic_payload.is_empty() {
            section_count += 1;
        }
        if obj_bytes.is_some() {
            section_count += 1;
        }

        let mut header = MlibHeader::new();
        header.section_count = section_count;
        
        let header_size = 122;
        let section_table_size = 40;
        
        let manifest_offset = (header_size + section_table_size * section_count as usize) as u64;
        let mvir_offset = manifest_offset + manifest_payload.len() as u64;
        let ast_offset = mvir_offset + mvir_payload.len() as u64;
        let semantic_offset = ast_offset + ast_payload.len() as u64;
        let obj_offset = if !semantic_payload.is_empty() {
            semantic_offset + semantic_payload.len() as u64
        } else {
            semantic_offset
        };
        
        header.section_table_offset = header_size as u64;

        // Write header
        header.write_to(writer)?;
        
        let manifest_section = SectionEntry {
            section_id: 1,
            section_type: SectionType::Manifest,
            offset: manifest_offset,
            size: manifest_payload.len() as u64,
            version: 1,
            compression: 0,
            reserved: [0u8; 5],
            hash: 0,
        };

        let mvir_section = SectionEntry {
            section_id: 2,
            section_type: SectionType::GenericMVIR,
            offset: mvir_offset,
            size: mvir_payload.len() as u64,
            version: 1,
            compression: 0,
            reserved: [0u8; 5],
            hash: 0,
        };

        let ast_section = SectionEntry {
            section_id: 3,
            section_type: SectionType::AstInterface,
            offset: ast_offset,
            size: ast_payload.len() as u64,
            version: 1,
            compression: 0,
            reserved: [0u8; 5],
            hash: 0,
        };
        
        manifest_section.write_to(writer)?;
        mvir_section.write_to(writer)?;
        ast_section.write_to(writer)?;

        let mut next_section_id = 4;
        if !semantic_payload.is_empty() {
            let semantic_section = SectionEntry {
                section_id: next_section_id,
                section_type: SectionType::SemanticMetadata,
                offset: semantic_offset,
                size: semantic_payload.len() as u64,
                version: 1,
                compression: 0,
                reserved: [0u8; 5],
                hash: 0,
            };
            semantic_section.write_to(writer)?;
            next_section_id += 1;
        }

        if let Some(obj) = obj_bytes {
            let obj_section = SectionEntry {
                section_id: next_section_id,
                section_type: SectionType::ObjectCode,
                offset: obj_offset,
                size: obj.len() as u64,
                version: 1,
                compression: 0,
                reserved: [0u8; 5],
                hash: 0, // In reality, we'd hash this
            };
            obj_section.write_to(writer)?;
        }
        
        // Write payloads
        writer.write_all(&manifest_payload)?;
        writer.write_all(&mvir_payload)?;
        writer.write_all(&ast_payload)?;
        if !semantic_payload.is_empty() {
            writer.write_all(&semantic_payload)?;
        }
        
        if let Some(obj) = obj_bytes {
            writer.write_all(obj)?;
        }
        
        Ok(())
    }
    
    fn convert_module(module: &Module) -> MlibModule {
        MlibModule {
            functions: module.functions.iter().map(Self::convert_function).collect(),
            strings: Vec::new(),
            types: Vec::new(),
        }
    }
    
    fn convert_function(func: &Function) -> MlibFunction {
        MlibFunction {
            name: func.name.name.clone(),
            arg_count: func.arg_count as u32,
            is_async: func.is_async,
            values: func.values.iter().enumerate().map(|(i, v)| MlibValue {
                id: i as u32,
                inst: Self::convert_instruction(&v.inst),
            }).collect(),
            blocks: func.blocks.iter().enumerate().map(|(i, b)| MlibBlock {
                id: i as u32,
                label: b.label.name.clone(),
                insts: b.insts.iter().map(|v| v.0).collect(),
                terminator: b.terminator.as_ref().map(Self::convert_terminator),
            }).collect(),
        }
    }
    
    fn convert_instruction(inst: &Instruction) -> MlibInstruction {
        match inst {
            Instruction::BoxNew { value } => MlibInstruction::BoxNew { value: Self::convert_operand(value) },
            Instruction::MarkInit { value } => MlibInstruction::MarkInit {
                value: Self::convert_operand(value),
            },
            Instruction::BoxFree { value } => MlibInstruction::BoxFree { value: Self::convert_operand(value) },

            Instruction::Drop { value, .. } => MlibInstruction::Drop { value: Self::convert_operand(value) },
            Instruction::Alloca => MlibInstruction::Alloca,
            Instruction::HeapAlloc => MlibInstruction::HeapAlloc,
            Instruction::Assign(val) => MlibInstruction::Assign(Self::convert_operand(val)),
            Instruction::Store { ptr, value } => MlibInstruction::Store {
                ptr: match ptr {
                    Operand::Value(v) => v.0,
                    _ => panic!("Store ptr must be a Value"),
                },
                value: Self::convert_operand(value),
            },
            Instruction::Load { ptr, .. } => MlibInstruction::Load {
                ptr: Self::convert_operand(ptr),
            },
            Instruction::CallDirect { callee, args, .. } => MlibInstruction::CallDirect {
                callee: callee.name.clone(),
                args: args.iter().map(Self::convert_operand).collect(),
            },
            Instruction::CallIndirect { callee, args, .. } => MlibInstruction::CallIndirect {
                callee: Self::convert_operand(callee),
                args: args.iter().map(Self::convert_operand).collect(),
            },
            Instruction::CallClosure { closure, args, .. } => MlibInstruction::CallClosure {
                closure: Self::convert_operand(closure),
                args: args.iter().map(Self::convert_operand).collect(),
            },
            Instruction::MakeClosure { func, env_ptr, captures } => MlibInstruction::MakeClosure {
                func: func.name.clone(),
                env_ptr: Self::convert_operand(env_ptr),
                captures: captures.iter().map(|capture| crate::ir::MlibCaptureInfo {
                    symbol: capture.symbol.0,
                    source: capture.source.0,
                    env_field: capture.env_field,
                    mode: match capture.mode {
                        CaptureMode::SharedBorrow => 0,
                        CaptureMode::MutableBorrow => 1,
                        CaptureMode::Move => 2,
                    },
                    ty: capture.ty.0,
                    env_ty: capture.env_ty.0,
                }).collect(),
            },
            Instruction::CallVirt { obj, method_idx, args } => MlibInstruction::CallVirt {
                obj: Self::convert_operand(obj),
                method_idx: *method_idx,
                args: args.iter().map(Self::convert_operand).collect(),
            },
            Instruction::MakeTraitObject { data_ptr, vtable, trait_sym, concrete_sym } => MlibInstruction::MakeTraitObject {
                data_ptr: Self::convert_operand(data_ptr),
                vtable: vtable.name.clone(),
                trait_sym: trait_sym.0,
                concrete_sym: concrete_sym.0,
            },
            Instruction::MakeSlice { data_ptr, len } => MlibInstruction::MakeSlice {
                data_ptr: Self::convert_operand(data_ptr),
                len: Self::convert_operand(len),
            },
            Instruction::DropVirt { obj } => MlibInstruction::DropVirt {
                obj: Self::convert_operand(obj),
            },
            Instruction::Add { left, right, .. } => MlibInstruction::Add {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::BoundsCheck { index, len } => {
                // Not supported in serialized format yet, or just map to dummy
                MlibInstruction::CallDirect { 
                    callee: "__mellis_bounds_fail".to_string(),
                    args: vec![Self::convert_operand(index), Self::convert_operand(len)]
                }
            }
            Instruction::CallIntrinsic { kind, args } => {
                MlibInstruction::CallDirect {
                    callee: format!("__mellis_intrinsic_{:?}", kind),
                    args: args.iter().map(Self::convert_operand).collect()
                }
            }
            Instruction::Sub { left, right, .. } => MlibInstruction::Sub {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::Mul { left, right, .. } => MlibInstruction::Mul {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::Div { left, right, .. } => MlibInstruction::Div {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::Rem { left, right, .. } => MlibInstruction::Rem {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::Eq { left, right, .. } => MlibInstruction::Eq {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::NotEq { left, right, .. } => MlibInstruction::NotEq {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::LessThan { left, right } => MlibInstruction::LessThan {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::LessOrEq { left, right } => MlibInstruction::LessOrEq {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::GreaterThan { left, right } => MlibInstruction::GreaterThan {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::GreaterOrEq { left, right } => MlibInstruction::GreaterOrEq {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::BitAnd { left, right } => MlibInstruction::BitAnd {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::BitOr { left, right } => MlibInstruction::BitOr {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::BitXor { left, right } => MlibInstruction::BitXor {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::Shl { left, right } => MlibInstruction::Shl {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::Shr { left, right } => MlibInstruction::Shr {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::Borrow { is_rw, base } => MlibInstruction::Borrow {
                is_rw: *is_rw,
                base: Self::convert_operand(base),
            },
            Instruction::Variant { enum_ty, variant_idx, args } => MlibInstruction::Variant {
                enum_ty: enum_ty.0,
                variant_idx: *variant_idx,
                args: args.iter().map(Self::convert_operand).collect(),
            },
            Instruction::Tag { value } => MlibInstruction::Tag {
                value: Self::convert_operand(value),
            },
            Instruction::Null { ty } => MlibInstruction::Null {
                ty: ty.0,
            },
            Instruction::Nop => MlibInstruction::Nop,
            Instruction::SizeOf { ty } => MlibInstruction::SizeOf {
                ty: ty.0,
            },
            Instruction::AlignOf { ty } => MlibInstruction::AlignOf {
                ty: ty.0,
            },
            Instruction::Cast { value: ptr, target_ty } => MlibInstruction::Cast {
                value: Self::convert_operand(ptr),
                ty: target_ty.0,
            },
            Instruction::PtrOffset { ptr, offset } => MlibInstruction::PtrOffset {
                base: Self::convert_operand(ptr),
                offset: Self::convert_operand(offset),
            },
            Instruction::Extract { value, variant_idx, field_idx } => MlibInstruction::Extract {
                value: Self::convert_operand(value),
                variant_idx: *variant_idx,
                field_idx: *field_idx,
            },
            Instruction::FieldPtr { base, field_idx } => MlibInstruction::FieldPtr {
                base: Self::convert_operand(base),
                field_idx: *field_idx,
            },
            Instruction::Await { future } => MlibInstruction::Await {
                future: Self::convert_operand(future),
            },
        }
    }
    
    fn convert_terminator(term: &Terminator) -> MlibTerminator {
        match term {
            Terminator::Br { target } => MlibTerminator::Br {
                // We'll need to map block labels back to block IDs or store string names.
                // For simplicity in the IR, we store the ID or we can just parse the label if it's strictly ordered.
                // Let's just assume we can map it. In real usage, targets should be BlockIds.
                // Our current MVIR uses BlockLabel. Let's just store 0 for now and fix it if we need to.
                // Wait, MVIR uses target: BlockLabel. The index isn't directly available.
                // We can parse the name like "entry0".
                target: target.name.replace("entry", "").parse().unwrap_or(0),
            },
            Terminator::CondBr { condition, true_target, false_target } => MlibTerminator::CondBr {
                condition: Self::convert_operand(condition),
                true_target: true_target.name.replace("entry", "").parse().unwrap_or(0),
                false_target: false_target.name.replace("entry", "").parse().unwrap_or(0),
            },
            Terminator::Ret { value } => MlibTerminator::Ret {
                value: value.as_ref().map(Self::convert_operand),
            },
            Terminator::Unreachable => MlibTerminator::Unreachable,
            Terminator::MissingReturn => MlibTerminator::MissingReturn,
        }
    }
    
    fn convert_operand(op: &Operand) -> MlibOperand {
        match op {
            Operand::Value(val_id) => MlibOperand::Value(val_id.0),
            Operand::Number(n) => MlibOperand::Number(n.clone()),
            Operand::Boolean(b) => MlibOperand::Boolean(*b),
            Operand::Block(id) => MlibOperand::Block(id.0),
            Operand::Global(g) => MlibOperand::Global(g.name.clone()),
            Operand::StringRef(s) => MlibOperand::StringRef(s.clone()),
            Operand::Char(c) => MlibOperand::Char(c.clone()),
        }
    }

    fn write_string<W: Write>(w: &mut W, s: &str) -> std::io::Result<()> {
        let bytes = s.as_bytes();
        w.write_all(&(bytes.len() as u32).to_le_bytes())?;
        w.write_all(bytes)?;
        Ok(())
    }

    fn serialize_operand<W: Write>(w: &mut W, op: &MlibOperand) -> std::io::Result<()> {
        match op {
            MlibOperand::Value(v) => {
                w.write_all(&[0u8])?;
                w.write_all(&v.to_le_bytes())?;
            }
            MlibOperand::Number(n) => {
                w.write_all(&[1u8])?;
                Self::write_string(w, n)?;
            }
            MlibOperand::Boolean(b) => {
                w.write_all(&[2u8])?;
                w.write_all(&[if *b { 1 } else { 0 }])?;
            }
            MlibOperand::Block(id) => {
                w.write_all(&[3u8])?;
                w.write_all(&id.to_le_bytes())?;
            }
            MlibOperand::Global(g) => {
                w.write_all(&[4u8])?;
                Self::write_string(w, g)?;
            }
            MlibOperand::StringRef(s) => {
                w.write_all(&[5u8])?;
                Self::write_string(w, s)?;
            }
            MlibOperand::Char(c) => {
                w.write_all(&[6u8])?;
                Self::write_string(w, c)?;
            }
        }
        Ok(())
    }

    fn serialize_instruction<W: Write>(w: &mut W, inst: &MlibInstruction) -> std::io::Result<()> {
        match inst {
            MlibInstruction::Alloca => {
                w.write_all(&[0u8])?;
            }
            MlibInstruction::HeapAlloc => {
                w.write_all(&[0x20u8])?;
            }
            MlibInstruction::Assign(op) => {
                w.write_all(&[1u8])?;
                Self::serialize_operand(w, op)?;
            }
            MlibInstruction::Store { ptr, value } => {
                w.write_all(&[2u8])?;
                w.write_all(&ptr.to_le_bytes())?;
                Self::serialize_operand(w, value)?;
            }
            MlibInstruction::Load { ptr } => {
                w.write_all(&[3u8])?;
                Self::serialize_operand(w, ptr)?;
            }
            MlibInstruction::CallDirect { callee, args } => {
                w.write_all(&[4u8])?;
                Self::write_string(w, callee)?;
                w.write_all(&(args.len() as u32).to_le_bytes())?;
                for arg in args {
                    Self::serialize_operand(w, arg)?;
                }
            }
            MlibInstruction::CallIndirect { callee, args } => {
                w.write_all(&[28u8])?;
                Self::serialize_operand(w, callee)?;
                w.write_all(&(args.len() as u32).to_le_bytes())?;
                for arg in args {
                    Self::serialize_operand(w, arg)?;
                }
            }
            MlibInstruction::CallClosure { closure, args } => {
                w.write_all(&[29u8])?;
                Self::serialize_operand(w, closure)?;
                w.write_all(&(args.len() as u32).to_le_bytes())?;
                for arg in args {
                    Self::serialize_operand(w, arg)?;
                }
            }
            MlibInstruction::MakeClosure { func, env_ptr, captures } => {
                w.write_all(&[0x23u8])?;
                Self::write_string(w, func)?;
                Self::serialize_operand(w, env_ptr)?;
                w.write_all(&(captures.len() as u32).to_le_bytes())?;
                for capture in captures {
                    w.write_all(&capture.symbol.to_le_bytes())?;
                    w.write_all(&capture.source.to_le_bytes())?;
                    w.write_all(&capture.env_field.to_le_bytes())?;
                    w.write_all(&[capture.mode])?;
                    w.write_all(&capture.ty.to_le_bytes())?;
                    w.write_all(&capture.env_ty.to_le_bytes())?;
                }
            }
            MlibInstruction::CallVirt { obj, method_idx, args } => {
                w.write_all(&[0x24u8])?;
                Self::serialize_operand(w, obj)?;
                w.write_all(&method_idx.to_le_bytes())?;
                w.write_all(&(args.len() as u32).to_le_bytes())?;
                for arg in args {
                    Self::serialize_operand(w, arg)?;
                }
            }
            MlibInstruction::MakeTraitObject { data_ptr, vtable, trait_sym, concrete_sym } => {
                w.write_all(&[0x25u8])?;
                Self::serialize_operand(w, data_ptr)?;
                Self::write_string(w, vtable)?;
                w.write_all(&trait_sym.to_le_bytes())?;
                w.write_all(&concrete_sym.to_le_bytes())?;
            }
            MlibInstruction::MakeSlice { data_ptr, len } => {
                w.write_all(&[0x6Eu8])?;
                Self::serialize_operand(w, data_ptr)?;
                Self::serialize_operand(w, len)?;
            }
            MlibInstruction::DropVirt { obj } => {
                w.write_all(&[0x6Fu8])?;
                Self::serialize_operand(w, obj)?;
            }
            MlibInstruction::Add { left, right } => {
                w.write_all(&[5u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::Sub { left, right } => {
                w.write_all(&[6u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::Mul { left, right } => {
                w.write_all(&[7u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::Div { left, right } => {
                w.write_all(&[63u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::Rem { left, right } => {
                w.write_all(&[64u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::Eq { left, right } => {
                w.write_all(&[8u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::NotEq { left, right } => {
                w.write_all(&[42])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::LessThan { left, right } => {
                w.write_all(&[22u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::LessOrEq { left, right } => {
                w.write_all(&[65u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::GreaterThan { left, right } => {
                w.write_all(&[66u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::GreaterOrEq { left, right } => {
                w.write_all(&[67u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::BitAnd { left, right } => {
                w.write_all(&[68u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::BitOr { left, right } => {
                w.write_all(&[69u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::BitXor { left, right } => {
                w.write_all(&[70u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::Shl { left, right } => {
                w.write_all(&[71u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::Shr { left, right } => {
                w.write_all(&[72u8])?;
                Self::serialize_operand(w, left)?;
                Self::serialize_operand(w, right)?;
            }
            MlibInstruction::Borrow { is_rw, base } => {
                w.write_all(&[9u8])?;
                w.write_all(&[if *is_rw { 1 } else { 0 }])?;
                Self::serialize_operand(w, base)?;
            }
            MlibInstruction::Variant { enum_ty, variant_idx, args } => {
                w.write_all(&[10u8])?;
                w.write_all(&enum_ty.to_le_bytes())?;
                w.write_all(&variant_idx.to_le_bytes())?;
                w.write_all(&(args.len() as u32).to_le_bytes())?;
                for arg in args {
                    Self::serialize_operand(w, arg)?;
                }
            }
            MlibInstruction::Tag { value } => {
                w.write_all(&[11u8])?;
                Self::serialize_operand(w, value)?;
            }
            MlibInstruction::Extract { value, variant_idx, field_idx } => {
                w.write_all(&[12u8])?;
                Self::serialize_operand(w, value)?;
                w.write_all(&variant_idx.to_le_bytes())?;
                w.write_all(&field_idx.to_le_bytes())?;
            }
            MlibInstruction::FieldPtr { base, field_idx } => {
                w.write_all(&[27u8])?;
                Self::serialize_operand(w, base)?;
                w.write_all(&field_idx.to_le_bytes())?;
            }
            MlibInstruction::Drop { value } => {
                w.write_all(&[13u8])?;
                Self::serialize_operand(w, value)?;
            }
            MlibInstruction::BoxNew { value } => {
                w.write_all(&[14u8])?;
                Self::serialize_operand(w, value)?;
            }
            MlibInstruction::MarkInit { value } => {
                w.write_all(&[0x21u8])?;
                Self::serialize_operand(w, value)?;
            }
            MlibInstruction::BoxFree { value } => {
                w.write_all(&[17u8])?;
                Self::serialize_operand(w, value)?;
            }
            MlibInstruction::ListNew => {
                w.write_all(&[18u8])?;
            }
            MlibInstruction::SizeOf { ty } => {
                w.write_all(&[23u8])?;
                w.write_all(&ty.to_le_bytes())?;
            }
            MlibInstruction::AlignOf { ty } => {
                w.write_all(&[24u8])?;
                w.write_all(&ty.to_le_bytes())?;
            }
            MlibInstruction::Null { ty } => {
                w.write_all(&[0x22u8])?;
                w.write_all(&ty.to_le_bytes())?;
            }
            MlibInstruction::Nop => {
                w.write_all(&[0x24u8])?; // Assuming 0x24 is unused (0x23 is Await probably?)
            }
            MlibInstruction::Cast { value, ty } => {
                w.write_all(&[25u8])?;
                Self::serialize_operand(w, value)?;
                w.write_all(&ty.to_le_bytes())?;
            }
            MlibInstruction::PtrOffset { base, offset } => {
                w.write_all(&[26u8])?;
                Self::serialize_operand(w, base)?;
                Self::serialize_operand(w, offset)?;
            }
            MlibInstruction::ListPush { list, value } => {
                w.write_all(&[19u8])?;
                Self::serialize_operand(w, list)?;
                Self::serialize_operand(w, value)?;
            }
            MlibInstruction::ListGet { list, index, is_mut } => {
                w.write_all(&[20u8])?;
                Self::serialize_operand(w, list)?;
                Self::serialize_operand(w, index)?;
                w.write_all(&[if *is_mut { 1 } else { 0 }])?;
            }
            MlibInstruction::Await { future } => {
                w.write_all(&[0x26u8])?;
                Self::serialize_operand(w, future)?;
            }
        }
        Ok(())
    }

    fn serialize_terminator<W: Write>(w: &mut W, term: &MlibTerminator) -> std::io::Result<()> {
        match term {
            MlibTerminator::Br { target } => {
                w.write_all(&[0u8])?;
                w.write_all(&target.to_le_bytes())?;
            }
            MlibTerminator::CondBr { condition, true_target, false_target } => {
                w.write_all(&[1u8])?;
                Self::serialize_operand(w, condition)?;
                w.write_all(&true_target.to_le_bytes())?;
                w.write_all(&false_target.to_le_bytes())?;
            }
            MlibTerminator::Ret { value } => {
                w.write_all(&[2u8])?;
                if let Some(val) = value {
                    w.write_all(&[1u8])?;
                    Self::serialize_operand(w, val)?;
                } else {
                    w.write_all(&[0u8])?;
                }
            }
            MlibTerminator::Unreachable => {
                w.write_all(&[4u8])?;
            }
            MlibTerminator::MissingReturn => {
                w.write_all(&[5u8])?;
            }
        }
        Ok(())
    }

    fn serialize_block<W: Write>(w: &mut W, block: &MlibBlock) -> std::io::Result<()> {
        w.write_all(&block.id.to_le_bytes())?;
        Self::write_string(w, &block.label)?;
        w.write_all(&(block.insts.len() as u32).to_le_bytes())?;
        for inst_id in &block.insts {
            w.write_all(&inst_id.to_le_bytes())?;
        }
        if let Some(term) = &block.terminator {
            w.write_all(&[1u8])?;
            Self::serialize_terminator(w, term)?;
        } else {
            w.write_all(&[0u8])?;
        }
        Ok(())
    }

    fn serialize_value<W: Write>(w: &mut W, value: &MlibValue) -> std::io::Result<()> {
        w.write_all(&value.id.to_le_bytes())?;
        Self::serialize_instruction(w, &value.inst)?;
        Ok(())
    }

    fn serialize_function<W: Write>(w: &mut W, func: &MlibFunction) -> std::io::Result<()> {
        Self::write_string(w, &func.name)?;
        w.write_all(&func.arg_count.to_le_bytes())?;
        w.write_all(&(func.is_async as u8).to_le_bytes())?;
        w.write_all(&(func.values.len() as u32).to_le_bytes())?;
        for val in &func.values {
            Self::serialize_value(w, val)?;
        }
        w.write_all(&(func.blocks.len() as u32).to_le_bytes())?;
        for block in &func.blocks {
            Self::serialize_block(w, block)?;
        }
        Ok(())
    }

    fn serialize_type_entry<W: Write>(w: &mut W, t: &MlibTypeEntry) -> std::io::Result<()> {
        Self::write_string(w, &t.name)?;
        w.write_all(&t.namespace_id.to_le_bytes())?;
        w.write_all(&t.size.to_le_bytes())?;
        w.write_all(&t.alignment.to_le_bytes())?;
        w.write_all(&[t.visibility])?;
        w.write_all(&t.module_id.to_le_bytes())?;
        Ok(())
    }

    fn serialize_module_internal<W: Write>(w: &mut W, module: &MlibModule) -> std::io::Result<()> {
        w.write_all(&(module.functions.len() as u32).to_le_bytes())?;
        for func in &module.functions {
            Self::serialize_function(w, func)?;
        }
        w.write_all(&(module.strings.len() as u32).to_le_bytes())?;
        for s in &module.strings {
            Self::write_string(w, s)?;
        }
        w.write_all(&(module.types.len() as u32).to_le_bytes())?;
        for t in &module.types {
            Self::serialize_type_entry(w, t)?;
        }
        Ok(())
    }
}
