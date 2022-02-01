use crate::manifest::AndroidManifest;
use crate::Resources;
use anyhow::Result;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

mod attributes;
mod mipmap;
mod xml;

pub fn compile(manifest: &AndroidManifest, icon: Option<&Path>) -> Result<Resources> {
    let mut resources = None;
    let mut icon_ref = None;
    if let Some(icon) = icon {
        let (chunk, id) = mipmap::compile_mipmap(&icon)?;
        resources = Some(chunk);
        icon_ref = Some(id);
    }
    let xml = quick_xml::se::to_string(manifest)?;
    let manifest = xml::compile_xml(&xml)?;
    Ok(Resources {
        manifest,
        resources,
    })
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
