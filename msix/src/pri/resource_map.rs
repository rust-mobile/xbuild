use anyhow::{ensure, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceMap {

}

impl ResourceMap {
    pub const IDENTIFIER: &'static [u8; 16] = b"[mrm_res_map2_]\0";

    pub fn read(r: &mut impl Read) -> Result<Self> {
        // v1 field environment_references_length
        ensure!(r.read_u16::<LittleEndian>()? == 0);
        // v1 field num_environment_references
        ensure!(r.read_u16::<LittleEndian>()? == 0);
        let schema_section = r.read_u16::<LittleEndian>()?;
        let schema_section_length = r.read_u16::<LittleEndian>()?;
        let decision_info_section = r.read_u16::<LittleEndian>()?;
        Ok(Self {

        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
    }
}
