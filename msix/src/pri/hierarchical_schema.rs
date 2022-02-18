use anyhow::{ensure, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HierarchicalSchema {
    unique_name: String,
    name: String,
    scopes: Vec<ResourceMapEntry>,
    items: Vec<ResourceMapEntry>,
}

impl HierarchicalSchema {
    pub const IDENTIFIER: &'static [u8; 16] = b"[mrm_hschemaex] ";

    pub fn read<R: Read + Seek>(r: &mut R) -> Result<Self> {
        ensure!(r.read_u16::<LittleEndian>()? == 1);
        let unique_name_length = r.read_u16::<LittleEndian>()? as usize;
        let name_length = r.read_u16::<LittleEndian>()? as usize;
        ensure!(r.read_u16::<LittleEndian>()? == 0);
        let mut hnames = [0; 16];
        r.read_exact(&mut hnames)?;
        ensure!(hnames == *b"[def_hnamesx]  \0");
        let _major_version = r.read_u16::<LittleEndian>()?;
        let _minor_version = r.read_u16::<LittleEndian>()?;
        ensure!(r.read_u32::<LittleEndian>()? == 0);
        let _checksum = r.read_u32::<LittleEndian>()?;
        let num_scopes = r.read_u32::<LittleEndian>()? as usize;
        let num_items = r.read_u32::<LittleEndian>()? as usize;
        let mut unique_name = String::with_capacity(unique_name_length);
        loop {
            let c = r.read_u16::<LittleEndian>()?;
            if c == 0 {
                break;
            }
            unique_name.push(char::from_u32(c as u32).unwrap());
        }
        ensure!(unique_name.len() + 1 == unique_name_length);
        let mut name = String::with_capacity(name_length);
        loop {
            let c = r.read_u16::<LittleEndian>()?;
            if c == 0 {
                break;
            }
            name.push(char::from_u32(c as u32).unwrap());
        }
        ensure!(name.len() + 1 == name_length);
        ensure!(r.read_u16::<LittleEndian>()? == 0);
        let _max_full_path_length = r.read_u16::<LittleEndian>();
        ensure!(r.read_u16::<LittleEndian>()? == 0);
        ensure!(r.read_u32::<LittleEndian>()? as usize == num_scopes + num_items);
        ensure!(r.read_u32::<LittleEndian>()? as usize == num_scopes);
        ensure!(r.read_u32::<LittleEndian>()? as usize == num_items);
        let unicode_data_length = r.read_u32::<LittleEndian>()? as u64;
        r.read_u32::<LittleEndian>()?;
        r.read_u32::<LittleEndian>()?;
        let mut scope_and_item_infos = Vec::with_capacity(num_scopes + num_items);
        for _ in 0..(num_scopes + num_items) {
            let parent = r.read_u16::<LittleEndian>()? as usize;
            let full_path_length = r.read_u16::<LittleEndian>()? as usize;
            let _uppercase_first_char = r.read_u16::<LittleEndian>()?;
            let _name_length_2 = r.read_u8()?;
            let flags = r.read_u8()?;
            let name_offset = r.read_u16::<LittleEndian>()? as u64 | (flags as u64 & 0xf << 16);
            let index = r.read_u16::<LittleEndian>()? as usize;
            let is_scope = flags & 0x10 > 0;
            let name_in_ascii = flags & 0x20 > 0;
            scope_and_item_infos.push(ScopeAndItemInfo {
                parent,
                full_path_length,
                is_scope,
                name_in_ascii,
                name_offset,
                index,
            });
        }
        let mut scope_ex_infos = Vec::with_capacity(num_scopes);
        for _ in 0..num_scopes {
            let scope_index = r.read_u16::<LittleEndian>()?;
            let child_count = r.read_u16::<LittleEndian>()?;
            let first_child_index = r.read_u16::<LittleEndian>()?;
            ensure!(r.read_u16::<LittleEndian>()? == 0);
            scope_ex_infos.push(ScopeExInfo {
                scope_index,
                child_count,
                first_child_index,
            });
        }
        let mut item_index_property_to_index = Vec::with_capacity(num_items);
        for _ in 0..num_items {
            item_index_property_to_index.push(r.read_u16::<LittleEndian>()?);
        }
        let unicode_data_offset = r.seek(SeekFrom::Current(0))?;
        let ascii_data_offset = unicode_data_offset + unicode_data_length * 2;
        let mut scopes = vec![ResourceMapEntry::default(); num_scopes];
        let mut items = vec![ResourceMapEntry::default(); num_items];
        for info in &scope_and_item_infos {
            let pos = if info.name_in_ascii {
                ascii_data_offset + info.name_offset
            } else {
                unicode_data_offset + info.name_offset * 2
            };
            r.seek(SeekFrom::Start(pos))?;
            let mut name = String::with_capacity(info.full_path_length);
            if info.full_path_length != 0 {
                if info.name_in_ascii {
                    loop {
                        let c = r.read_u8()?;
                        if c == 0 {
                            break;
                        }
                        name.push(char::from_u32(c as u32).unwrap());
                    }
                } else {
                    loop {
                        let c = r.read_u16::<LittleEndian>()?;
                        if c == 0 {
                            break;
                        }
                        name.push(char::from_u32(c as u32).unwrap());
                    }
                }
            }
            let parent = if info.parent != 0xffff {
                Some(info.parent)
            } else {
                None
            };
            let entry = ResourceMapEntry { parent, name };
            if info.is_scope {
                scopes[info.index] = entry;
            } else {
                items[info.index] = entry;
            }
        }
        Ok(Self {
            unique_name,
            name,
            scopes,
            items,
        })
    }

    pub fn write<W: Write + Seek>(&self, w: &mut W) -> Result<()> {
        w.write_u16::<LittleEndian>(1)?;
        w.write_u16::<LittleEndian>(self.unique_name.len() as u16 + 1)?;
        w.write_u16::<LittleEndian>(self.name.len() as u16 + 1)?;
        w.write_u16::<LittleEndian>(0)?;
        w.write_all(b"[def_hnamesx]  \0")?;
        w.write_u16::<LittleEndian>(1)?;
        w.write_u16::<LittleEndian>(0)?;
        w.write_u32::<LittleEndian>(0)?;
        // TODO: checksum
        w.write_u32::<LittleEndian>(0)?;
        w.write_u32::<LittleEndian>(self.scopes.len() as u32)?;
        w.write_u32::<LittleEndian>(self.items.len() as u32)?;
        for c in self.unique_name.chars() {
            w.write_u16::<LittleEndian>(c as u16)?;
        }
        w.write_u16::<LittleEndian>(0)?;
        for c in self.name.chars() {
            w.write_u16::<LittleEndian>(c as u16)?;
        }
        w.write_u16::<LittleEndian>(0)?;
        w.write_u16::<LittleEndian>(0)?;
        // TODO: max full path length
        w.write_u16::<LittleEndian>(256)?;
        w.write_u16::<LittleEndian>(0)?;
        w.write_u32::<LittleEndian>((self.scopes.len() + self.items.len()) as u32)?;
        w.write_u32::<LittleEndian>(self.scopes.len() as u32)?;
        w.write_u32::<LittleEndian>(self.items.len() as u32)?;
        // TODO: unicode_data_length
        w.write_u32::<LittleEndian>(0)?;
        // TODO: what's this for
        w.write_u32::<LittleEndian>(0)?;
        // TODO: what's this for
        w.write_u32::<LittleEndian>(0)?;

        Ok(())
    }
}

struct ScopeAndItemInfo {
    parent: usize,
    full_path_length: usize,
    is_scope: bool,
    name_in_ascii: bool,
    name_offset: u64,
    index: usize,
}

struct ScopeExInfo {
    scope_index: u16,
    child_count: u16,
    first_child_index: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceMapEntry {
    pub parent: Option<usize>,
    pub name: String,
}
