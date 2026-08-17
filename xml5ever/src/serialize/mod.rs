// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::QualName;
pub use markup5ever::serialize::{AttrRef, Serialize, Serializer, TraversalScope};
use markup5ever::{LocalName, Namespace, Prefix};
use std::{
    collections::BTreeMap,
    io::{self, Write},
};

#[derive(Clone)]
/// Struct for setting serializer options.
pub struct SerializeOpts {
    /// Serialize the root node? Default: ChildrenOnly
    pub traversal_scope: TraversalScope,
}

impl Default for SerializeOpts {
    fn default() -> SerializeOpts {
        SerializeOpts {
            traversal_scope: TraversalScope::ChildrenOnly(None),
        }
    }
}

/// Struct used for serializing nodes into a text that other XML
/// parsers can read.
///
/// Serializer contains a set of functions (start_elem, end_elem...)
/// that make serializing nodes easier.
pub struct XmlSerializer<Wr> {
    writer: Wr,
    prefix_index: u32,
}

/// For each `Namespace` that has been encountered so far, this contains the list of prefixes that
/// are defined to map to that namespace.
#[derive(Clone, Debug)]
pub struct NamespacePrefixMap {
    scope: BTreeMap<Namespace, Vec<Prefix>>,
}

impl Default for NamespacePrefixMap {
    /// Create a new `NamespacePrefixMap`, containing only the "xml" prefix for the XML namespace.
    fn default() -> Self {
        let mut scope = BTreeMap::new();
        scope.insert(ns!(xml), vec![namespace_prefix!(xml)]);
        NamespacePrefixMap { scope }
    }
}

impl NamespacePrefixMap {
    fn find(&self, ns: &Namespace, prefix: &Prefix) -> bool {
        let Some(list) = self.scope.get(ns) else {
            return false;
        };
        list.contains(prefix)
    }

    fn add(&mut self, ns: Namespace, prefix: Prefix) {
        self.scope.entry(ns).or_default().push(prefix);
    }

    fn retrieve(&self, ns: &Namespace, preferred: &Option<Prefix>) -> Option<Prefix> {
        let candidates = self.scope.get(ns)?;
        if let Some(preferred) = preferred {
            if candidates.contains(preferred) {
                return Some(preferred.clone());
            }
        }
        Some(
            candidates
                .last()
                .expect("candidates should be non-empty")
                .clone(),
        )
    }

    fn generate_a_prefix(&mut self, ns: &Namespace, prefix_index: &mut u32) -> Prefix {
        // Note: https://github.com/w3c/DOM-Parsing/issues/44 is reproduced faithfully here.
        let generated_prefix = Prefix::from(format!("ns{}", *prefix_index));
        *prefix_index += 1;
        self.add(ns.clone(), generated_prefix.clone());
        generated_prefix
    }
}

/// Writes given text into the Serializer, escaping it,
/// depending on where the text is written inside the tag or attribute value.
fn write_to_buf_escaped<W: Write>(writer: &mut W, text: &str, attr_mode: bool) -> io::Result<()> {
    for c in text.chars() {
        match c {
            '&' => writer.write_all(b"&amp;"),
            '"' if attr_mode => writer.write_all(b"&quot;"),
            '<' => writer.write_all(b"&lt;"),
            '>' => writer.write_all(b"&gt;"),
            '\t' if attr_mode => writer.write_all(b"&#9;"),
            '\n' if attr_mode => writer.write_all(b"&#xA;"),
            '\r' if attr_mode => writer.write_all(b"&#xD;"),
            c => writer.write_fmt(format_args!("{c}")),
        }?;
    }
    Ok(())
}

impl<Wr: Write> XmlSerializer<Wr> {
    /// Creates a new Serializier from a writer and given serialization options.
    pub fn new(writer: Wr) -> Self {
        XmlSerializer {
            writer,
            prefix_index: 1,
        }
    }

    fn record_the_namespace_information<'a, AttrIter>(
        &self,
        attributes: AttrIter,
        map: &mut NamespacePrefixMap,
    ) -> (BTreeMap<Prefix, Namespace>, Option<Namespace>)
    where
        AttrIter: Iterator<Item = AttrRef<'a>>,
    {
        // Note: the specification creates `local_prefixes_map` outside this algorithm; our approach
        // equivalent.
        let mut local_prefixes_map: BTreeMap<Prefix, Namespace> = BTreeMap::new();
        // Step 1.
        let mut local_default_namespace: Option<Namespace> = None;
        // Step 2.
        for (name, value) in attributes {
            // Step 2.1.
            let attribute_namespace = &name.ns;
            // Step 2.2.
            let attribute_prefix = &name.prefix;
            // Step 2.3.
            if *attribute_namespace == ns!(xmlns) {
                if attribute_prefix.is_none() {
                    local_default_namespace = Some(Namespace::from(value));
                    continue;
                }
                let prefix_definition = Prefix::from(&*name.local);
                let namespace_definition = Namespace::from(value);
                if namespace_definition == ns!(xmlns) {
                    continue;
                }
                if map.find(&namespace_definition, &prefix_definition) {
                    continue;
                }
                map.add(namespace_definition.clone(), prefix_definition.clone());
                local_prefixes_map.insert(prefix_definition.clone(), namespace_definition.clone());
            }
        }
        (local_prefixes_map, local_default_namespace)
    }

    fn serialize_attribute(
        &mut self,
        prefix: &Option<Prefix>,
        local: &LocalName,
        value: &str,
    ) -> io::Result<()> {
        self.writer.write_all(b" ")?;
        if let Some(ref prefix) = prefix {
            self.writer.write_all(prefix.as_bytes())?;
            self.writer.write_all(b":")?;
        }
        self.writer.write_all(local.as_bytes())?;
        self.writer.write_all(b"=\"")?;
        write_to_buf_escaped(&mut self.writer, value, true)?;
        self.writer.write_all(b"\"")?;
        Ok(())
    }

    fn serialize_attributes<'a, AttrIter>(
        &mut self,
        attributes: AttrIter,
        map: &mut NamespacePrefixMap,
        local_prefixes_map: &mut BTreeMap<Prefix, Namespace>,
        ignore_namespace_definition_attribute: bool,
    ) -> io::Result<()>
    where
        AttrIter: Iterator<Item = AttrRef<'a>>,
    {
        // Step 3.
        for (name, value) in attributes {
            // Step 3.1.
            let attribute_namespace = &name.ns;
            // Step 3.5
            let mut candidate_prefix = None;
            // Step 3.6.
            if *attribute_namespace != ns!() {
                // Step 3.6.1.
                let prefix = &name.prefix;
                // Step 3.6.2.
                candidate_prefix = map.retrieve(attribute_namespace, prefix);
                // Step 3.6.3.
                if *attribute_namespace == ns!(xmlns) {
                    let value = Namespace::from(value);
                    // Step 3.6.3.1.
                    if value == ns!(xml) {
                        continue;
                    }
                    // `xmlns="ns"`
                    if prefix.is_none() && ignore_namespace_definition_attribute {
                        continue;
                    }
                    // `xmlns:foo="ns"`
                    if prefix.is_some() {
                        let local = Prefix::from(&*name.local);
                        let x = local_prefixes_map.get(&local);
                        match x {
                            Some(x) if *x == value => {},
                            _ => {
                                let found = map.find(&value, &local);
                                if found {
                                    continue;
                                }
                            },
                        }
                    }
                    // Step 3.6.3.4.
                    if *prefix == Some(namespace_prefix!(xmlns)) {
                        candidate_prefix = Some(namespace_prefix!(xmlns));
                    }
                // Step 3.6.4.
                } else if candidate_prefix.is_none() {
                    let new_prefix = match prefix {
                        Some(prefix) if !local_prefixes_map.contains_key(prefix) => prefix.clone(),
                        _ => map.generate_a_prefix(attribute_namespace, &mut self.prefix_index),
                    };
                    map.add(attribute_namespace.clone(), new_prefix.clone());
                    local_prefixes_map.insert(new_prefix.clone(), attribute_namespace.clone());
                    candidate_prefix = Some(new_prefix.clone());
                    self.serialize_attribute(
                        &Some(namespace_prefix!(xmlns)),
                        &LocalName::from(&*new_prefix),
                        attribute_namespace,
                    )?;
                }
            }
            self.serialize_attribute(&candidate_prefix, &name.local, value)?;
        }
        Ok(())
    }

    /// This serializes the entire start tag, except for the final `>`. The caller decides whether
    /// to serialize `>` and a separate end tag, or a self-closing `/>`.
    fn start_elem_aux<'a, AttrIter>(
        &mut self,
        name: &QualName,
        attrs: AttrIter,
        namespace: Namespace,
        prefix_map: &NamespacePrefixMap,
    ) -> io::Result<(String, Namespace, NamespacePrefixMap)>
    where
        AttrIter: Iterator<Item = AttrRef<'a>> + Clone,
    {
        let mut extra_attr: Option<(QualName, String)> = None;
        // Step 2.
        self.writer.write_all(b"<")?;

        // Step 3.
        let qualified_name: String;

        // Step 4. (See `write_empty_elem`)

        // Step 5.
        let mut ignore_namespace_definition_attribute = false;

        // Step 6.
        let mut map = prefix_map.clone();

        // Step 7-8.
        let (mut local_prefixes_map, local_default_namespace) =
            self.record_the_namespace_information(attrs.clone(), &mut map);

        // Step 9.
        let mut inherited_ns = namespace;

        // Step 10.
        let ns = &name.ns;

        // Step 11.
        if *inherited_ns == *ns {
            // Step 11.1.
            if local_default_namespace.is_some() {
                ignore_namespace_definition_attribute = true;
            }
            // Step 11.2.
            if *ns == ns!(xml) {
                qualified_name = format!("xml:{}", name.local);
            // Step 11.3.
            } else {
                qualified_name = (*name.local).to_owned();
            }
            // Step 11.4. (see below)
        } else {
            // Step 12.
            // Step 12.1.
            let prefix = &name.prefix;
            // Step 12.3.
            let candidate_prefix = if *prefix == Some(namespace_prefix!(xmlns)) {
                prefix.clone()
            } else {
                // Step 12.2.
                map.retrieve(ns, prefix)
            };
            match candidate_prefix {
                // Step 12.4.
                Some(candidate_prefix) => {
                    qualified_name = format!("{}:{}", candidate_prefix, name.local);
                    match local_default_namespace {
                        Some(ldn) if ldn != ns!(xml) => {
                            inherited_ns = ldn;
                        },
                        _ => {},
                    }
                },
                None => {
                    match prefix {
                        // Step 12.5.
                        Some(prefix) => {
                            let prefix = if local_prefixes_map.contains_key(prefix) {
                                map.generate_a_prefix(ns, &mut self.prefix_index)
                            } else {
                                map.add(ns.clone(), prefix.clone());
                                prefix.clone()
                            };
                            qualified_name = format!("{}:{}", prefix, name.local);
                            debug_assert!(extra_attr.is_none());
                            extra_attr = Some((
                                QualName::new(
                                    Some(namespace_prefix!(xmlns)),
                                    ns!(xmlns),
                                    LocalName::from(&*prefix),
                                ),
                                (**ns).to_owned(),
                            ));

                            match local_default_namespace {
                                Some(ldn) if ldn != ns!(xml) => {
                                    inherited_ns = ldn;
                                },
                                _ => {},
                            }
                        },
                        None => {
                            match local_default_namespace {
                                // Step 12.7
                                Some(ldn) if ldn == *ns => {
                                    qualified_name = (*name.local).to_owned();
                                    inherited_ns = ns.clone();
                                },
                                // Step 12.6
                                _ => {
                                    ignore_namespace_definition_attribute = true;
                                    qualified_name = (*name.local).to_owned();
                                    inherited_ns = ns.clone();
                                    debug_assert!(extra_attr.is_none());
                                    extra_attr = Some((
                                        QualName::new(None, ns!(xmlns), local_name!(xmlns)),
                                        (**ns).to_owned(),
                                    ));
                                },
                            }
                        },
                    }
                },
            }
        }
        self.writer.write_all(qualified_name.as_bytes())?;
        if let Some((name, value)) = extra_attr {
            self.serialize_attribute(&name.prefix, &name.local, &value)?;
        }
        self.serialize_attributes(
            attrs,
            &mut map,
            &mut local_prefixes_map,
            ignore_namespace_definition_attribute,
        )?;
        Ok((qualified_name, inherited_ns, map))
    }
}

impl<Wr: Write> XmlSerializer<Wr> {
    /// Serializes given start element into text. Start element contains
    /// qualified name and a list of attributes.
    pub fn start_elem<'a, AttrIter>(
        &mut self,
        name: &QualName,
        attrs: AttrIter,
        namespace: Namespace,
        prefix_map: &NamespacePrefixMap,
    ) -> io::Result<(String, Namespace, NamespacePrefixMap)>
    where
        AttrIter: Iterator<Item = AttrRef<'a>> + Clone,
    {
        let result = self.start_elem_aux(name, attrs, namespace, prefix_map)?;
        self.writer.write_all(b">")?;
        Ok(result)
    }

    /// Serializes an element without children into text. This may use self-closing syntax,
    /// depending on the situation.
    /// Note: HTML template elements do not necessarily come through this function.
    pub fn write_empty_elem<'a, AttrIter>(
        &mut self,
        name: &QualName,
        attrs: AttrIter,
        namespace: Namespace,
        prefix_map: &NamespacePrefixMap,
    ) -> io::Result<()>
    where
        AttrIter: Iterator<Item = AttrRef<'a>> + Clone,
    {
        let (qualified_name, _, _) = self.start_elem_aux(name, attrs, namespace, prefix_map)?;
        if name.ns == ns!(html) {
            match name.local {
                local_name!("area")
                | local_name!("base")
                | local_name!("basefont")
                | local_name!("bgsound")
                | local_name!("br")
                | local_name!("col")
                | local_name!("embed")
                | local_name!("frame")
                | local_name!("hr")
                | local_name!("img")
                | local_name!("input")
                | local_name!("keygen")
                | local_name!("link")
                | local_name!("menuitem")
                | local_name!("meta")
                | local_name!("param")
                | local_name!("source")
                | local_name!("track")
                | local_name!("wbr") => {
                    self.writer.write_all(b" />")?;
                    return Ok(());
                },
                _ => {},
            }
        } else {
            self.writer.write_all(b"/>")?;
            return Ok(());
        }
        self.writer.write_all(b">")?;
        self.end_elem(qualified_name)?;
        Ok(())
    }

    /// Serialize the end of an element, for example `</div>`.
    pub fn end_elem(&mut self, name: String) -> io::Result<()> {
        self.writer.write_all(b"</")?;
        self.writer.write_all(name.as_bytes())?;
        self.writer.write_all(b">")
    }

    /// Serializes comment into text.
    pub fn write_comment(&mut self, text: &str) -> io::Result<()> {
        self.writer.write_all(b"<!--")?;
        self.writer.write_all(text.as_bytes())?;
        self.writer.write_all(b"-->")
    }

    /// Serializes given doctype
    pub fn write_doctype(&mut self, name: &str) -> io::Result<()> {
        self.writer.write_all(b"<!DOCTYPE ")?;
        self.writer.write_all(name.as_bytes())?;
        self.writer.write_all(b">")
    }

    /// Serializes text for a node or an attributes.
    pub fn write_text(&mut self, text: &str) -> io::Result<()> {
        write_to_buf_escaped(&mut self.writer, text, false)
    }

    /// Serializes given processing instruction.
    pub fn write_processing_instruction(&mut self, target: &str, data: &str) -> io::Result<()> {
        self.writer.write_all(b"<?")?;
        self.writer.write_all(target.as_bytes())?;
        self.writer.write_all(b" ")?;
        self.writer.write_all(data.as_bytes())?;
        self.writer.write_all(b"?>")
    }
}
