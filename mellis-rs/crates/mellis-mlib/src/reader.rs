use std::io::{Read, Seek, SeekFrom};
use crate::format::{MlibHeader, SectionEntry, SectionType, MLIB_MAGIC, MLIB_FORMAT_VERSION};
use crate::ir::{MlibModule, MlibFunction, MlibValue, MlibBlock, MlibInstruction, MlibTerminator, MlibOperand, MlibTypeEntry, MlibCaptureInfo};

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
    pub fn read_manifest<R: Read + Seek>(reader: &mut R) -> Result<crate::format::Manifest, MlibError> {
        let header = MlibHeader::read_from(reader)?;
        if header.magic != MLIB_MAGIC {
            return Err(MlibError::InvalidMagic);
        }
        if header.format_version != MLIB_FORMAT_VERSION {
            return Err(MlibError::VersionMismatch(header.format_version));
        }
        reader.seek(SeekFrom::Start(header.section_table_offset))?;
        for _ in 0..header.section_count {
            let section = SectionEntry::read_from(reader)?;
            if section.section_type == SectionType::Manifest {
                reader.seek(SeekFrom::Start(section.offset))?;
                let mut data = vec![0u8; section.size as usize];
                reader.read_exact(&mut data)?;
                let manifest: crate::format::Manifest = bincode::deserialize(&data)
                    .map_err(|_| MlibError::CorruptedData)?;
                return Ok(manifest);
            }
        }
        Err(MlibError::CorruptedData) // Or MissingManifest
    }

    pub fn read_module<R: Read + Seek>(reader: &mut R) -> Result<(MlibModule, Option<crate::format::Manifest>, Option<Vec<u8>>), MlibError> {
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
            }
        }
        
        let mut mlib_module = MlibModule::default();
        let mut manifest_opt = None;
        let mut obj_bytes_opt = None;
        
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
                SectionType::Manifest => {
                    reader.seek(SeekFrom::Start(section.offset))?;
                    let mut data = vec![0u8; section.size as usize];
                    reader.read_exact(&mut data)?;
                    let manifest: crate::format::Manifest = match bincode::deserialize(&data) {
                        Ok(m) => m,
                        Err(e) => {
                            println!("Failed to deserialize manifest: {:?}", e);
                            return Err(MlibError::CorruptedData);
                        }
                    };
                    manifest_opt = Some(manifest);
                }
                SectionType::ObjectCode => {
                    reader.seek(SeekFrom::Start(section.offset))?;
                    let mut data = vec![0u8; section.size as usize];
                    reader.read_exact(&mut data)?;
                    obj_bytes_opt = Some(data);
                }
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
                    let m = match Self::deserialize_module_internal(&mut cursor) {
                        Ok(m) => m,
                        Err(e) => {
                            println!("Failed to deserialize module internal: {:?}", e);
                            return Err(MlibError::CorruptedData);
                        }
                    };
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

        Ok((mlib_module, manifest_opt, obj_bytes_opt))
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
            5 => {
                let s = Self::read_string(r)?;
                Ok(MlibOperand::StringRef(s))
            }
            6 => {
                let s = Self::read_string(r)?;
                Ok(MlibOperand::Char(s))
            }
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid operand tag")),
        }
    }

    fn deserialize_instruction<R: Read>(r: &mut R) -> std::io::Result<MlibInstruction> {
        let mut tag_buf = [0u8; 1];
        r.read_exact(&mut tag_buf)?;
        match tag_buf[0] {
            0 => Ok(MlibInstruction::Alloca),
            0x20 => Ok(MlibInstruction::HeapAlloc),
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
                let callee = Self::read_string(r)?;
                let mut count_buf = [0u8; 4];
                r.read_exact(&mut count_buf)?;
                let count = u32::from_le_bytes(count_buf) as usize;
                let mut args = Vec::with_capacity(count);
                for _ in 0..count {
                    args.push(Self::deserialize_operand(r)?);
                }
                Ok(MlibInstruction::CallDirect { callee, args })
            }
            28 => {
                let callee = Self::deserialize_operand(r)?;
                let mut count_buf = [0u8; 4];
                r.read_exact(&mut count_buf)?;
                let count = u32::from_le_bytes(count_buf) as usize;
                let mut args = Vec::with_capacity(count);
                for _ in 0..count {
                    args.push(Self::deserialize_operand(r)?);
                }
                Ok(MlibInstruction::CallIndirect { callee, args })
            }
            29 => {
                let closure = Self::deserialize_operand(r)?;
                let mut count_buf = [0u8; 4];
                r.read_exact(&mut count_buf)?;
                let count = u32::from_le_bytes(count_buf) as usize;
                let mut args = Vec::with_capacity(count);
                for _ in 0..count {
                    args.push(Self::deserialize_operand(r)?);
                }
                Ok(MlibInstruction::CallClosure { closure, args })
            }
            0x23 => {
                let mut len_buf = [0u8; 4];
                r.read_exact(&mut len_buf)?;
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut name_buf = vec![0u8; len];
                r.read_exact(&mut name_buf)?;
                let func = String::from_utf8(name_buf).unwrap();
                let env_ptr = Self::deserialize_operand(r)?;
                let mut count_buf = [0u8; 4];
                r.read_exact(&mut count_buf)?;
                let count = u32::from_le_bytes(count_buf) as usize;
                let mut captures = Vec::with_capacity(count);
                for _ in 0..count {
                    let mut symbol_buf = [0u8; 4];
                    let mut source_buf = [0u8; 4];
                    let mut field_buf = [0u8; 4];
                    let mut mode_buf = [0u8; 1];
                    let mut ty_buf = [0u8; 4];
                    let mut env_ty_buf = [0u8; 4];
                    r.read_exact(&mut symbol_buf)?;
                    r.read_exact(&mut source_buf)?;
                    r.read_exact(&mut field_buf)?;
                    r.read_exact(&mut mode_buf)?;
                    r.read_exact(&mut ty_buf)?;
                    r.read_exact(&mut env_ty_buf)?;
                    if mode_buf[0] > 2 {
                        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid closure capture mode"));
                    }
                    captures.push(MlibCaptureInfo {
                        symbol: u32::from_le_bytes(symbol_buf),
                        source: u32::from_le_bytes(source_buf),
                        env_field: u32::from_le_bytes(field_buf),
                        mode: mode_buf[0],
                        ty: u32::from_le_bytes(ty_buf),
                        env_ty: u32::from_le_bytes(env_ty_buf),
                    });
                }
                Ok(MlibInstruction::MakeClosure { func, env_ptr, captures })
            }
            0x24 => {
                let obj = Self::deserialize_operand(r)?;
                let mut idx_buf = [0u8; 4];
                r.read_exact(&mut idx_buf)?;
                let method_idx = u32::from_le_bytes(idx_buf);
                let mut count_buf = [0u8; 4];
                r.read_exact(&mut count_buf)?;
                let count = u32::from_le_bytes(count_buf) as usize;
                let mut args = Vec::with_capacity(count);
                for _ in 0..count {
                    args.push(Self::deserialize_operand(r)?);
                }
                Ok(MlibInstruction::CallVirt { obj, method_idx, args })
            }
            0x25 => {
                let data_ptr = Self::deserialize_operand(r)?;
                let vtable = Self::read_string(r)?;
                let mut trait_buf = [0u8; 4];
                r.read_exact(&mut trait_buf)?;
                let trait_sym = u32::from_le_bytes(trait_buf);
                Ok(MlibInstruction::MakeTraitObject { data_ptr, vtable, trait_sym })
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
            63 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Div { left, right })
            }
            64 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Rem { left, right })
            }
            8 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Eq { left, right })
            }
            42 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::NotEq { left, right })
            }
            17 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Eq { left, right })
            }
            22 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::LessThan { left, right })
            }
            65 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::LessOrEq { left, right })
            }
            66 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::GreaterThan { left, right })
            }
            67 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::GreaterOrEq { left, right })
            }
            68 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::BitAnd { left, right })
            }
            69 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::BitOr { left, right })
            }
            70 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::BitXor { left, right })
            }
            71 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Shl { left, right })
            }
            72 => {
                let left = Self::deserialize_operand(r)?;
                let right = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Shr { left, right })
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
                let mut field_buf = [0u8; 4];
                r.read_exact(&mut field_buf)?;
                Ok(MlibInstruction::Extract {
                    value,
                    variant_idx: u32::from_le_bytes(var_buf),
                    field_idx: u32::from_le_bytes(field_buf),
                })
            }
            27 => {
                let base = Self::deserialize_operand(r)?;
                let mut field_buf = [0u8; 4];
                r.read_exact(&mut field_buf)?;
                Ok(MlibInstruction::FieldPtr {
                    base,
                    field_idx: u32::from_le_bytes(field_buf),
                })
            }
            13 => {
                let value = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Drop { value })
            }
            14 => {
                let value = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::BoxNew { value })
            }
            15 => {
                let value = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::BoxFree { value })
            }
            18 => Ok(MlibInstruction::ListNew),
            0x21 => {
                let value = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::MarkInit { value })
            }
            23 => {
                let mut ty_buf = [0u8; 4];
                r.read_exact(&mut ty_buf)?;
                Ok(MlibInstruction::SizeOf { ty: u32::from_le_bytes(ty_buf) })
            }
            24 => {
                let mut ty_buf = [0u8; 4];
                r.read_exact(&mut ty_buf)?;
                Ok(MlibInstruction::AlignOf { ty: u32::from_le_bytes(ty_buf) })
            }
            0x22 => {
                let mut ty_buf = [0u8; 4];
                r.read_exact(&mut ty_buf)?;
                Ok(MlibInstruction::Null { ty: u32::from_le_bytes(ty_buf) })
            }
            0x24 => Ok(MlibInstruction::Nop),
            25 => {
                let value = Self::deserialize_operand(r)?;
                let mut ty_buf = [0u8; 4];
                r.read_exact(&mut ty_buf)?;
                Ok(MlibInstruction::Cast { value, ty: u32::from_le_bytes(ty_buf) })
            }
            26 => {
                let base = Self::deserialize_operand(r)?;
                let offset = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::PtrOffset { base, offset })
            }
            19 => {
                let list = Self::deserialize_operand(r)?;
                let value = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::ListPush { list, value })
            }
            20 => {
                let list = Self::deserialize_operand(r)?;
                let index = Self::deserialize_operand(r)?;
                let mut mut_buf = [0u8; 1];
                r.read_exact(&mut mut_buf)?;
                Ok(MlibInstruction::ListGet { list, index, is_mut: mut_buf[0] != 0 })
            }
            0x26 => {
                let future = Self::deserialize_operand(r)?;
                Ok(MlibInstruction::Await { future })
            }
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Unknown instruction opcode {}", tag_buf[0]))),
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
            4 => Ok(MlibTerminator::Unreachable),
            5 => Ok(MlibTerminator::MissingReturn),
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
        let mut arg_count_buf = [0u8; 4];
        r.read_exact(&mut arg_count_buf)?;
        let arg_count = u32::from_le_bytes(arg_count_buf);
        let mut is_async_buf = [0u8; 1];
        r.read_exact(&mut is_async_buf)?;
        let is_async = is_async_buf[0] != 0;
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
        Ok(MlibFunction { name, arg_count, is_async, values, blocks })
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

    pub fn read_ast_interface<R: Read + Seek>(reader: &mut R) -> Result<Option<(mellis_ast::AstArena, Vec<mellis_ast::Item>, String)>, MlibError> {
        let header = MlibHeader::read_from(reader)?;
        
        if header.magic != MLIB_MAGIC {
            return Err(MlibError::InvalidMagic);
        }
        
        reader.seek(SeekFrom::Start(header.section_table_offset))?;

        let mut sections = Vec::new();
        for _ in 0..header.section_count {
            let section = SectionEntry::read_from(reader)?;
            sections.push(section);
        }
        
        for section in &sections {
            if section.section_type == SectionType::AstInterface {
                reader.seek(SeekFrom::Start(section.offset))?;
                let mut data = vec![0u8; section.size as usize];
                reader.read_exact(&mut data)?;
                
                let result = bincode::deserialize::<(mellis_ast::AstArena, Vec<mellis_ast::Item>, String)>(&data)
                    .map_err(|e| MlibError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
                return Ok(Some(result));
            }
        }
        
        Ok(None)
    }
}
