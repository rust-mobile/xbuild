use crate::manifest::AndroidManifest;
use crate::res::{
    Chunk, ResValue, ResXmlAttribute, ResXmlEndElement, ResXmlNamespace, ResXmlNodeHeader,
    ResXmlStartElement,
};
use anyhow::Result;
use roxmltree::{Attribute, Document, Node, NodeType};
use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::path::Path;

static ATTRIBUTES: &[AAttribute<'static>] = &[
    AAttribute::new("compileSdkVersion", Some(16844146), DataType::IntDec),
    AAttribute::new(
        "compileSdkVersionCodename",
        Some(16844147),
        DataType::String,
    ),
    AAttribute::new("minSdkVersion", Some(16843276), DataType::IntDec),
    AAttribute::new("targetSdkVersion", Some(16843376), DataType::IntDec),
    AAttribute::new("name", Some(16842755), DataType::String),
    AAttribute::new("label", Some(16842753), DataType::String),
    AAttribute::new("debuggable", Some(16842767), DataType::IntBoolean),
    AAttribute::new("appComponentFactory", Some(16844154), DataType::String),
    AAttribute::new("exported", Some(16842768), DataType::IntBoolean),
    AAttribute::new("launchMode", Some(16842781), DataType::IntDec),
    AAttribute::new("configChanges", Some(16842783), DataType::IntHex),
    AAttribute::new("windowSoftInputMode", Some(16843307), DataType::IntHex),
    AAttribute::new("hardwareAccelerated", Some(16843475), DataType::IntBoolean),
    AAttribute::new("value", Some(16842788), DataType::IntDec),
    AAttribute::new("package", None, DataType::String),
    AAttribute::new("platformBuildVersionCode", None, DataType::IntDec),
    AAttribute::new("platformBuildVersionName", None, DataType::IntDec),
];

struct AAttribute<'a> {
    name: &'a str,
    res_id: Option<u32>,
    ty: DataType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum DataType {
    //Null = 0x00,
    //Reference = 0x01,
    //Attribute = 0x02,
    String = 0x03,
    //Float = 0x04,
    //Dimension = 0x05,
    //Fraction = 0x06,
    IntDec = 0x10,
    IntHex = 0x11,
    IntBoolean = 0x12,
    //IntColorArgb8 = 0x1c,
    //IntColorRgb8 = 0x1d,
    //IntColorArgb4 = 0x1e,
    //IntColorRgb4 = 0x1f,
}

impl AAttribute<'static> {
    const fn new(name: &'static str, res_id: Option<u32>, ty: DataType) -> Self {
        Self { name, res_id, ty }
    }
}

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
        fn create_resource_map(
            node: Node,
            strings: &mut Strings,
            map: &mut Vec<u32>,
        ) -> Result<()> {
            for attr in node.attributes() {
                if !strings.contains(attr.name()) {
                    if let Some(info) = ATTRIBUTES.iter().find(|a| a.name == attr.name()) {
                        if let Some(res_id) = info.res_id {
                            strings.id(attr.name());
                            map.push(res_id);
                        }
                    } else {
                        anyhow::bail!("unsupported attribute {}", attr.name());
                    }
                }
            }
            for node in node.children() {
                create_resource_map(node, strings, map)?;
            }
            Ok(())
        }
        fn compile_attr<'a>(attr: &'a Attribute, strings: &mut Strings) -> Result<ResXmlAttribute> {
            let info = ATTRIBUTES
                .iter()
                .find(|a| a.name == attr.name())
                .expect("creating resource map failed");
            // TODO: temporary hack
            let value = match attr.name() {
                "configChanges" => "0x40003fb4",
                "windowSoftInputMode" => "0x10",
                "launchMode" => "1",
                _ => attr.value(),
            };
            let data = match info.ty {
                DataType::String => strings.id(value) as u32,
                DataType::IntDec => value.parse()?,
                DataType::IntHex => {
                    anyhow::ensure!(&value[..2] == "0x");
                    u32::from_str_radix(&value[2..], 16)?
                }
                DataType::IntBoolean => match value {
                    "true" => 0xffff_ffff,
                    "false" => 0x0000_0000,
                    _ => anyhow::bail!("expected boolean"),
                },
            };
            let raw_value = if info.ty == DataType::String {
                strings.id(value)
            } else {
                -1
            };
            Ok(ResXmlAttribute {
                namespace: attr.namespace().map(|ns| strings.id(ns)).unwrap_or(-1),
                name: strings.id(attr.name()),
                raw_value,
                typed_value: ResValue {
                    size: 8,
                    res0: 0,
                    data_type: info.ty as u8,
                    data,
                },
            })
        }

        fn compile_node(node: Node, strings: &mut Strings, chunks: &mut Vec<Chunk>) -> Result<()> {
            if node.node_type() != NodeType::Element {
                for node in node.children() {
                    compile_node(node, strings, chunks)?;
                }
                return Ok(());
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
                attrs.push(compile_attr(attr, strings)?);
            }
            let namespace = node
                .tag_name()
                .namespace()
                .map(|ns| strings.id(ns))
                .unwrap_or(-1);
            let name = strings.id(node.tag_name().name());
            chunks.push(Chunk::XmlStartElement(
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
            ));
            for node in node.children() {
                compile_node(node, strings, chunks)?;
            }
            chunks.push(Chunk::XmlEndElement(
                ResXmlNodeHeader::default(),
                ResXmlEndElement { namespace, name },
            ));
            Ok(())
        }

        let doc = Document::parse(&self.0)?;
        let mut strings = Strings::default();
        let mut chunks = vec![Chunk::Null];
        let root = doc.root_element();
        let mut map = vec![];
        create_resource_map(root, &mut strings, &mut map)?;
        chunks.push(Chunk::XmlResourceMap(map));
        for ns in root.namespaces() {
            chunks.push(Chunk::XmlStartNamespace(
                ResXmlNodeHeader::default(),
                ResXmlNamespace {
                    prefix: ns.name().map(|ns| strings.id(ns)).unwrap_or(-1),
                    uri: strings.id(ns.uri()),
                },
            ));
        }
        compile_node(root, &mut strings, &mut chunks)?;
        for ns in root.namespaces() {
            chunks.push(Chunk::XmlEndNamespace(
                ResXmlNodeHeader::default(),
                ResXmlNamespace {
                    prefix: ns.name().map(|ns| strings.id(ns)).unwrap_or(-1),
                    uri: strings.id(ns.uri()),
                },
            ));
        }
        let strings = strings.finalize();
        chunks[0] = Chunk::StringPool(strings, vec![]);
        let mut buf = vec![];
        let mut w = Cursor::new(&mut buf);
        Chunk::Xml(chunks).write(&mut w)?;
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
