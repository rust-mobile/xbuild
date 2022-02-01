use crate::compiler::Strings;
use crate::res::{Chunk, ResXmlEndElement, ResXmlNamespace, ResXmlNodeHeader, ResXmlStartElement};
use anyhow::Result;
use roxmltree::{Document, Node, NodeType};

pub fn compile_xml(xml: &str) -> Result<Chunk> {
    let doc = Document::parse(xml)?;
    let mut strings = Strings::default();
    let mut chunks = vec![Chunk::Null];
    let root = doc.root_element();
    let mut map = vec![];
    super::attributes::create_resource_map(root, &mut strings, &mut map)?;
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
    Ok(Chunk::Xml(chunks))
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
        attrs.push(super::attributes::compile_attr(attr, strings)?);
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
