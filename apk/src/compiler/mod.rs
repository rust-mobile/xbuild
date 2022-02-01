use crate::manifest::AndroidManifest;
use crate::res::{
    Chunk, ResKey, ResTableConfig, ResTableHeader, ResTablePackageHeader, ResTableRef,
    ResTableTypeHeader, ResTableTypeSpecHeader, ResValue, ScreenType,
};
use anyhow::Result;
use std::collections::{BTreeMap, HashMap};

mod attributes;
mod xml;

pub fn compile_manifest(
    mut manifest: AndroidManifest,
    icon_ref: Option<ResTableRef>,
) -> Result<Chunk> {
    manifest.application.icon = icon_ref.map(|r| r.to_string());
    // TODO: correctly encode attributes
    let mut activity = &mut manifest.application.activity;
    activity.config_changes = activity
        .config_changes
        .as_ref()
        .map(|_| "0x40003fb4".to_string());
    activity.launch_mode = activity.launch_mode.as_ref().map(|_| "1".to_string());
    activity.window_soft_input_mode = activity
        .window_soft_input_mode
        .as_ref()
        .map(|_| "0x10".to_string());

    let xml = quick_xml::se::to_string(&manifest)?;
    xml::compile_xml(&xml)
}

const DPI_SIZE: [u32; 5] = [48, 72, 96, 144, 192];

fn variants(name: &str) -> impl Iterator<Item = (String, u32)> + '_ {
    DPI_SIZE
        .into_iter()
        .map(move |size| (format!("res/{0}/{0}{1}.png", name, size), size))
}

pub fn compile_mipmap<'a>(package_name: &str, name: &'a str) -> Result<Mipmap<'a>> {
    let chunk = Chunk::Table(
        ResTableHeader { package_count: 1 },
        vec![
            Chunk::StringPool(variants(name).map(|(res, _)| res).collect(), vec![]),
            Chunk::TablePackage(
                ResTablePackageHeader {
                    id: 127,
                    name: package_name.to_string(),
                    type_strings: 288,   // TODO
                    last_public_type: 2, // TODO
                    key_strings: 340,    // TODO
                    last_public_key: 1,  // TODO
                    type_id_offset: 0,
                },
                vec![
                    Chunk::StringPool(vec!["mipmap".to_string()], vec![]),
                    Chunk::TableTypeSpec(
                        ResTableTypeSpecHeader {
                            id: 1,
                            res0: 0,
                            res1: 0,
                            entry_count: 1,
                        },
                        vec![256],
                    ),
                    mipmap_table_type(1, 160, 0),
                    mipmap_table_type(1, 240, 1),
                    mipmap_table_type(1, 320, 2),
                    mipmap_table_type(1, 480, 3),
                    mipmap_table_type(1, 640, 4),
                ],
            ),
        ],
    );
    Ok(Mipmap {
        name,
        chunk,
        attr_ref: ResTableRef::new(127, 1, 0),
    })
}

fn mipmap_table_type(type_id: u8, density: u16, string_id: u32) -> Chunk {
    Chunk::TableType(
        ResTableTypeHeader {
            id: type_id,
            res0: 0,
            res1: 0,
            entry_count: 1,
            entries_start: 88,
            config: ResTableConfig {
                size: 32,
                imsi: 0,
                locale: 0,
                screen_type: ScreenType {
                    orientation: 0,
                    touchscreen: 0,
                    density,
                },
                input: 0,
                screen_size: 0,
                version: 4,
                unknown: vec![],
            },
        },
        vec![0],
        vec![(
            ResKey {
                size: 8,
                flags: 0,
                key: 0,
            },
            ResValue {
                size: 8,
                res0: 0,
                data_type: 3,
                data: string_id,
            },
        )],
    )
}

pub struct Mipmap<'a> {
    name: &'a str,
    chunk: Chunk,
    attr_ref: ResTableRef,
}

impl<'a> Mipmap<'a> {
    pub fn chunk(&self) -> &Chunk {
        &self.chunk
    }

    pub fn attr_ref(&self) -> ResTableRef {
        self.attr_ref
    }

    pub fn variants(&self) -> impl Iterator<Item = (String, u32)> + 'a {
        variants(self.name)
    }
}

#[derive(Default)]
pub struct Strings {
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

    pub fn contains(&self, s: &str) -> bool {
        self.strings.contains_key(s)
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
