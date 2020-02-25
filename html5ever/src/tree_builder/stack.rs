use std::collections::HashMap;
use std::marker::PhantomData;

use super::{namespace_url, ns, ExpandedName, LocalName, Namespace, TreeSink};

// Number of elements to scan through in the stack of open elements,
// before switching to the hashmap based index.
const SCAN_THRESHOLD: usize = 100;

#[derive(Hash, PartialEq, Eq)]
struct ElemName {
    ns: Namespace,
    local: LocalName,
}

impl ElemName {
    fn expanded(&self) -> ExpandedName<'_> {
        ExpandedName {
            ns: &self.ns,
            local: &self.local,
        }
    }
}

pub struct ElemStack<Handle, Sink> {
    open_elems: Vec<Handle>,
    elem_index: Option<HashMap<ElemName, Vec<usize>>>,
    marker: PhantomData<Sink>,
}

impl<Handle, Sink> ElemStack<Handle, Sink>
where
    Handle: Clone,
    Sink: TreeSink<Handle = Handle>,
{
    pub fn new() -> Self {
        ElemStack {
            open_elems: Vec::new(),
            elem_index: None,
            marker: PhantomData,
        }
    }

    fn build_index(&mut self, sink: &Sink) {
        let mut elem_index = HashMap::<_, Vec<_>>::new();
        for (index, elem) in self.open_elems.iter().enumerate() {
            let name = elem_name(sink, elem);
            elem_index
                .entry(name)
                .and_modify(|v| v.push(index))
                .or_insert_with(|| vec![index]);
        }

        self.elem_index = Some(elem_index);
    }

    /// Return topmost element with name in the `scope` set of names
    pub fn top_index_of_set<TagSet>(&self, scope: TagSet) -> Option<usize>
    where
        TagSet: Fn(ExpandedName) -> bool,
    {
        let elem_index = self.elem_index.as_ref().expect("index is missing");
        elem_index
            .iter()
            .filter(|(n, v)| !v.is_empty() && scope(n.expanded()))
            .map(|(_, v)| v.last().cloned())
            .max()
            .unwrap_or(None)
    }

    /// Return topmost index of an element with the given name
    pub fn top_index_of(&self, local: &LocalName) -> Option<usize> {
        let name = ElemName {
            ns: ns!(html),
            local: local.clone(),
        };
        let elem_index = self.elem_index.as_ref().expect("index is missing");
        elem_index.get(&name).and_then(|v| v.last()).cloned()
    }

    fn scan_in_scope<TagSet, Pred>(
        &self,
        sink: &Sink,
        scope: TagSet,
        pred: Pred,
    ) -> Option<Position>
    where
        TagSet: Fn(ExpandedName) -> bool,
        Pred: Fn(&Handle) -> bool,
    {
        for (index, node) in self
            .open_elems
            .iter()
            .enumerate()
            .rev()
            .take(SCAN_THRESHOLD)
        {
            if pred(node) {
                return Some(Position::Some(index));
            }
            if scope(sink.elem_name(node)) {
                return Some(Position::NotInScope);
            }
        }

        if self.open_elems.len() > SCAN_THRESHOLD {
            None
        } else {
            Some(Position::None)
        }
    }

    pub fn in_scope_named<TagSet>(
        &mut self,
        sink: &Sink,
        scope: TagSet,
        name: &LocalName,
    ) -> Position
    where
        TagSet: Fn(ExpandedName) -> bool,
    {
        if let None = self.elem_index {
            if let Some(res) =
                self.scan_in_scope(sink, &scope, |elem| html_elem_named(sink, elem, name))
            {
                return res;
            }

            self.build_index(sink);
        }

        let elem_depth = self.top_index_of(name);
        let scope_depth = self.top_index_of_set(&scope);

        if let Some(elem_depth) = elem_depth {
            if scope_depth.unwrap_or(0) <= elem_depth {
                return Position::Some(elem_depth);
            }
        }
        if scope_depth.is_some() {
            return Position::NotInScope;
        }

        Position::None
    }

    pub fn rposition(&mut self, sink: &Sink, elem: &Handle) -> Option<usize> {
        if let None = self.elem_index {
            if let Some(res) = self.scan_in_scope(sink, |n| false, |n| sink.same_node(n, elem)) {
                return match res {
                    Position::Some(pos) => return Some(pos),
                    _ => return None,
                };
            }

            self.build_index(sink);
        }

        let elem_index = self.elem_index.as_ref().expect("index is missing");

        elem_index
            .get(&elem_name(sink, elem))
            .and_then(|v| v.last())
            .cloned()
    }

    pub fn push(&mut self, sink: &Sink, elem: &Handle) {
        let index = self.open_elems.len();
        self.open_elems.push(elem.clone());

        if let Some(elem_index) = self.elem_index.as_mut() {
            let name = elem_name(sink, elem);
            elem_index.entry(name).or_insert_with(Vec::new).push(index);
        }
    }

    pub fn pop(&mut self, sink: &Sink) -> Option<Handle> {
        let elem = self.open_elems.pop()?;
        if let Some(elem_index) = self.elem_index.as_mut() {
            let name = elem_name(sink, &elem);
            let index = elem_index
                .get_mut(&name)
                .and_then(Vec::pop)
                .expect("inconsistent stack state");

            debug_assert_eq!(index, self.open_elems.len());
        }

        Some(elem)
    }

    pub fn truncate(&mut self, sink: &Sink, len: usize) {
        while self.open_elems.len() > len {
            self.pop(sink);
        }
    }

    pub fn insert(&mut self, sink: &Sink, index: usize, new_element: Handle) {
        self.open_elems.insert(index, new_element.clone());

        if let Some(elem_index) = self.elem_index.as_mut() {
            let name = sink.elem_name(&new_element);
            for (n, v) in elem_index.iter_mut() {
                let ipos = v.binary_search(&index).unwrap_or_else(|i| i);
                for i in &mut v[ipos..] {
                    *i += 1;
                }
                if name.ns == &n.ns && name.local == &n.local {
                    v.insert(ipos, index);
                }
            }
        }
    }

    pub fn remove(&mut self, index: usize) {
        let elem = self.open_elems.remove(index);

        if let Some(elem_index) = self.elem_index.as_mut() {
            for v in elem_index.values_mut() {
                let ipos = match v.binary_search(&index) {
                    Ok(ipos) => {
                        v.remove(ipos);
                        ipos
                    }
                    Err(ipos) => ipos,
                };
                for i in &mut v[ipos..] {
                    *i -= 1;
                }
            }
        }
    }

    pub fn replace(&mut self, sink: &Sink, index: usize, handle: Handle) {
        let old_handle = std::mem::replace(&mut self.open_elems[index], handle.clone());

        if let Some(elem_index) = self.elem_index.as_mut() {
            let name = elem_name(sink, &old_handle);
            let list = elem_index.get_mut(&name).unwrap();
            let ipos = list.binary_search(&index).unwrap();
            list.remove(ipos);

            let name = elem_name(sink, &handle);
            elem_index
                .entry(name)
                .and_modify(|v| {
                    let pos = v.binary_search(&index).expect_err("duplicate index");
                    v.insert(pos, index);
                })
                .or_insert_with(|| vec![index]);
        }
    }

    pub fn len(&self) -> usize {
        self.open_elems.len()
    }

    pub fn is_empty(&self) -> bool {
        self.open_elems.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<Handle> {
        self.open_elems.iter()
    }

    pub fn last(&self) -> Option<&Handle> {
        self.open_elems.last()
    }

    pub fn drain<R>(&mut self, range: R) -> std::vec::Drain<Handle>
    where
        R: std::ops::RangeBounds<usize>,
    {
        self.open_elems.drain(range)
    }

    pub fn as_ref(&self) -> &[Handle] {
        &self.open_elems
    }
}

impl<Handle, Sink, I> std::ops::Index<I> for ElemStack<Handle, Sink>
where
    I: std::slice::SliceIndex<[Handle]>,
{
    type Output = I::Output;
    fn index(&self, index: I) -> &I::Output {
        self.open_elems.index(index)
    }
}

impl<'a, Handle, Sink> IntoIterator for &'a ElemStack<Handle, Sink> {
    type IntoIter = std::slice::Iter<'a, Handle>;
    type Item = &'a Handle;

    fn into_iter(self) -> Self::IntoIter {
        self.open_elems.iter()
    }
}

fn html_elem_named<Sink: TreeSink>(sink: &Sink, elem: &Sink::Handle, local: &LocalName) -> bool {
    let name = sink.elem_name(elem);
    name.ns == &ns!(html) && name.local == local
}

fn elem_name<Sink: TreeSink>(sink: &Sink, elem: &Sink::Handle) -> ElemName {
    let name = sink.elem_name(elem);
    ElemName {
        ns: name.ns.clone(),
        local: name.local.clone(),
    }
}

/// Result of scoped lookups.
#[derive(Debug, Clone, Copy)]
pub enum Position {
    /// Searched element was found.
    Some(usize),
    /// Scope boundary was reached.
    NotInScope,
    /// Stack was exhausted without finding named element or any of the scope elements.
    None,
}

impl Position {
    pub fn is_some(&self) -> bool {
        match self {
            Position::Some(_) => true,
            _ => false,
        }
    }
}
