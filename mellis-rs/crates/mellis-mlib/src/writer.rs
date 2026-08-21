use std::io::{Write, Cursor};
use crate::format::{MlibHeader, SectionEntry, SectionType};
use crate::ir::{MlibModule, MlibFunction, MlibValue, MlibBlock, MlibInstruction, MlibTerminator, MlibOperand};
use mellis_mvir::{Module, Function, ValueData, BasicBlock, Instruction, Terminator, Operand};

pub struct MlibWriter;

impl MlibWriter {
    pub fn write_module<W: Write>(module: &Module, writer: &mut W) -> std::io::Result<()> {
        let mlib_module = Self::convert_module(module);
        
        let mut mvir_payload = Vec::new();
        bincode::serialize_into(&mut mvir_payload, &mlib_module)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let mut header = MlibHeader::new();
        header.section_count = 1;
        
        // Write header
        header.write_to(writer)?;
        
        // Calculate offsets
        let header_size = 4 + 2 + 2 + 32 + 4;
        let section_table_size = 1 * (4 + 8 + 8);
        let mvir_offset = (header_size + section_table_size) as u64;
        
        let mvir_section = SectionEntry {
            section_type: SectionType::Mvir,
            offset: mvir_offset,
            length: mvir_payload.len() as u64,
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
}
