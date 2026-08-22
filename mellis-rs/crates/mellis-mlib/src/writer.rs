use std::io::{Write, Cursor};
use crate::format::{MlibHeader, SectionEntry, SectionType};
use crate::ir::{MlibModule, MlibFunction, MlibValue, MlibBlock, MlibInstruction, MlibTerminator, MlibOperand, MlibTypeEntry};
use mellis_mvir::{Module, Function, ValueData, BasicBlock, Instruction, Terminator, Operand};

pub struct MlibWriter;

impl MlibWriter {
    pub fn write_module<W: Write>(module: &Module, writer: &mut W) -> std::io::Result<()> {
        let mlib_module = Self::convert_module(module);
        
        let mut mvir_payload = Vec::new();
        Self::serialize_module_internal(&mut mvir_payload, &mlib_module)?;

        let mut header = MlibHeader::new();
        header.section_count = 1;
        
        let header_size = 122;
        let section_table_size = 40;
        let mvir_offset = (header_size + section_table_size) as u64;
        
        header.section_table_offset = header_size as u64;

        // Write header
        header.write_to(writer)?;
        
        let mvir_section = SectionEntry {
            section_id: 1,
            section_type: SectionType::GenericMVIR,
            offset: mvir_offset,
            size: mvir_payload.len() as u64,
            version: 1,
            compression: 0,
            reserved: [0u8; 5],
            hash: 0,
        };
        
        // Write section table
        mvir_section.write_to(writer)?;
        
        // Write payloads
        writer.write_all(&mvir_payload)?;
        
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
            Instruction::Alloca => MlibInstruction::Alloca,
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
            Instruction::Call { callee, args, .. } => MlibInstruction::Call {
                callee: Self::convert_operand(callee),
                args: args.iter().map(Self::convert_operand).collect(),
            },
            Instruction::Add { left, right, .. } => MlibInstruction::Add {
                left: Self::convert_operand(left),
                right: Self::convert_operand(right),
            },
            Instruction::BoundsCheck { index, len } => {
                // Not supported in serialized format yet, or just map to dummy
                MlibInstruction::Call { 
                    callee: crate::ir::MlibOperand::Global("__mellis_bounds_fail".to_string()),
                    args: vec![Self::convert_operand(index), Self::convert_operand(len)]
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
            Instruction::Eq { left, right, .. } => MlibInstruction::Eq {
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
            Instruction::Extract { value, variant_idx, field_idx } => MlibInstruction::Extract {
                value: Self::convert_operand(value),
                variant_idx: *variant_idx,
                field_idx: *field_idx,
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
            Terminator::Unreachable => MlibTerminator::Ret { value: None }, // For now, or add Unreachable to MlibTerminator
        }
    }
    
    fn convert_operand(op: &Operand) -> MlibOperand {
        match op {
            Operand::Value(val_id) => MlibOperand::Value(val_id.0),
            Operand::Number(n) => MlibOperand::Number(n.clone()),
            Operand::Boolean(b) => MlibOperand::Boolean(*b),
            Operand::Block(id) => MlibOperand::Block(id.0),
            Operand::Global(g) => MlibOperand::Global(g.name.clone()),
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
        }
        Ok(())
    }

    fn serialize_instruction<W: Write>(w: &mut W, inst: &MlibInstruction) -> std::io::Result<()> {
        match inst {
            MlibInstruction::Alloca => {
                w.write_all(&[0u8])?;
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
            MlibInstruction::Call { callee, args } => {
                w.write_all(&[4u8])?;
                Self::serialize_operand(w, callee)?;
                w.write_all(&(args.len() as u32).to_le_bytes())?;
                for arg in args {
                    Self::serialize_operand(w, arg)?;
                }
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
            MlibInstruction::Eq { left, right } => {
                w.write_all(&[8u8])?;
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
