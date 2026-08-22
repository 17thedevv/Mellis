use std::io::{Read, Seek, SeekFrom};
use crate::format::{MlibHeader, SectionEntry, SectionType, MLIB_MAGIC, MLIB_FORMAT_VERSION};
use crate::ir::{MlibModule, MlibFunction, MlibValue, MlibBlock, MlibInstruction, MlibTerminator, MlibOperand, MlibTypeEntry};

#[derive(Debug)]
pub enum MlibError {
    Io(std::io::Error),
    InvalidMagic,
    VersionMismatch(u16),
    UnknownSection(u32),
    CorruptedData,
}

impl From<std::io::Error> for MlibError {
    fn from(err: std::io::Error) -> Self {
        MlibError::Io(err)
    }
}

pub struct MlibReader;

impl MlibReader {
    pub fn read_module<R: Read + Seek>(reader: &mut R) -> Result<MlibModule, MlibError> {
        let header = MlibHeader::read_from(reader)?;
        
        if header.magic != MLIB_MAGIC {
            return Err(MlibError::InvalidMagic);
        }
        
        if header.format_version != MLIB_FORMAT_VERSION {
            return Err(MlibError::VersionMismatch(header.format_version));
        }
        
        reader.seek(SeekFrom::Start(header.section_table_offset))?;

        let mut sections = Vec::new();
        for _ in 0..header.section_count {
            let section = SectionEntry::read_from(reader)?;
            sections.push(section);
        }
        
        let mut raw_string_table = Vec::new();

        // Pass 1: String Table
        for section in &sections {
            if section.section_type == SectionType::StringTable {
                reader.seek(SeekFrom::Start(section.offset))?;
                let mut data = vec![0u8; section.size as usize];
                reader.read_exact(&mut data)?;
                raw_string_table = data;
                break;
            }
        }
        
        let mut mlib_module = MlibModule::default();
        
        // Helper to extract string by offset
        let get_string = |offset: u32| -> String {
            if offset as usize >= raw_string_table.len() {
                return String::new();
            }
            let slice = &raw_string_table[offset as usize..];
            let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
            String::from_utf8_lossy(&slice[..end]).into_owned()
        };

        // Pass 2: Metadata and MVIR
        for section in &sections {
            match section.section_type {
                SectionType::TypeMetadata => {
                    reader.seek(SeekFrom::Start(section.offset))?;
                    // Read TypeMetadata header/entries
                    // Wait, TypeMetadata section layout:
                    // uint32_t count;
                    // count * TypeEntry
                    
                    let mut count_buf = [0u8; 4];
                    reader.read_exact(&mut count_buf)?;
                    let count = u32::from_le_bytes(count_buf);
                    
                    for _ in 0..count {
                        let mut entry_buf = [0u8; 32];
                        reader.read_exact(&mut entry_buf)?;
                        
                        // TypeEntry layout:
                        // uint32_t nameStringID; 0-3
                        // uint32_t namespaceID; 4-7
                        // uint64_t size; 8-15
                        // uint64_t alignment; 16-23
                        // uint8_t  visibility; 24
                        // uint8_t pad[3]? wait, moduleID is uint32_t.
                        // Wait, C++ struct layout:
                        // uint32_t nameStringID (4)
                        // uint32_t namespaceID (4)
                        // uint64_t size (8)
                        // uint64_t alignment (8)
                        // uint8_t visibility (1)
                        // padding (3 bytes for alignment to 4) -> wait, moduleID is uint32_t.
                        // 1 + 3 padding + 4 = 8. Total = 4 + 4 + 8 + 8 + 8 = 32 bytes!
                        
                        let name_id = u32::from_le_bytes(entry_buf[0..4].try_into().unwrap());
                        let namespace_id = u32::from_le_bytes(entry_buf[4..8].try_into().unwrap());
                        let size = u64::from_le_bytes(entry_buf[8..16].try_into().unwrap());
                        let alignment = u64::from_le_bytes(entry_buf[16..24].try_into().unwrap());
                        let visibility = entry_buf[24];
                        let module_id = u32::from_le_bytes(entry_buf[28..32].try_into().unwrap());
                        
                        mlib_module.types.push(crate::ir::MlibTypeEntry {
                            name: get_string(name_id),
                            namespace_id,
                            size,
                            alignment,
                            visibility,
                            module_id,
                        });
                    }
                }
                SectionType::GenericMVIR => {
                    reader.seek(SeekFrom::Start(section.offset))?;
                    let mut data = vec![0u8; section.size as usize];
                    reader.read_exact(&mut data)?;
                    let mut cursor = std::io::Cursor::new(data);
                    let m = Self::deserialize_module_internal(&mut cursor)
                        .map_err(|_| MlibError::CorruptedData)?;
                    mlib_module.functions = m.functions;
                    if !m.strings.is_empty() {
                        mlib_module.strings = m.strings;
                    }
                    if mlib_module.types.is_empty() && !m.types.is_empty() {
                        mlib_module.types = m.types;
                    }
                }
                SectionType::StringTable => {} // Already parsed
                _ => {
                    // Unknown section, skip it
                }
            }
        }
        
        println!("Found {} sections", sections.len());
        println!("String table raw size: {}", raw_string_table.len());
        println!("Found {} types", mlib_module.types.len());

        Ok(mlib_module)
    }

    fn read_string<R: Read>(r: &mut R) -> std::io::Result<String> {
        let mut len_buf = [0u8; 4];
        r.read_exact(&mut len_buf)?;
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut bytes = vec![0u8; len];
        r.read_exact(&mut bytes)?;
        String::from_utf8(bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn deserialize_operand<R: Read>(r: &mut R) -> std::io::Result<MlibOperand> {
        let mut tag_buf = [0u8; 1];
        r.read_exact(&mut tag_buf)?;
        match tag_buf[0] {
            0 => {
                let mut v_buf = [0u8; 4];
                r.read_exact(&mut v_buf)?;
                Ok(MlibOperand::Value(u32::from_le_bytes(v_buf)))
            }
            1 => {
                let s = Self::read_string(r)?;
                Ok(MlibOperand::Number(s))
            }
            2 => {
                let mut b_buf = [0u8; 1];
                r.read_exact(&mut b_buf)?;
                Ok(MlibOperand::Boolean(b_buf[0] != 0))
            }
            3 => {
                let mut id_buf = [0u8; 4];
                r.read_exact(&mut id_buf)?;
                Ok(MlibOperand::Block(u32::from_le_bytes(id_buf)))
            }
            4 => {
                let s = Self::read_string(r)?;
                Ok(MlibOperand::Global(s))
            }
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid operand tag")),
        }
    }

    fn deserialize_instruction<R: Read>(r: &mut R) -> std::io::Result<MlibInstruction> {
        let mut tag_buf = [0u8; 1];
        r.read_exact(&mut tag_buf)?;
        match tag_buf[0] {
            0 => Ok(MlibInstruction::Alloca),
            1 => {
                let op = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Assign(op))
            }
            2 => {
                let mut ptr_buf = [0u8; 4];
                r.read_exact(&mut ptr_buf)?;
                let ptr = u32::from_le_bytes(ptr_buf);
                let value = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Store { ptr, value })
            }
            3 => {
                let ptr = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Load { ptr })
            }
            4 => {
                let callee = Self::deserialize_operand(r)?;
                let mut count_buf = [0u8; 4];
                r.read_exact(&mut count_buf)?;
                let count = u32::from_le_bytes(count_buf) as usize;
                let mut args = Vec::with_capacity(count);
                for _ in 0..count {
                    args.push(Self::deserialize_operand(r)?);
                }
                Ok(MlibInstruction::Call { callee, args })
            }
            5 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Add { left, right })
            }
            6 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Sub { left, right })
            }
            7 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Mul { left, right })
            }
            8 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Eq { left, right })
            }
            9 => {
                let mut rw_buf = [0u8; 1];
                r.read_exact(&mut rw_buf)?;
                let base = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Borrow { is_rw: rw_buf[0] != 0, base })
            }
            10 => {
                let mut ty_buf = [0u8; 4];
                r.read_exact(&mut ty_buf)?;
                let enum_ty = u32::from_le_bytes(ty_buf);
                let mut idx_buf = [0u8; 4];
                r.read_exact(&mut idx_buf)?;
                let variant_idx = u32::from_le_bytes(idx_buf);
                let mut count_buf = [0u8; 4];
                r.read_exact(&mut count_buf)?;
                let count = u32::from_le_bytes(count_buf) as usize;
                let mut args = Vec::with_capacity(count);
                for _ in 0..count {
                    args.push(Self::deserialize_operand(r)?);
                }
                Ok(MlibInstruction::Variant { enum_ty, variant_idx, args })
            }
            11 => {
                let value = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Tag { value })
            }
            12 => {
                let value = Self::deserialize_operand(r)?;
                let mut var_buf = [0u8; 4];
                r.read_exact(&mut var_buf)?;
                let variant_idx = u32::from_le_bytes(var_buf);
                let mut fld_buf = [0u8; 4];
                r.read_exact(&mut fld_buf)?;
                let field_idx = u32::from_le_bytes(fld_buf);
                Ok(MlibInstruction::Extract { value, variant_idx, field_idx })
            }
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid instruction tag")),
        }
    }

    fn deserialize_terminator<R: Read>(r: &mut R) -> std::io::Result<MlibTerminator> {
        let mut tag_buf = [0u8; 1];
        r.read_exact(&mut tag_buf)?;
        match tag_buf[0] {
            0 => {
                let mut target_buf = [0u8; 4];
                r.read_exact(&mut target_buf)?;
                Ok(MlibTerminator::Br { target: u32::from_le_bytes(target_buf) })
            }
            1 => {
                let condition = Self::deserialize_operand(r)?;
                let mut t_buf = [0u8; 4];
                r.read_exact(&mut t_buf)?;
                let true_target = u32::from_le_bytes(t_buf);
                let mut f_buf = [0u8; 4];
                r.read_exact(&mut f_buf)?;
                let false_target = u32::from_le_bytes(f_buf);
                Ok(MlibTerminator::CondBr { condition, true_target, false_target })
            }
            2 => {
                let mut has_val_buf = [0u8; 1];
                r.read_exact(&mut has_val_buf)?;
                let value = if has_val_buf[0] != 0 {
                    Some(Self::deserialize_operand(r)?)
                } else {
                    None
                };
                Ok(MlibTerminator::Ret { value })
            }
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid terminator tag")),
        }
    }

    fn deserialize_block<R: Read>(r: &mut R) -> std::io::Result<MlibBlock> {
        let mut id_buf = [0u8; 4];
        r.read_exact(&mut id_buf)?;
        let id = u32::from_le_bytes(id_buf);
        let label = Self::read_string(r)?;
        let mut insts_count_buf = [0u8; 4];
        r.read_exact(&mut insts_count_buf)?;
        let insts_count = u32::from_le_bytes(insts_count_buf) as usize;
        let mut insts = Vec::with_capacity(insts_count);
        for _ in 0..insts_count {
            let mut inst_id_buf = [0u8; 4];
            r.read_exact(&mut inst_id_buf)?;
            insts.push(u32::from_le_bytes(inst_id_buf));
        }
        let mut has_term_buf = [0u8; 1];
        r.read_exact(&mut has_term_buf)?;
        let terminator = if has_term_buf[0] != 0 {
            Some(Self::deserialize_terminator(r)?)
        } else {
            None
        };
        Ok(MlibBlock { id, label, insts, terminator })
    }

    fn deserialize_value<R: Read>(r: &mut R) -> std::io::Result<MlibValue> {
        let mut id_buf = [0u8; 4];
        r.read_exact(&mut id_buf)?;
        let id = u32::from_le_bytes(id_buf);
        let inst = Self::deserialize_instruction(r)?;
        Ok(MlibValue { id, inst })
    }

    fn deserialize_function<R: Read>(r: &mut R) -> std::io::Result<MlibFunction> {
        let name = Self::read_string(r)?;
        let mut val_count_buf = [0u8; 4];
        r.read_exact(&mut val_count_buf)?;
        let val_count = u32::from_le_bytes(val_count_buf) as usize;
        let mut values = Vec::with_capacity(val_count);
        for _ in 0..val_count {
            values.push(Self::deserialize_value(r)?);
        }
        let mut block_count_buf = [0u8; 4];
        r.read_exact(&mut block_count_buf)?;
        let block_count = u32::from_le_bytes(block_count_buf) as usize;
        let mut blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            blocks.push(Self::deserialize_block(r)?);
        }
        Ok(MlibFunction { name, values, blocks })
    }

    fn deserialize_type_entry<R: Read>(r: &mut R) -> std::io::Result<MlibTypeEntry> {
        let name = Self::read_string(r)?;
        let mut ns_buf = [0u8; 4];
        r.read_exact(&mut ns_buf)?;
        let namespace_id = u32::from_le_bytes(ns_buf);
        let mut sz_buf = [0u8; 8];
        r.read_exact(&mut sz_buf)?;
        let size = u64::from_le_bytes(sz_buf);
        let mut align_buf = [0u8; 8];
        r.read_exact(&mut align_buf)?;
        let alignment = u64::from_le_bytes(align_buf);
        let mut vis_buf = [0u8; 1];
        r.read_exact(&mut vis_buf)?;
        let visibility = vis_buf[0];
        let mut mod_buf = [0u8; 4];
        r.read_exact(&mut mod_buf)?;
        let module_id = u32::from_le_bytes(mod_buf);
        Ok(MlibTypeEntry { name, namespace_id, size, alignment, visibility, module_id })
    }

    fn deserialize_module_internal<R: Read>(r: &mut R) -> std::io::Result<MlibModule> {
        let mut func_count_buf = [0u8; 4];
        r.read_exact(&mut func_count_buf)?;
        let func_count = u32::from_le_bytes(func_count_buf) as usize;
        let mut functions = Vec::with_capacity(func_count);
        for _ in 0..func_count {
            functions.push(Self::deserialize_function(r)?);
        }
        let mut str_count_buf = [0u8; 4];
        r.read_exact(&mut str_count_buf)?;
        let str_count = u32::from_le_bytes(str_count_buf) as usize;
        let mut strings = Vec::with_capacity(str_count);
        for _ in 0..str_count {
            strings.push(Self::read_string(r)?);
        }
        let mut ty_count_buf = [0u8; 4];
        r.read_exact(&mut ty_count_buf)?;
        let ty_count = u32::from_le_bytes(ty_count_buf) as usize;
        let mut types = Vec::with_capacity(ty_count);
        for _ in 0..ty_count {
            types.push(Self::deserialize_type_entry(r)?);
        }
        Ok(MlibModule { functions, strings, types })
    }
}
