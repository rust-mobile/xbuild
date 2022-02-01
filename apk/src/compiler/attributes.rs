use crate::compiler::Strings;
use crate::res::{ResValue, ResXmlAttribute};
use anyhow::Result;
use roxmltree::{Attribute, Node};

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

pub fn compile_attr(attr: &Attribute, strings: &mut Strings) -> Result<ResXmlAttribute> {
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

pub fn create_resource_map(node: Node, strings: &mut Strings, map: &mut Vec<u32>) -> Result<()> {
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
