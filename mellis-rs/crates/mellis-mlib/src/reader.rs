use std::io::{Read, Seek, SeekFrom};
use crate::format::{MlibHeader, SectionEntry, SectionType, MLIB_MAGIC, MLIB_FORMAT_VERSION};
use crate::ir::MlibModule;

#[derive(Debug)]
pub enum MlibError {
    Io(std::io::Error),
    InvalidMagic,
    VersionMismatch(u16),
    BincodeError(bincode::Error),
    UnknownSection(u32),
    CorruptedData,
}

impl From<std::io::Error> for MlibError {
    fn from(err: std::io::Error) -> Self {
        MlibError::Io(err)
    }
}

impl From<bincode::Error> for MlibError {
    fn from(err: bincode::Error) -> Self {
        MlibError::BincodeError(err)
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
        
        let mut sections = Vec::new();
        for _ in 0..header.section_count {
            let section = SectionEntry::read_from(reader)?;
            sections.push(section);
        }
        
        let mut mlib_module = None;
        
        for section in sections {
            match section.section_type {
                SectionType::Mvir => {
                    reader.seek(SeekFrom::Start(section.offset))?;
                    // Bincode reads directly. To prevent reading beyond section, we can use a take() adapter
                    let mut limited_reader = (&mut *reader).take(section.length);
                    let decoded: MlibModule = bincode::deserialize_from(&mut limited_reader)?;
                    mlib_module = Some(decoded);
                    // The mutable borrow of reader is dropped here when limited_reader goes out of scope
                }
                _ => {
                    // Unknown section, skip it
                }
            }
        }
        
        mlib_module.ok_or(MlibError::CorruptedData)
    }
}
