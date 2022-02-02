use crate::res::{ResValue, ResXmlAttribute};
use anyhow::Result;
use cafebabe::attributes::AttributeData;
use cafebabe::constant_pool::LiteralConstant;
use roxmltree::Attribute;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use zip::ZipArchive;

// While the R values for all attributes except `compileSdkVersion` and
// `compileSdkVersionCodename` can be found quite easily by parsing the
// `android.jar`, it's not clear why the mentioned attributes need special
// casing or how to determine the data type.
static ATTRIBUTES: &[AAttribute<'static>] = &[
    AAttribute::new("label", Some(16842753), DataType::String),
    AAttribute::new("icon", Some(16842754), DataType::Reference),
    AAttribute::new("name", Some(16842755), DataType::String),
    AAttribute::new("hasCode", Some(16842764), DataType::IntBoolean),
    AAttribute::new("debuggable", Some(16842767), DataType::IntBoolean),
    AAttribute::new("exported", Some(16842768), DataType::IntBoolean),
    AAttribute::new("launchMode", Some(16842781), DataType::IntDec),
    AAttribute::new("configChanges", Some(16842783), DataType::IntHex),
    AAttribute::new("value", Some(16842788), DataType::IntDec),
    AAttribute::new("minSdkVersion", Some(16843276), DataType::IntDec),
    AAttribute::new("versionCode", Some(16843291), DataType::IntDec),
    AAttribute::new("versionName", Some(16843292), DataType::String),
    AAttribute::new("windowSoftInputMode", Some(16843307), DataType::IntHex),
    AAttribute::new("targetSdkVersion", Some(16843376), DataType::IntDec),
    AAttribute::new("hardwareAccelerated", Some(16843475), DataType::IntBoolean),
    AAttribute::new("compileSdkVersion", Some(16844146), DataType::IntDec),
    AAttribute::new(
        "compileSdkVersionCodename",
        Some(16844147),
        DataType::String,
    ),
    AAttribute::new("appComponentFactory", Some(16844154), DataType::String),
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
    Reference = 0x01,
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

pub fn compile_attr(attr: &Attribute, strings: &Strings) -> Result<ResXmlAttribute> {
    let info = ATTRIBUTES
        .iter()
        .find(|a| a.name == attr.name())
        .expect("creating resource map failed");
    let value = attr.value();
    let data = match info.ty {
        DataType::String => strings.id(value) as u32,
        DataType::Reference => value.parse()?,
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

#[derive(Default)]
pub struct StringPoolBuilder<'a> {
    attributes: BTreeMap<u32, &'a str>,
    strings: BTreeSet<&'a str>,
}

impl<'a> StringPoolBuilder<'a> {
    pub fn add_attribute(&mut self, attr: &'a Attribute<'a>) -> Result<()> {
        if let Some(info) = ATTRIBUTES.iter().find(|a| a.name == attr.name()) {
            if let Some(res_id) = info.res_id {
                self.attributes.insert(res_id, attr.name());
            } else {
                self.strings.insert(attr.name());
            }
            if info.ty == DataType::String {
                self.strings.insert(attr.value());
            }
        } else {
            anyhow::bail!("unsupported attribute {}", attr.name());
        }
        Ok(())
    }

    pub fn add_string(&mut self, s: &'a str) {
        self.strings.insert(s);
    }

    pub fn build(self) -> Strings {
        let mut strings = Vec::with_capacity(self.attributes.len() + self.strings.len());
        let mut map = Vec::with_capacity(self.attributes.len());
        for (id, name) in self.attributes {
            strings.push(name.to_string());
            map.push(id);
        }
        for string in self.strings {
            strings.push(string.to_string());
        }
        Strings { strings, map }
    }
}

pub struct Strings {
    pub strings: Vec<String>,
    pub map: Vec<u32>,
}

impl Strings {
    pub fn id(&self, s2: &str) -> i32 {
        self.strings
            .iter()
            .position(|s| s == s2)
            .expect("all strings added to the string pool") as i32
    }
}

pub fn r_value(jar: &Path, class_name: &str, field_name: &str) -> Result<i32> {
    let mut zip = ZipArchive::new(BufReader::new(File::open(jar)?))?;
    let mut f = zip.by_name(&format!("android/R${}.class", class_name))?;
    let mut buf = vec![];
    f.read_to_end(&mut buf)?;
    let class = cafebabe::parse_class(&buf).map_err(|err| anyhow::anyhow!("{}", err))?;
    let field = class
        .fields
        .iter()
        .find(|field| field.name == field_name)
        .ok_or_else(|| anyhow::anyhow!("failed to locate field {}", field_name))?;
    let attr = field
        .attributes
        .iter()
        .find(|attr| attr.name == "ConstantValue")
        .ok_or_else(|| anyhow::anyhow!("field is not a constant {}", field_name))?;
    let i = match attr.data {
        AttributeData::ConstantValue(LiteralConstant::Integer(i)) => i,
        _ => anyhow::bail!("unexpected type {:?}", attr.data),
    };
    Ok(i)
}
