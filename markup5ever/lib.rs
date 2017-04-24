// Copyright 2016 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![cfg_attr(feature = "heap_size", feature(proc_macro))]
#[cfg(feature = "heap_size")] #[macro_use] extern crate heapsize_derive;
#[cfg(feature = "heap_size")] extern crate heapsize;
extern crate string_cache;
extern crate phf;

pub mod data;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[macro_export]
macro_rules! qualname {
    ("", $local:tt) => {
        $crate::QualName {
            ns: ns!(),
            local: local_name!($local),
        }
    };
    ($ns:tt, $local:tt) => {
        $crate::QualName {
            ns: ns!($ns),
            local: local_name!($local),
        }
    }
}

/// A name with a namespace.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
#[cfg_attr(feature = "heap_size", derive(HeapSizeOf))]
pub struct QualName {
    pub ns: Namespace,
    pub local: LocalName,
}

impl QualName {
    #[inline]
    pub fn new(ns: Namespace, local: LocalName) -> QualName {
        QualName {
            ns: ns,
            local: local,
        }
    }
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
                   QualName { ns: ns!(), local: LocalName::from("") });
        assert_eq!(QualName::new(ns!(xml), local_name!("base")),
                   QualName { ns: ns!(xml), local: local_name!("base") });
    }

    #[test]
    fn qualname_macro() {
        assert_eq!(qualname!("", ""), QualName { ns: ns!(), local: local_name!("") });
        assert_eq!(qualname!(xml, "base"), QualName { ns: ns!(xml), local: local_name!("base") });
    }
}
