use std::io::{Read, Write};

pub const MLIB_MAGIC: [u8; 4] = *b"MLB2";
pub const MLIB_FORMAT_VERSION: u16 = 1;
pub const MLIB_COMPILER_VERSION: u16 = 1; // v1.0

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionType {
    ExportTable = 1,
    TypeMetadata = 2,
    Mvir = 3,
    DependencyTable = 4,
    Manifest = 5,
}

impl SectionType {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            1 => Some(SectionType::ExportTable),
            2 => Some(SectionType::TypeMetadata),
            3 => Some(SectionType::Mvir),
            4 => Some(SectionType::DependencyTable),
            5 => Some(SectionType::Manifest),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SectionEntry {
    pub section_type: SectionType,
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone)]
pub struct MlibHeader {
    pub magic: [u8; 4],
    pub format_version: u16,
    pub compiler_version: u16,
    pub target: [u8; 32],
    pub section_count: u32,
}

impl MlibHeader {
    pub fn new() -> Self {
        let mut target = [0u8; 32];
        let target_str = "x86_64-pc-windows-msvc";
        let bytes = target_str.as_bytes();
        let len = std::cmp::min(bytes.len(), 32);
        target[..len].copy_from_slice(&bytes[..len]);

        Self {
            magic: MLIB_MAGIC,
            format_version: MLIB_FORMAT_VERSION,
            compiler_version: MLIB_COMPILER_VERSION,
            target,
            section_count: 0,
        }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.magic)?;
        writer.write_all(&self.format_version.to_le_bytes())?;
        writer.write_all(&self.compiler_version.to_le_bytes())?;
        writer.write_all(&self.target)?;
        writer.write_all(&self.section_count.to_le_bytes())?;
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        
        let mut f_ver = [0u8; 2];
        reader.read_exact(&mut f_ver)?;
        
        let mut c_ver = [0u8; 2];
        reader.read_exact(&mut c_ver)?;
        
        let mut target = [0u8; 32];
        reader.read_exact(&mut target)?;
        
        let mut s_cnt = [0u8; 4];
        reader.read_exact(&mut s_cnt)?;
        
        Ok(Self {
            magic,
            format_version: u16::from_le_bytes(f_ver),
            compiler_version: u16::from_le_bytes(c_ver),
            target,
            section_count: u32::from_le_bytes(s_cnt),
        })
    }
}

impl SectionEntry {
    pub fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&(self.section_type as u32).to_le_bytes())?;
        writer.write_all(&self.offset.to_le_bytes())?;
        writer.write_all(&self.length.to_le_bytes())?;
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut s_type = [0u8; 4];
        reader.read_exact(&mut s_type)?;
        
        let mut offset = [0u8; 8];
        reader.read_exact(&mut offset)?;
        
        let mut length = [0u8; 8];
        reader.read_exact(&mut length)?;
        
        let st = SectionType::from_u32(u32::from_le_bytes(s_type))
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Unknown section type"))?;
            
        Ok(Self {
            section_type: st,
            offset: u64::from_le_bytes(offset),
            length: u64::from_le_bytes(length),
        })
    }
}
