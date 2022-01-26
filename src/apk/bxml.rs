use crate::apk::manifest::AndroidManifest;
use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use roxmltree::{Document, Node};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub struct Xml(String);

impl Xml {
    pub fn new(xml: String) -> Self {
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

    pub fn compile(&self) -> Result<Vec<u8>> {
        fn compile_node(node: Node, strings: &mut Strings, chunks: &mut Vec<u8>) -> Result<()> {
            for ns in node.namespaces() {
                Chunk::XmlStartNamespace(
                    ResChunkHeader {
                        ty: ChunkType::XmlStartNamespace as u16,
                        header_size: 16,
                        size: 24,
                    },
                    ResXmlNodeHeader::default(),
                    ResXmlNamespace {
                        prefix: ns.name().map(|ns| strings.id(ns)).unwrap_or(-1),
                        uri: strings.id(ns.uri()),
                    },
                )
                .write(chunks)?;
            }
            let mut id_index = 0;
            let mut class_index = 0;
            let mut style_index = 0;
            let mut attrs = vec![];
            for (i, attr) in node.attributes().iter().enumerate() {
                match attr.name() {
                    "id" => id_index = i as u16 + 1,
                    "class" => class_index = i as u16 + 1,
                    "style" => style_index = i as u16 + 1,
                    _ => {}
                }
                attrs.push(ResXmlAttribute {
                    namespace: attr.namespace().map(|ns| strings.id(ns)).unwrap_or(-1),
                    name: strings.id(attr.name()),
                    raw_value: strings.id(attr.value()),
                    typed_value: ResValue {
                        size: attr.value().len() as u16,
                        res0: 0,
                        data_type: 0x03, // string
                        data: strings.id(attr.value()) as u32,
                    },
                });
            }
            let namespace = node
                .tag_name()
                .namespace()
                .map(|ns| strings.id(ns))
                .unwrap_or(-1);
            let name = strings.id(node.tag_name().name());
            Chunk::XmlStartElement(
                ResChunkHeader {
                    ty: ChunkType::XmlStartElement as u16,
                    header_size: 16,
                    size: 36 + attrs.len() as u32 * 20,
                },
                ResXmlNodeHeader::default(),
                ResXmlStartElement {
                    namespace,
                    name,
                    attribute_start: 0x0014,
                    attribute_size: 0x0014,
                    attribute_count: attrs.len() as _,
                    id_index,
                    class_index,
                    style_index,
                },
                attrs,
            )
            .write(chunks)?;
            for node in node.children() {
                compile_node(node, strings, chunks)?;
            }
            Chunk::XmlEndElement(
                ResChunkHeader {
                    ty: ChunkType::XmlEndElement as u16,
                    header_size: 16,
                    size: 24,
                },
                ResXmlNodeHeader::default(),
                ResXmlEndElement { namespace, name },
            )
            .write(chunks)?;
            for ns in node.namespaces() {
                Chunk::XmlEndNamespace(
                    ResChunkHeader {
                        ty: ChunkType::XmlEndNamespace as u16,
                        header_size: 16,
                        size: 24,
                    },
                    ResXmlNodeHeader::default(),
                    ResXmlNamespace {
                        prefix: ns.name().map(|ns| strings.id(ns)).unwrap_or(-1),
                        uri: strings.id(ns.uri()),
                    },
                )
                .write(chunks)?;
            }
            Ok(())
        }

        let doc = Document::parse(&self.0)?;
        let mut strings = Strings::default();
        let mut chunks = vec![];
        compile_node(doc.root(), &mut strings, &mut chunks)?;
        let strings = strings.finalize();

        let mut buf = vec![];
        ResChunkHeader {
            ty: ChunkType::Xml as u16,
            header_size: 8,
            size: 0,
        }
        .write(&mut buf)?;
        Chunk::StringPool(
            ResChunkHeader {
                ty: ChunkType::StringPool as u16,
                header_size: 28,
                size: 0,
            },
            ResStringPoolHeader {
                string_count: strings.len() as u32,
                style_count: 0,
                flags: 0,
                strings_start: 28 + strings.len() as u32 * 4,
                styles_start: 0,
            },
            strings,
            vec![],
        )
        .write(&mut buf)?;
        let string_pool_size = buf.len() as u32 - 8;
        buf[12..16].copy_from_slice(&string_pool_size.to_le_bytes());
        buf.extend(chunks);
        let xml_size = buf.len() as u32;
        buf[4..8].copy_from_slice(&xml_size.to_le_bytes());
        Ok(buf)
    }
}

#[derive(Default)]
struct Strings {
    strings: HashMap<String, i32>,
}

impl Strings {
    pub fn id(&mut self, s: &str) -> i32 {
        if let Some(id) = self.strings.get(s).copied() {
            id
        } else {
            let id = self.strings.len() as i32;
            self.strings.insert(s.to_string(), id);
            id
        }
    }

    pub fn finalize(self) -> Vec<String> {
        self.strings
            .into_iter()
            .map(|(k, v)| (v, k))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|(_, v)| v)
            .collect::<Vec<_>>()
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(u16)]
pub enum ChunkType {
    //Null = 0x0000,
    StringPool = 0x0001,
    Table = 0x0002,
    Xml = 0x0003,
    XmlStartNamespace = 0x0100,
    XmlEndNamespace = 0x0101,
    XmlStartElement = 0x0102,
    XmlEndElement = 0x0103,
    //XmlCdata = 0x0104,
    //XmlLastChunk = 0x017f,
    XmlResourceMap = 0x0180,
    TablePackage = 0x0200,
    TableType = 0x0201,
    TableTypeSpec = 0x0202,
}

impl ChunkType {
    pub fn from_u16(ty: u16) -> Option<Self> {
        Some(match ty {
            //ty if ty == ChunkType::Null as u16 => ChunkType::Null,
            ty if ty == ChunkType::StringPool as u16 => ChunkType::StringPool,
            ty if ty == ChunkType::Table as u16 => ChunkType::Table,
            ty if ty == ChunkType::Xml as u16 => ChunkType::Xml,
            ty if ty == ChunkType::XmlStartNamespace as u16 => ChunkType::XmlStartNamespace,
            ty if ty == ChunkType::XmlEndNamespace as u16 => ChunkType::XmlEndNamespace,
            ty if ty == ChunkType::XmlStartElement as u16 => ChunkType::XmlStartElement,
            ty if ty == ChunkType::XmlEndElement as u16 => ChunkType::XmlEndElement,
            //ty if ty == ChunkType::XmlCdata as u16 => ChunkType::XmlCdata,
            //ty if ty == ChunkType::XmlLastChunk as u16 => ChunkType::XmlLastChunk,
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

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_u32::<LittleEndian>(self.string_count)?;
        w.write_u32::<LittleEndian>(self.style_count)?;
        w.write_u32::<LittleEndian>(self.flags)?;
        w.write_u32::<LittleEndian>(self.strings_start)?;
        w.write_u32::<LittleEndian>(self.styles_start)?;
        Ok(())
    }

    pub fn is_utf8(&self) -> bool {
        self.flags & Self::UTF8_FLAG > 0
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

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_u32::<LittleEndian>(self.package_count)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResXmlNodeHeader {
    line_number: u32,
    comment: i32,
}

impl ResXmlNodeHeader {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let line_number = r.read_u32::<LittleEndian>()?;
        let comment = r.read_i32::<LittleEndian>()?;
        Ok(Self {
            line_number,
            comment,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_u32::<LittleEndian>(self.line_number)?;
        w.write_i32::<LittleEndian>(self.comment)?;
        Ok(())
    }
}

impl Default for ResXmlNodeHeader {
    fn default() -> Self {
        Self {
            line_number: 1,
            comment: -1,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResXmlNamespace {
    prefix: i32,
    uri: i32,
}

impl ResXmlNamespace {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let prefix = r.read_i32::<LittleEndian>()?;
        let uri = r.read_i32::<LittleEndian>()?;
        Ok(Self { prefix, uri })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_i32::<LittleEndian>(self.prefix)?;
        w.write_i32::<LittleEndian>(self.uri)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResXmlStartElement {
    /// String of the full namespace of this element.
    namespace: i32,
    /// String name of this node if it is an ELEMENT; the raw
    /// character data if this is a CDATA node.
    name: i32,
    /// Byte offset from the start of this structure to where
    /// the attributes start.
    attribute_start: u16,
    /// Size of the attribute structures that follow.
    attribute_size: u16,
    /// Number of attributes associated with an ELEMENT. These are
    /// available as an array of ResXmlAttribute structures
    /// immediately following this node.
    attribute_count: u16,
    /// Index (1-based) of the "id" attribute. 0 if none.
    id_index: u16,
    /// Index (1-based) of the "class" attribute. 0 if none.
    class_index: u16,
    /// Index (1-based) of the "style" attribute. 0 if none.
    style_index: u16,
}

impl Default for ResXmlStartElement {
    fn default() -> Self {
        Self {
            namespace: -1,
            name: -1,
            attribute_start: 0x0014,
            attribute_size: 0x0014,
            attribute_count: 0,
            id_index: 0,
            class_index: 0,
            style_index: 0,
        }
    }
}

impl ResXmlStartElement {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let namespace = r.read_i32::<LittleEndian>()?;
        let name = r.read_i32::<LittleEndian>()?;
        let attribute_start = r.read_u16::<LittleEndian>()?;
        let attribute_size = r.read_u16::<LittleEndian>()?;
        let attribute_count = r.read_u16::<LittleEndian>()?;
        let id_index = r.read_u16::<LittleEndian>()?;
        let class_index = r.read_u16::<LittleEndian>()?;
        let style_index = r.read_u16::<LittleEndian>()?;
        Ok(Self {
            namespace,
            name,
            attribute_start,
            attribute_size,
            attribute_count,
            id_index,
            class_index,
            style_index,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_i32::<LittleEndian>(self.namespace)?;
        w.write_i32::<LittleEndian>(self.name)?;
        w.write_u16::<LittleEndian>(self.attribute_start)?;
        w.write_u16::<LittleEndian>(self.attribute_size)?;
        w.write_u16::<LittleEndian>(self.attribute_count)?;
        w.write_u16::<LittleEndian>(self.id_index)?;
        w.write_u16::<LittleEndian>(self.class_index)?;
        w.write_u16::<LittleEndian>(self.style_index)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResXmlAttribute {
    namespace: i32,
    name: i32,
    raw_value: i32,
    typed_value: ResValue,
}

impl ResXmlAttribute {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let namespace = r.read_i32::<LittleEndian>()?;
        let name = r.read_i32::<LittleEndian>()?;
        let raw_value = r.read_i32::<LittleEndian>()?;
        let typed_value = ResValue::read(r)?;
        Ok(Self {
            namespace,
            name,
            raw_value,
            typed_value,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_i32::<LittleEndian>(self.namespace)?;
        w.write_i32::<LittleEndian>(self.name)?;
        w.write_i32::<LittleEndian>(self.raw_value)?;
        self.typed_value.write(w)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResXmlEndElement {
    namespace: i32,
    name: i32,
}

impl ResXmlEndElement {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let namespace = r.read_i32::<LittleEndian>()?;
        let name = r.read_i32::<LittleEndian>()?;
        Ok(Self { namespace, name })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_i32::<LittleEndian>(self.namespace)?;
        w.write_i32::<LittleEndian>(self.name)?;
        Ok(())
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

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_u32::<LittleEndian>(self.id)?;
        for c in self.name {
            w.write_u16::<LittleEndian>(c)?;
        }
        w.write_u32::<LittleEndian>(self.type_strings)?;
        w.write_u32::<LittleEndian>(self.last_public_type)?;
        w.write_u32::<LittleEndian>(self.key_strings)?;
        w.write_u32::<LittleEndian>(self.last_public_key)?;
        w.write_u32::<LittleEndian>(self.type_id_offset)?;
        Ok(())
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
    pub fn read(r: &mut impl Read) -> Result<Self> {
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

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_u8(self.id)?;
        w.write_u8(self.res0)?;
        w.write_u16::<LittleEndian>(self.res1)?;
        w.write_u32::<LittleEndian>(self.entry_count)?;
        Ok(())
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
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let id = r.read_u8()?;
        let res0 = r.read_u8()?;
        let res1 = r.read_u16::<LittleEndian>()?;
        let entry_count = r.read_u32::<LittleEndian>()?;
        let entries_start = r.read_u32::<LittleEndian>()?;
        Ok(Self {
            id,
            res0,
            res1,
            entry_count,
            entries_start,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_u8(self.id)?;
        w.write_u8(self.res0)?;
        w.write_u16::<LittleEndian>(self.res1)?;
        w.write_u32::<LittleEndian>(self.entry_count)?;
        w.write_u32::<LittleEndian>(self.entries_start)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResValue {
    size: u16,
    res0: u8,
    data_type: u8,
    data: u32,
}

impl ResValue {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let size = r.read_u16::<LittleEndian>()?;
        let res0 = r.read_u8()?;
        let data_type = r.read_u8()?;
        let data = r.read_u32::<LittleEndian>()?;
        Ok(Self {
            size,
            res0,
            data_type,
            data,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_u16::<LittleEndian>(self.size)?;
        w.write_u8(self.res0)?;
        w.write_u8(self.data_type)?;
        w.write_u32::<LittleEndian>(self.data)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResSpan {
    name: i32,
    first_char: u32,
    last_char: u32,
}

impl ResSpan {
    pub fn read(r: &mut impl Read) -> Result<Option<Self>> {
        let name = r.read_i32::<LittleEndian>()?;
        if name == -1 {
            return Ok(None);
        }
        let first_char = r.read_u32::<LittleEndian>()?;
        let last_char = r.read_u32::<LittleEndian>()?;
        Ok(Some(Self {
            name,
            first_char,
            last_char,
        }))
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_i32::<LittleEndian>(self.name)?;
        w.write_u32::<LittleEndian>(self.first_char)?;
        w.write_u32::<LittleEndian>(self.last_char)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum Chunk {
    StringPool(
        ResChunkHeader,
        ResStringPoolHeader,
        Vec<String>,
        Vec<Vec<ResSpan>>,
    ),
    Table(ResChunkHeader, ResTableHeader, Vec<Chunk>),
    Xml(ResChunkHeader, Vec<Chunk>),
    XmlStartNamespace(ResChunkHeader, ResXmlNodeHeader, ResXmlNamespace),
    XmlEndNamespace(ResChunkHeader, ResXmlNodeHeader, ResXmlNamespace),
    XmlStartElement(
        ResChunkHeader,
        ResXmlNodeHeader,
        ResXmlStartElement,
        Vec<ResXmlAttribute>,
    ),
    XmlEndElement(ResChunkHeader, ResXmlNodeHeader, ResXmlEndElement),
    XmlResourceMap(ResChunkHeader, Vec<u32>),
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
                let count =
                    string_pool_header.string_count as i64 + string_pool_header.style_count as i64;
                r.seek(SeekFrom::Current(count * 4))?;
                /*let mut string_indices = Vec::with_capacity(string_pool_header.string_count);
                for _ in 0..string_pool_header.string_count {
                    string_indices.push(r.read_u32::<LittleEndian>()?);
                }
                let mut style_indices = Vec::with_capacity(string_pool_header.style_count);
                for _ in 0..string_pool_header.style_count {
                    style_indices.push(r.read_u32::<LittleEndian>()?);
                }*/
                let mut strings = Vec::with_capacity(string_pool_header.string_count as usize);
                for _ in 0..string_pool_header.string_count {
                    if string_pool_header.is_utf8() {
                        let charsh = r.read_u8()? as u16;
                        let _chars = if charsh > 0x7f {
                            charsh & 0x7f | r.read_u8()? as u16
                        } else {
                            charsh
                        };
                        let bytesh = r.read_u8()? as u16;
                        let bytes = if bytesh > 0x7f {
                            bytesh & 0x7f | r.read_u8()? as u16
                        } else {
                            bytesh
                        };
                        let mut buf = vec![0; bytes as usize];
                        r.read_exact(&mut buf)?;
                        let s = String::from_utf8(buf)?;
                        strings.push(s);
                        if r.read_u8()? != 0 {
                            // fails to read some files otherwise
                            r.seek(SeekFrom::Start(end_pos))?;
                        }
                    } else {
                        let charsh = r.read_u16::<LittleEndian>()? as u32;
                        let chars = if charsh > 0x7fff {
                            charsh & 0x7fff | r.read_u16::<LittleEndian>()? as u32
                        } else {
                            charsh
                        };
                        let mut buf = Vec::with_capacity(chars as usize * 2);
                        loop {
                            let code = r.read_u16::<LittleEndian>()?;
                            if code != 0 {
                                buf.push(code);
                            } else {
                                break;
                            }
                        }
                        let s = String::from_utf16(unsafe { std::mem::transmute(buf.as_slice()) })?;
                        strings.push(s);
                    }
                }
                let pos = r.seek(SeekFrom::Current(0))? as i64;
                if pos % 4 != 0 {
                    r.seek(SeekFrom::Current(4 - pos % 4))?;
                }
                let mut styles = Vec::with_capacity(string_pool_header.style_count as usize);
                for _ in 0..string_pool_header.style_count {
                    let mut spans = vec![];
                    loop {
                        if let Some(span) = ResSpan::read(r)? {
                            spans.push(span);
                        } else {
                            break;
                        }
                    }
                    styles.push(spans);
                }
                Ok(Chunk::StringPool(
                    header,
                    string_pool_header,
                    strings,
                    styles,
                ))
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
                let start_element = ResXmlStartElement::read(r)?;
                let mut attributes = Vec::with_capacity(start_element.attribute_count as usize);
                for _ in 0..start_element.attribute_count {
                    attributes.push(ResXmlAttribute::read(r)?);
                }
                Ok(Chunk::XmlStartElement(
                    header,
                    node_header,
                    start_element,
                    attributes,
                ))
            }
            Some(ChunkType::XmlEndElement) => {
                let node_header = ResXmlNodeHeader::read(r)?;
                let end_element = ResXmlEndElement::read(r)?;
                Ok(Chunk::XmlEndElement(header, node_header, end_element))
            }
            Some(ChunkType::XmlResourceMap) => {
                let mut resource_map =
                    Vec::with_capacity((header.size as usize - header.header_size as usize) / 4);
                for _ in 0..resource_map.capacity() {
                    resource_map.push(r.read_u32::<LittleEndian>()?);
                }
                Ok(Chunk::XmlResourceMap(header, resource_map))
            }
            Some(ChunkType::TablePackage) => {
                let package_header = ResTablePackageHeader::read(r)?;
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
            None => {
                anyhow::bail!("unrecognized chunk {:?}", header);
            }
        }
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        match self {
            Chunk::StringPool(header, string_pool_header, strings, styles) => {
                header.write(w)?;
                string_pool_header.write(w)?;
                todo!()
            }
            Chunk::Table(header, table_header, chunks) => {
                header.write(w)?;
                table_header.write(w)?;
                for chunk in chunks {
                    chunk.write(w)?;
                }
            }
            Chunk::Xml(header, chunks) => {
                header.write(w)?;
                for chunk in chunks {
                    chunk.write(w)?;
                }
            }
            Chunk::XmlStartNamespace(header, node_header, namespace) => {
                header.write(w)?;
                node_header.write(w)?;
                namespace.write(w)?;
            }
            Chunk::XmlEndNamespace(header, node_header, namespace) => {
                header.write(w)?;
                node_header.write(w)?;
                namespace.write(w)?;
            }
            Chunk::XmlStartElement(header, node_header, start_element, attributes) => {
                header.write(w)?;
                node_header.write(w)?;
                start_element.write(w)?;
                for attr in attributes {
                    attr.write(w)?;
                }
            }
            Chunk::XmlEndElement(header, node_header, end_element) => {
                header.write(w)?;
                node_header.write(w)?;
                end_element.write(w)?;
            }
            Chunk::XmlResourceMap(header, resource_map) => {
                header.write(w)?;
                for entry in resource_map {
                    w.write_u32::<LittleEndian>(*entry)?;
                }
            }
            Chunk::TablePackage(header, package_header, chunks) => {
                header.write(w)?;
                package_header.write(w)?;
                for chunk in chunks {
                    chunk.write(w)?;
                }
            }
            Chunk::TableType(header, type_header, ty) => {
                header.write(w)?;
                type_header.write(w)?;
                w.write_all(&ty)?;
            }
            Chunk::TableTypeSpec(header, type_spec_header, type_spec) => {
                header.write(w)?;
                type_spec_header.write(w)?;
                for spec in type_spec {
                    w.write_u32::<LittleEndian>(*spec)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_bxml_parse_manifest() -> Result<()> {
        const BXML: &[u8] = include_bytes!("../../assets/AndroidManifest.bxml");
        let mut r = Cursor::new(BXML);
        let _chunk = Chunk::parse(&mut r)?;
        let pos = r.seek(SeekFrom::Current(0))?;
        assert_eq!(pos, BXML.len() as u64);
        Ok(())
    }

    #[test]
    fn test_bxml_gen_manifest() -> Result<()> {
        const XML: &str = include_str!("../../assets/AndroidManifest.xml");
        let bxml = Xml::new(XML.to_string()).compile()?;
        let mut cursor = Cursor::new(bxml.as_slice());
        let _chunk = Chunk::parse(&mut cursor).unwrap();
        let pos = cursor.seek(SeekFrom::Current(0))?;
        assert_eq!(pos, bxml.len() as u64);
        Ok(())
    }

    #[test]
    fn test_bxml_parse_arsc() -> Result<()> {
        const BXML: &[u8] = include_bytes!("../../assets/resources.arsc");
        let mut r = Cursor::new(BXML);
        let _chunk = Chunk::parse(&mut r)?;
        let pos = r.seek(SeekFrom::Current(0))?;
        assert_eq!(pos, BXML.len() as u64);
        Ok(())
    }
}
