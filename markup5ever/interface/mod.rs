// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;
use tendril::StrTendril;

use super::{LocalName, Prefix, Namespace};
pub use self::tree_builder::{NodeOrText, AppendNode, AppendText, create_element, ElementFlags};
pub use self::tree_builder::{QuirksMode, Quirks, LimitedQuirks, NoQuirks};
pub use self::tree_builder::{TreeSink, Tracer, NextParserState};

/// https://www.w3.org/TR/REC-xml-names/#dt-expname
#[derive(Copy, Clone, Eq, Hash)]
pub struct ExpandedName<'a> {
    pub ns: &'a Namespace,
    pub local: &'a LocalName,
}

impl<'a, 'b> PartialEq<ExpandedName<'a>> for ExpandedName<'b> {
    fn eq(&self, other: &ExpandedName<'a>) -> bool {
        self.ns == other.ns && self.local == other.local
    }
}

impl<'a> fmt::Debug for ExpandedName<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.ns.is_empty() {
            write!(f, "{}", self.local)
        } else {
            write!(f, "{{{}}}:{}", self.ns, self.local)
        }
    }
}

#[macro_export]
macro_rules! expanded_name {
    ("", $local: tt) => {
        $crate::interface::ExpandedName {
            ns: &ns!(),
            local: &local_name!($local),
        }
    };
    ($ns: ident $local: tt) => {
        $crate::interface::ExpandedName {
            ns: &ns!($ns),
            local: &local_name!($local),
        }
    }
}

pub mod tree_builder;

/// A name with a namespace.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
/// Fully qualified name. Used to depict names of tags and attributes.
///
/// Used to differentiate between similar XML fragments. For example:
///
/// ```ignore
/// // HTML
/// <table>
///   <tr>
///     <td>Apples</td>
///     <td>Bananas</td>
///   </tr>
/// </table>
///
/// // Furniture XML
/// <table>
///   <name>African Coffee Table</name>
///   <width>80</width>
///   <length>120</length>
/// </table>
/// ```
///
/// Without XML namespaces, we can't use those two fragments in the same document
/// at the same time. However if we declare a namespace we could instead say:
///
/// ```ignore
/// // Furniture XML
/// <furn:table>
///   <furn:name>African Coffee Table</furn:name>
///   <furn:width>80</furn:width>
///   <furn:length>120</furn:length>
/// </furn:table>
/// ```
///
/// and bind it to a different name.
///
/// For this reason we parse names that contain a colon in the following way:
///
/// ```ignore
/// <furn:table>
///    |    |
///    |    +- local name
///    |
///  prefix (when resolved gives namespace_url)
/// ```
pub struct QualName {
    pub prefix: Option<Prefix>,
    pub ns: Namespace,
    pub local: LocalName,
}

impl QualName {
    #[inline]
    pub fn new(prefix: Option<Prefix>, ns: Namespace, local: LocalName) -> QualName {
        QualName {
            prefix: prefix,
            ns: ns,
            local: local,
        }
    }

    #[inline]
    pub fn expanded(&self) -> ExpandedName {
        ExpandedName {
            ns: &self.ns,
            local: &self.local
        }
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
    use super::Namespace;

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
}
