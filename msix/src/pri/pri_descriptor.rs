use anyhow::{ensure, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PriDescriptor {
    pub pri_flags: u16,
    pub included_file_list_section: bool,
    pub hierarchical_schema_sections: Vec<u16>,
    pub decision_info_sections: Vec<u16>,
    pub resource_map_sections: Vec<u16>,
    pub primary_resource_map_section: Option<u16>,
    pub referenced_file_sections: Vec<u16>,
    pub data_item_sections: Vec<u16>,
}

impl PriDescriptor {
    pub const IDENTIFIER: &'static [u8; 16] = b"[mrm_pridescex]\0";

    pub fn read(r: &mut impl Read) -> Result<Self> {
        let pri_flags = r.read_u16::<LittleEndian>()?;
        let included_file_list_section = r.read_u16::<LittleEndian>()? == 0xffff;
        ensure!(r.read_u16::<LittleEndian>()? == 0);
        let num_hierarchical_schema_sections = r.read_u16::<LittleEndian>()? as usize;
        let num_decision_info_sections = r.read_u16::<LittleEndian>()? as usize;
        let num_resource_map_sections = r.read_u16::<LittleEndian>()? as usize;
        let primary_resource_map_section = r.read_u16::<LittleEndian>()?;
        let primary_resource_map_section = if primary_resource_map_section == 0xfff {
            None
        } else {
            Some(primary_resource_map_section)
        };
        let num_referenced_file_sections = r.read_u16::<LittleEndian>()? as usize;
        let num_data_item_sections = r.read_u16::<LittleEndian>()? as usize;
        ensure!(r.read_u16::<LittleEndian>()? == 0);
        let mut hierarchical_schema_sections = Vec::with_capacity(num_hierarchical_schema_sections);
        for _ in 0..num_hierarchical_schema_sections {
            hierarchical_schema_sections.push(r.read_u16::<LittleEndian>()?);
        }
        let mut decision_info_sections = Vec::with_capacity(num_decision_info_sections);
        for _ in 0..num_decision_info_sections {
            decision_info_sections.push(r.read_u16::<LittleEndian>()?);
        }
        let mut resource_map_sections = Vec::with_capacity(num_resource_map_sections);
        for _ in 0..num_resource_map_sections {
            resource_map_sections.push(r.read_u16::<LittleEndian>()?);
        }
        let mut referenced_file_sections = Vec::with_capacity(num_referenced_file_sections);
        for _ in 0..num_referenced_file_sections {
            referenced_file_sections.push(r.read_u16::<LittleEndian>()?);
        }
        let mut data_item_sections = Vec::with_capacity(num_data_item_sections);
        for _ in 0..num_data_item_sections {
            data_item_sections.push(r.read_u16::<LittleEndian>()?);
        }
        Ok(Self {
            pri_flags,
            included_file_list_section,
            hierarchical_schema_sections,
            decision_info_sections,
            resource_map_sections,
            primary_resource_map_section,
            referenced_file_sections,
            data_item_sections,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_u16::<LittleEndian>(self.pri_flags)?;
        let included_file_list_section = if self.included_file_list_section {
            0xffff
        } else {
            0
        };
        w.write_u16::<LittleEndian>(included_file_list_section)?;
        w.write_u16::<LittleEndian>(0)?;
        w.write_u16::<LittleEndian>(self.hierarchical_schema_sections.len() as u16)?;
        w.write_u16::<LittleEndian>(self.decision_info_sections.len() as u16)?;
        w.write_u16::<LittleEndian>(self.resource_map_sections.len() as u16)?;
        let primary_resource_map_section = self.primary_resource_map_section.unwrap_or(0xffff);
        w.write_u16::<LittleEndian>(primary_resource_map_section)?;
        w.write_u16::<LittleEndian>(self.referenced_file_sections.len() as u16)?;
        w.write_u16::<LittleEndian>(self.data_item_sections.len() as u16)?;
        w.write_u16::<LittleEndian>(0)?;
        for id in &self.hierarchical_schema_sections {
            w.write_u16::<LittleEndian>(*id)?;
        }
        for id in &self.decision_info_sections {
            w.write_u16::<LittleEndian>(*id)?;
        }
        for id in &self.resource_map_sections {
            w.write_u16::<LittleEndian>(*id)?;
        }
        for id in &self.referenced_file_sections {
            w.write_u16::<LittleEndian>(*id)?;
        }
        for id in &self.data_item_sections {
            w.write_u16::<LittleEndian>(*id)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum PriDescriptorFlags {
    AutoMerge = 1,
    IsDeploymentMergeable = 2,
    IsDeploymentMergeResult = 4,
    IsAutomergeMergeResult = 8,
}
