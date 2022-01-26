use crate::apk::manifest::AndroidManifest;
use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use roxmltree::Document;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub struct Xml(String);

impl Xml {
    pub fn from_string(xml: String) -> Self {
        Self(xml)
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        Ok(Self(std::fs::read_to_string(path)?))
    }

    pub fn from_manifest(manifest: &AndroidManifest) -> Result<Self> {
        let mut xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#
            .as_bytes()
            .to_vec();
        quick_xml::se::to_writer(&mut xml, manifest)?;
        Ok(Self(String::from_utf8(xml)?))
    }

    pub fn as_doc(&self) -> Result<Document> {
        Ok(Document::parse(&self.0)?)
    }

    //pub fn parse_bxml(
}

pub struct Bxml(Vec<u8>);

impl Bxml {
    //pub fn parse(slice: &[u8]) -
    pub fn emit(xml: &Xml) -> Result<Self> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(u16)]
pub enum ChunkType {
    Null = 0x0000,
    StringPool = 0x0001,
    Table = 0x0002,
    Xml = 0x0003,
    XmlStartNamespace = 0x0100,
    XmlEndNamespace = 0x0101,
    XmlStartElement = 0x0102,
    XmlEndElement = 0x0103,
    XmlCdata = 0x0104,
    XmlLastChunk = 0x017f,
    XmlResourceMap = 0x0180,
    TablePackage = 0x0200,
    TableType = 0x0201,
    TableTypeSpec = 0x0202,
}

impl ChunkType {
    pub fn from_u16(ty: u16) -> Option<Self> {
        Some(match ty {
            ty if ty == ChunkType::Null as u16 => ChunkType::Null,
            ty if ty == ChunkType::StringPool as u16 => ChunkType::StringPool,
            ty if ty == ChunkType::Table as u16 => ChunkType::Table,
            ty if ty == ChunkType::Xml as u16 => ChunkType::Xml,
            ty if ty == ChunkType::XmlStartNamespace as u16 => ChunkType::XmlStartNamespace,
            ty if ty == ChunkType::XmlEndNamespace as u16 => ChunkType::XmlEndNamespace,
            ty if ty == ChunkType::XmlStartElement as u16 => ChunkType::XmlStartElement,
            ty if ty == ChunkType::XmlEndElement as u16 => ChunkType::XmlEndElement,
            ty if ty == ChunkType::XmlCdata as u16 => ChunkType::XmlCdata,
            ty if ty == ChunkType::XmlLastChunk as u16 => ChunkType::XmlLastChunk,
            ty if ty == ChunkType::XmlResourceMap as u16 => ChunkType::XmlResourceMap,
            ty if ty == ChunkType::TablePackage as u16 => ChunkType::TablePackage,
            ty if ty == ChunkType::TableType as u16 => ChunkType::TableType,
            ty if ty == ChunkType::TableTypeSpec as u16 => ChunkType::TableTypeSpec,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResChunkHeader {
    /// Type identifier for this chunk. The meaning of this value depends
    /// on the containing chunk.
    ty: u16,
    /// Size of the chunk header (in bytes). Adding this value to the address
    /// of the chunk allows you to find its associated data (if any).
    header_size: u16,
    /// Total size of this chunk (in bytes). This is the header_size plus the
    /// size of any data associated with the chunk. Adding this value to the
    /// chunk allows you to completely skip its contents (including any child
    /// chunks). If this value is the same as header_size, there is no data
    /// associated with the chunk.
    size: u32,
}

impl ResChunkHeader {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let ty = r.read_u16::<LittleEndian>()?;
        let header_size = r.read_u16::<LittleEndian>()?;
        let size = r.read_u32::<LittleEndian>()?;
        Ok(Self {
            ty,
            header_size,
            size,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_u16::<LittleEndian>(self.ty)?;
        w.write_u16::<LittleEndian>(self.header_size)?;
        w.write_u32::<LittleEndian>(self.size)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResStringPoolHeader {
    string_count: u32,
    style_count: u32,
    flags: u32,
    strings_start: u32,
    styles_start: u32,
}

impl ResStringPoolHeader {
    pub const SORTED_FLAG: u32 = 1 << 0;
    pub const UTF8_FLAG: u32 = 1 << 8;

    pub fn read(r: &mut impl Read) -> Result<Self> {
        let string_count = r.read_u32::<LittleEndian>()?;
        let style_count = r.read_u32::<LittleEndian>()?;
        let flags = r.read_u32::<LittleEndian>()?;
        let strings_start = r.read_u32::<LittleEndian>()?;
        let styles_start = r.read_u32::<LittleEndian>()?;
        Ok(Self {
            string_count,
            style_count,
            flags,
            strings_start,
            styles_start,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResTableHeader {
    package_count: u32,
}

impl ResTableHeader {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let package_count = r.read_u32::<LittleEndian>()?;
        Ok(Self { package_count })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResXmlNodeHeader {
    line_number: u32,
    comment: i32,
}

impl ResXmlNodeHeader {
    pub fn read<R: Read + Seek>(r: &mut R) -> Result<Self> {
        let line_number = r.read_u32::<LittleEndian>()?;
        let comment = r.read_i32::<LittleEndian>()?;
        Ok(Self {
            line_number,
            comment,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResXmlNamespace {
    prefix: i32,
    uri: i32,
}

impl ResXmlNamespace {
    pub fn read<R: Read + Seek>(r: &mut R) -> Result<Self> {
        let prefix = r.read_i32::<LittleEndian>()?;
        let uri = r.read_i32::<LittleEndian>()?;
        Ok(Self { prefix, uri })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResXmlEndElement {
    namespace: i32,
    name: i32,
}

impl ResXmlEndElement {
    pub fn read<R: Read + Seek>(r: &mut R) -> Result<Self> {
        let namespace = r.read_i32::<LittleEndian>()?;
        let name = r.read_i32::<LittleEndian>()?;
        Ok(Self { namespace, name })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResTablePackageHeader {
    /// If this is a base package, its ID. Package IDs start
    /// at 1 (corresponding to the value of the package bits in a
    /// resource identifier). 0 means this is not a base package.
    id: u32,

    /// Actual name of this package, \0-terminated.
    name: [u16; 128],

    /// Offset to a ResStringPoolHeader defining the resource
    /// type symbol table. If zero, this package is inheriting
    /// from another base package (overriding specific values in it).
    type_strings: u32,

    /// Last index into type_strings that is for public use by others.
    last_public_type: u32,

    /// Offset to a ResStringPoolHeader defining the resource key
    /// symbol table. If zero, this package is inheriting from another
    /// base package (overriding specific values in it).
    key_strings: u32,

    /// Last index into key_strings that is for public use by others.
    last_public_key: u32,

    type_id_offset: u32,
}

impl ResTablePackageHeader {
    pub fn read<R: Read + Seek>(r: &mut R) -> Result<Self> {
        let id = r.read_u32::<LittleEndian>()?;
        let mut name = [0; 128];
        for c in name.iter_mut() {
            *c = r.read_u16::<LittleEndian>()?;
        }
        let type_strings = r.read_u32::<LittleEndian>()?;
        let last_public_type = r.read_u32::<LittleEndian>()?;
        let key_strings = r.read_u32::<LittleEndian>()?;
        let last_public_key = r.read_u32::<LittleEndian>()?;
        let type_id_offset = r.read_u32::<LittleEndian>()?;
        Ok(Self {
            id,
            name,
            type_strings,
            last_public_type,
            key_strings,
            last_public_key,
            type_id_offset,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResTableTypeSpecHeader {
    /// The type identifier this chunk is holding. Type IDs start
    /// at 1 (corresponding to the value of the type bits in a
    /// resource identifier). 0 is invalid.
    id: u8,
    /// Must be 0.
    res0: u8,
    /// Must be 0.
    res1: u16,
    /// Number of u32 entry configuration masks that follow.
    entry_count: u32,
}

impl ResTableTypeSpecHeader {
    pub fn read<R: Read + Seek>(r: &mut R) -> Result<Self> {
        let id = r.read_u8()?;
        let res0 = r.read_u8()?;
        let res1 = r.read_u16::<LittleEndian>()?;
        let entry_count = r.read_u32::<LittleEndian>()?;
        Ok(Self {
            id,
            res0,
            res1,
            entry_count,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResTableTypeHeader {
    /// The type identifier this chunk is holding. Type IDs start
    /// at 1 (corresponding to the value of the type bits in a
    /// resource identifier). 0 is invalid.
    id: u8,
    /// Must be 0.
    res0: u8,
    /// Must be 0.
    res1: u16,
    /// Number of u32 entry indices that follow.
    entry_count: u32,
    /// Offset from header where ResTableEntry data starts.
    entries_start: u32,
    // Configuration this collection of entries is designed for.
    // config: ResTableConfig,
}

impl ResTableTypeHeader {
    pub fn read<R: Read + Seek>(r: &mut R) -> Result<Self> {
        let id = r.read_u8()?;
        let res0 = r.read_u8()?;
        let res1 = r.read_u16::<LittleEndian>()?;
        let entry_count = r.read_u32::<LittleEndian>()?;
        let entries_start = r.read_u32::<LittleEndian>()?;
        // let config = ResTableConfig::parse(r)?;
        Ok(Self {
            id,
            res0,
            res1,
            entry_count,
            entries_start,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResTableConfig {}

#[derive(Clone, Debug)]
pub enum Chunk {
    StringPool(ResChunkHeader, ResStringPoolHeader, Vec<u8>),
    Table(ResChunkHeader, ResTableHeader, Vec<Chunk>),
    Xml(ResChunkHeader, Vec<Chunk>),
    XmlStartNamespace(ResChunkHeader, ResXmlNodeHeader, ResXmlNamespace),
    XmlEndNamespace(ResChunkHeader, ResXmlNodeHeader, ResXmlNamespace),
    XmlStartElement(ResChunkHeader, ResXmlNodeHeader, Vec<u8>),
    XmlEndElement(ResChunkHeader, ResXmlNodeHeader, ResXmlEndElement),
    XmlResourceMap(ResChunkHeader, Vec<u8>),
    TablePackage(ResChunkHeader, ResTablePackageHeader, Vec<Chunk>),
    TableType(ResChunkHeader, ResTableTypeHeader, Vec<u8>),
    TableTypeSpec(ResChunkHeader, ResTableTypeSpecHeader, Vec<u32>),
}

impl Chunk {
    pub fn parse<R: Read + Seek>(r: &mut R) -> Result<Self> {
        let start_pos = r.seek(SeekFrom::Current(0))?;
        let header = ResChunkHeader::read(r)?;
        let end_pos = start_pos + header.size as u64;
        match ChunkType::from_u16(header.ty) {
            Some(ChunkType::StringPool) => {
                let string_pool_header = ResStringPoolHeader::read(r)?;
                let mut string_pool = vec![0; header.size as usize - header.header_size as usize];
                r.read_exact(&mut string_pool)?;
                Ok(Chunk::StringPool(header, string_pool_header, string_pool))
            }
            Some(ChunkType::Table) => {
                let table_header = ResTableHeader::read(r)?;
                let mut chunks = vec![];
                while r.seek(SeekFrom::Current(0))? < end_pos {
                    chunks.push(Chunk::parse(r)?);
                }
                Ok(Chunk::Table(header, table_header, chunks))
            }
            Some(ChunkType::Xml) => {
                let mut chunks = vec![];
                while r.seek(SeekFrom::Current(0))? < end_pos {
                    chunks.push(Chunk::parse(r)?);
                }
                Ok(Chunk::Xml(header, chunks))
            }
            Some(ChunkType::XmlStartNamespace) => {
                let node_header = ResXmlNodeHeader::read(r)?;
                let namespace = ResXmlNamespace::read(r)?;
                Ok(Chunk::XmlStartNamespace(header, node_header, namespace))
            }
            Some(ChunkType::XmlEndNamespace) => {
                let node_header = ResXmlNodeHeader::read(r)?;
                let namespace = ResXmlNamespace::read(r)?;
                Ok(Chunk::XmlEndNamespace(header, node_header, namespace))
            }
            Some(ChunkType::XmlStartElement) => {
                let node_header = ResXmlNodeHeader::read(r)?;
                let mut start_element = vec![0; header.size as usize - header.header_size as usize];
                r.read_exact(&mut start_element)?;
                Ok(Chunk::XmlStartElement(header, node_header, start_element))
            }
            Some(ChunkType::XmlEndElement) => {
                let node_header = ResXmlNodeHeader::read(r)?;
                let end_element = ResXmlEndElement::read(r)?;
                Ok(Chunk::XmlEndElement(header, node_header, end_element))
            }
            Some(ChunkType::XmlResourceMap) => {
                let mut resource_map = vec![0; header.size as usize - header.header_size as usize];
                r.read_exact(&mut resource_map)?;
                Ok(Chunk::XmlResourceMap(header, resource_map))
            }
            Some(ChunkType::TablePackage) => {
                let mut package_header = ResTablePackageHeader::read(r)?;
                let mut chunks = vec![];
                while r.seek(SeekFrom::Current(0))? < end_pos {
                    chunks.push(Chunk::parse(r)?);
                }
                Ok(Chunk::TablePackage(header, package_header, chunks))
            }
            Some(ChunkType::TableType) => {
                let type_header = ResTableTypeHeader::read(r)?;
                let mut ty = vec![0; header.size as usize - 20]; //header.header_size as usize];
                r.read_exact(&mut ty)?;
                Ok(Chunk::TableType(header, type_header, ty))
            }
            Some(ChunkType::TableTypeSpec) => {
                let type_spec_header = ResTableTypeSpecHeader::read(r)?;
                let mut type_spec = vec![0; type_spec_header.entry_count as usize];
                for c in type_spec.iter_mut() {
                    *c = r.read_u32::<LittleEndian>()?;
                }
                Ok(Chunk::TableTypeSpec(header, type_spec_header, type_spec))
            }
            Some(ty) => {
                unimplemented!("{:?} {:?}", ty, header);
            }
            None => {
                anyhow::bail!("unrecognized chunk {:?}", header);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_bxml_parse_manifest() -> Result<()> {
        const BXML: &[u8] = include_bytes!("../../assets/AndroidManifest.xml");
        let mut r = Cursor::new(BXML);
        let chunk = Chunk::parse(&mut r)?;
        let pos = r.seek(SeekFrom::Current(0))?;
        assert_eq!(pos, BXML.len() as u64);
        println!("{:?}", chunk);
        panic!();
        Ok(())
    }

    #[test]
    fn test_bxml_parse_arsc() -> Result<()> {
        const BXML: &[u8] = include_bytes!("../../assets/resources.arsc");
        let mut r = Cursor::new(BXML);
        let chunk = Chunk::parse(&mut r)?;
        let pos = r.seek(SeekFrom::Current(0))?;
        assert_eq!(pos, BXML.len() as u64);
        println!("{:?}", chunk);
        panic!();
        Ok(())
    }
}
