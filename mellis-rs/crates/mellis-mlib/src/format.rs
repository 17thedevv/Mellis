use std::io::{Read, Write};

pub const MLIB_MAGIC: [u8; 4] = *b"MLIB";
pub const MLIB_FORMAT_VERSION: u16 = 1;
pub const MLIB_COMPILER_VERSION: u16 = 1; // v1.0
pub const MLIB_MVIR_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionType {
    ExportTable = 1,
    TypeMetadata = 2,
    TraitMetadata = 3,
    StringTable = 4,
    DependencyTable = 5,
    FeatureTable = 6,
    GenericMVIR = 7,
    ObjectCode = 8,
    Debug = 9,
    Manifest = 10,
    ImplTable = 11,
    MacroMetadata = 12,
    GenericMetadata = 13,
    TypeRefTable = 14,
    Custom = 0xFFFFFFFF,
}

impl SectionType {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            1 => Some(SectionType::ExportTable),
            2 => Some(SectionType::TypeMetadata),
            3 => Some(SectionType::TraitMetadata),
            4 => Some(SectionType::StringTable),
            5 => Some(SectionType::DependencyTable),
            6 => Some(SectionType::FeatureTable),
            7 => Some(SectionType::GenericMVIR),
            8 => Some(SectionType::ObjectCode),
            9 => Some(SectionType::Debug),
            10 => Some(SectionType::Manifest),
            11 => Some(SectionType::ImplTable),
            12 => Some(SectionType::MacroMetadata),
            13 => Some(SectionType::GenericMetadata),
            14 => Some(SectionType::TypeRefTable),
            0xFFFFFFFF => Some(SectionType::Custom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SectionEntry {
    pub section_id: u32,
    pub section_type: SectionType,
    pub offset: u64,
    pub size: u64,
    pub version: u16,
    pub compression: u8, // 0 = None, 1 = LZ4, 2 = ZSTD
    pub reserved: [u8; 5],
    pub hash: u64,
}

#[derive(Debug, Clone)]
pub struct MlibHeader {
    pub magic: [u8; 4],
    pub format_version: u16,
    pub compiler_version: u16,
    pub mvir_version: u16,
    pub target_triple: [u8; 64],
    pub module_uuid: [u8; 16],
    pub module_hash: u64,
    pub timestamp: u64,
    pub flags: u32,
    pub section_count: u32,
    pub section_table_offset: u64,
}

impl MlibHeader {
    pub fn new() -> Self {
        let mut target_triple = [0u8; 64];
        let target_str = "x86_64-pc-windows-msvc";
        let bytes = target_str.as_bytes();
        let len = std::cmp::min(bytes.len(), 64);
        target_triple[..len].copy_from_slice(&bytes[..len]);

        Self {
            magic: MLIB_MAGIC,
            format_version: MLIB_FORMAT_VERSION,
            compiler_version: MLIB_COMPILER_VERSION,
            mvir_version: MLIB_MVIR_VERSION,
            target_triple,
            module_uuid: [0u8; 16],
            module_hash: 0,
            timestamp: 0,
            flags: 0,
            section_count: 0,
            section_table_offset: 0,
        }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.magic)?;
        writer.write_all(&self.format_version.to_le_bytes())?;
        writer.write_all(&self.compiler_version.to_le_bytes())?;
        writer.write_all(&self.mvir_version.to_le_bytes())?;
        writer.write_all(&self.target_triple)?;
        writer.write_all(&self.module_uuid)?;
        writer.write_all(&self.module_hash.to_le_bytes())?;
        writer.write_all(&self.timestamp.to_le_bytes())?;
        writer.write_all(&self.flags.to_le_bytes())?;
        writer.write_all(&self.section_count.to_le_bytes())?;
        writer.write_all(&self.section_table_offset.to_le_bytes())?;
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        
        let mut f_ver = [0u8; 2];
        reader.read_exact(&mut f_ver)?;
        
        let mut c_ver = [0u8; 2];
        reader.read_exact(&mut c_ver)?;
        
        let mut m_ver = [0u8; 2];
        reader.read_exact(&mut m_ver)?;

        let mut target_triple = [0u8; 64];
        reader.read_exact(&mut target_triple)?;
        
        let mut module_uuid = [0u8; 16];
        reader.read_exact(&mut module_uuid)?;

        let mut module_hash = [0u8; 8];
        reader.read_exact(&mut module_hash)?;

        let mut timestamp = [0u8; 8];
        reader.read_exact(&mut timestamp)?;

        let mut flags = [0u8; 4];
        reader.read_exact(&mut flags)?;

        let mut s_cnt = [0u8; 4];
        reader.read_exact(&mut s_cnt)?;

        let mut s_offset = [0u8; 8];
        reader.read_exact(&mut s_offset)?;
        
        Ok(Self {
            magic,
            format_version: u16::from_le_bytes(f_ver),
            compiler_version: u16::from_le_bytes(c_ver),
            mvir_version: u16::from_le_bytes(m_ver),
            target_triple,
            module_uuid,
            module_hash: u64::from_le_bytes(module_hash),
            timestamp: u64::from_le_bytes(timestamp),
            flags: u32::from_le_bytes(flags),
            section_count: u32::from_le_bytes(s_cnt),
            section_table_offset: u64::from_le_bytes(s_offset),
        })
    }
}

impl SectionEntry {
    pub fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.section_id.to_le_bytes())?;
        writer.write_all(&(self.section_type as u32).to_le_bytes())?;
        writer.write_all(&self.offset.to_le_bytes())?;
        writer.write_all(&self.size.to_le_bytes())?;
        writer.write_all(&self.version.to_le_bytes())?;
        writer.write_all(&[self.compression])?;
        writer.write_all(&self.reserved)?;
        writer.write_all(&self.hash.to_le_bytes())?;
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut s_id = [0u8; 4];
        reader.read_exact(&mut s_id)?;

        let mut s_type = [0u8; 4];
        reader.read_exact(&mut s_type)?;
        
        let mut offset = [0u8; 8];
        reader.read_exact(&mut offset)?;
        
        let mut size = [0u8; 8];
        reader.read_exact(&mut size)?;
        
        let mut version = [0u8; 2];
        reader.read_exact(&mut version)?;

        let mut compression = [0u8; 1];
        reader.read_exact(&mut compression)?;

        let mut reserved = [0u8; 5];
        reader.read_exact(&mut reserved)?;

        let mut hash = [0u8; 8];
        reader.read_exact(&mut hash)?;

        let st = SectionType::from_u32(u32::from_le_bytes(s_type))
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Unknown section type"))?;
            
        Ok(Self {
            section_id: u32::from_le_bytes(s_id),
            section_type: st,
            offset: u64::from_le_bytes(offset),
            size: u64::from_le_bytes(size),
            version: u16::from_le_bytes(version),
            compression: compression[0],
            reserved,
            hash: u64::from_le_bytes(hash),
        })
    }
}
