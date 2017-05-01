// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use tendril::StrTendril;

pub mod tree_builder;

use super::{LocalName, Prefix, Namespace};
pub use self::tree_builder::{NodeOrText, AppendNode, AppendText};
pub use self::tree_builder::{QuirksMode, Quirks, LimitedQuirks, NoQuirks};
pub use self::tree_builder::{TreeSink, Tracer, NextParserState};

/// https://www.w3.org/TR/REC-xml-names/#dt-expname
pub type ExpandedName<'a> = (&'a Namespace, &'a LocalName);

#[macro_export]
macro_rules! expanded_name {
    ($ns: ident $local: tt) => {
        (&ns!($ns), &local_name!($local))
    }
}

/// A name with a namespace.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
#[cfg_attr(feature = "heap_size", derive(HeapSizeOf))]
/// Fully qualified name. Used to depict names of tags and attributes.
///
/// Used to differentiate between similar XML fragments. For example
/// ```ignore
///    // HTML
///    <table>
///      <tr>
///        <td>Apples</td>
///        <td>Bananas</td>
///      </tr>
///    </table>
///
///    // Furniture XML
///    <table>
///      <name>African Coffee Table</name>
///      <width>80</width>
///      <length>120</length>
///    </table>
/// ```
/// Without XML namespaces we can't use those two fragments in occur
/// XML at same time. however if we declare a namespace we could instead say:
///
/// ```ignore
///    // Furniture XML
///    <furn:table>
///      <furn:name>African Coffee Table</furn:name>
///      <furn:width>80</furn:width>
///      <furn:length>120</furn:length>
///    </furn:table>
/// ```
/// and bind it to a different name.
///
/// For this reason we parse names that contain a colon in the following way
///
/// ```ignore
///    < furn:table>
///        |    |
///        |    +- local name
///        |
///      prefix (when resolved gives namespace_url)
/// ```
pub struct QualName {
    pub ns: Namespace,
    pub local: LocalName,
    pub prefix: Option<Prefix>,
}

impl QualName {
    #[inline]
    pub fn new(ns: Namespace, local: LocalName) -> QualName {
        QualName {
            ns: ns,
            local: local,
            prefix: None,
        }
    }

    #[inline]
    pub fn new_localname(local: LocalName) -> QualName {
        QualName {
            ns: ns!(),
            local: local,
            prefix: None,
        }
    }

    #[inline]
    pub fn new_prefixed(prefix: Prefix, local: LocalName) -> QualName {
        QualName {
            ns: ns!(),
            local: local,
            prefix: Some(prefix),
        }
    }

    #[inline]
    pub fn expanded(&self) -> ExpandedName {
        (&self.ns, &self.local)
    }
}

/// A tag attribute.
///
/// The namespace on the attribute name is almost always ns!("").
/// The tokenizer creates all attributes this way, but the tree
/// builder will adjust certain attribute names inside foreign
/// content (MathML, SVG).
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct Attribute {
    pub name: QualName,
    pub value: StrTendril,
}


#[cfg(test)]
mod tests {
    use super::{Namespace, QualName};
    use LocalName;

    #[test]
    fn ns_macro() {
        assert_eq!(ns!(),       Namespace::from(""));

        assert_eq!(ns!(html),   Namespace::from("http://www.w3.org/1999/xhtml"));
        assert_eq!(ns!(xml),    Namespace::from("http://www.w3.org/XML/1998/namespace"));
        assert_eq!(ns!(xmlns),  Namespace::from("http://www.w3.org/2000/xmlns/"));
        assert_eq!(ns!(xlink),  Namespace::from("http://www.w3.org/1999/xlink"));
        assert_eq!(ns!(svg),    Namespace::from("http://www.w3.org/2000/svg"));
        assert_eq!(ns!(mathml), Namespace::from("http://www.w3.org/1998/Math/MathML"));
    }

    #[test]
    fn qualname() {
        assert_eq!(QualName::new(ns!(), local_name!("")),
                   QualName { ns: ns!(), local: LocalName::from(""), prefix: None });
        assert_eq!(QualName::new(ns!(xml), local_name!("base")),
                   QualName { ns: ns!(xml), local: local_name!("base"), prefix: None });
    }

    #[test]
    fn qualname_macro() {
        assert_eq!(qualname!("", ""), QualName { ns: ns!(), local: local_name!(""), prefix: None });
        assert_eq!(qualname!(xml, "base"), QualName { ns: ns!(xml), local: local_name!("base"), prefix: None });
    }
}
