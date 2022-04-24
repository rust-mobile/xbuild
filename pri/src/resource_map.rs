use anyhow::{ensure, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::io::{Read, Write};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceMap {
    hierarchical_schema_section: u16,
    decision_info_section: u16,
    //candidate_sets: HashMap<u16, CandidateSet>,
    item_to_item_info_groups: Vec<ItemToItemInfoGroup>,
    item_info_groups: Vec<ItemInfoGroup>,
    item_infos: Vec<ItemInfo>,
    candidate_infos: Vec<CandidateInfo>,
}

impl ResourceMap {
    pub const IDENTIFIER: &'static [u8; 16] = b"[mrm_res_map2_]\0";

    pub fn read(r: &mut impl Read) -> Result<Self> {
        let environment_references_length = r.read_u16::<LittleEndian>()?;
        let num_environment_references = r.read_u16::<LittleEndian>()?;
        ensure!(environment_references_length == 0);
        ensure!(num_environment_references == 0);
        let hierarchical_schema_section = r.read_u16::<LittleEndian>()?;
        let hierarchical_schema_reference_length = r.read_u16::<LittleEndian>()?;
        let decision_info_section = r.read_u16::<LittleEndian>()?;
        let resource_value_type_table_size = r.read_u16::<LittleEndian>()? as usize;
        let item_to_item_info_group_count = r.read_u16::<LittleEndian>()? as usize;
        let item_info_group_count = r.read_u16::<LittleEndian>()? as usize;
        let item_info_count = r.read_u32::<LittleEndian>()? as usize;
        let num_candidates = r.read_u32::<LittleEndian>()? as usize;
        let data_length = r.read_u32::<LittleEndian>()?;
        let large_table_length = r.read_u32::<LittleEndian>()?;
        ensure!(large_table_length == 0);
        let mut resource_value_type_table = Vec::with_capacity(resource_value_type_table_size);
        for _ in 0..resource_value_type_table_size {
            ensure!(r.read_u32::<LittleEndian>()? == 4);
            let resource_value_type = r.read_u32::<LittleEndian>()?;
            resource_value_type_table.push(resource_value_type);
        }
        let mut item_to_item_info_groups = Vec::with_capacity(item_to_item_info_group_count);
        for _ in 0..item_to_item_info_group_count {
            let first_item = r.read_u16::<LittleEndian>()? as u32;
            let item_info_group = r.read_u16::<LittleEndian>()? as u32;
            item_to_item_info_groups.push(ItemToItemInfoGroup {
                first_item,
                item_info_group,
            });
        }
        let mut item_info_groups = Vec::with_capacity(item_info_group_count);
        for _ in 0..item_info_group_count {
            let group_size = r.read_u16::<LittleEndian>()? as u32;
            let first_item_info = r.read_u16::<LittleEndian>()? as u32;
            item_info_groups.push(ItemInfoGroup {
                group_size,
                first_item_info,
            });
        }
        let mut item_infos = Vec::with_capacity(item_info_count);
        for _ in 0..item_info_count {
            let decision = r.read_u16::<LittleEndian>()? as u32;
            let first_candidate = r.read_u16::<LittleEndian>()? as u32;
            item_infos.push(ItemInfo {
                decision,
                first_candidate,
            });
        }
        /*if large_table_length > 0 {
            let item_to_item_info_group_count_large = r.read_u32::<LittleEndian>()?;
            let item_info_group_count_large = r.read_u32::<LittleEndian>()?;
            let item_info_count_large = r.read_u32::<LittleEndian>()?;
            for _ in 0..item_to_item_info_group_count_large {
                let first_item = r.read_u32::<LittleEndian>()?;
                let item_info_group = r.read_u32::<LittleEndian>()?;
                item_to_item_info_groups.push(ItemToItemInfoGroup {
                    first_item,
                    item_info_group,
                });
            }
            for _ in 0..item_info_group_count_large {
                let group_size = r.read_u32::<LittleEndian>()?;
                let first_item_info = r.read_u32::<LittleEndian>()?;
                item_info_groups.push(ItemInfoGroup {
                    group_size,
                    first_item_info,
                });
            }
            for _ in 0..item_info_count_large {
                let decision = r.read_u32::<LittleEndian>()?;
                let first_candidate = r.read_u32::<LittleEndian>()?;
                item_infos.push(ItemInfo {
                    decision,
                    first_candidate,
                });
            }
        }*/
        let mut candidate_infos = Vec::with_capacity(num_candidates);
        for _ in 0..num_candidates {
            ensure!(r.read_u8()? == 0x01);
            let resource_value_type = resource_value_type_table[r.read_u8()? as usize];
            let source_file_index = r.read_u16::<LittleEndian>()?;
            let data_item_index = r.read_u16::<LittleEndian>()?;
            let data_item_section = r.read_u16::<LittleEndian>()?;
            candidate_infos.push(CandidateInfo {
                resource_value_type,
                source_file_index,
                data_item_index,
                data_item_section,
            });
        }
        Ok(Self {
            hierarchical_schema_section,
            decision_info_section,
            item_to_item_info_groups,
            item_info_groups,
            item_infos,
            candidate_infos,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
struct ItemToItemInfoGroup {
    first_item: u32,
    item_info_group: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
struct ItemInfoGroup {
    group_size: u32,
    first_item_info: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
struct ItemInfo {
    decision: u32,
    first_candidate: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct CandidateInfo {
    resource_value_type: u32,
    source_file_index: u16,
    data_item_index: u16,
    data_item_section: u16,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceValueType {
    String,
    Path,
    EmbeddedData,
    AsciiString,
    Utf8String,
    AsciiPath,
    Utf8Path,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CandidateSet {
    resource_map_item: u32,
    decision_index: u16,
    candidates: Vec<Candidate>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Candidate {
    qualifier_set: u16,
    ty: ResourceValueType,
    data_item_section: u16,
    data_item_index: u16,
}
