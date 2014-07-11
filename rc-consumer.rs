#![feature(globs)]

use std::rc::{Rc, Weak};
use std::cell::RefCell;
use std::iter::Repeat;

use tree_builder::*;

mod tree_builder;

struct Node {
    pub name: StrBuf,
    pub parent: Option<WeakHandle>,
    pub children: Vec<Handle>,
}

impl Node {
    fn parent(&self) -> Handle {
        self.parent.as_ref().expect("no parent!").upgrade()
    }
}

#[unsafe_destructor]
impl Drop for Node {
    fn drop(&mut self) {
        println!("deleting {:s}", self.name);
    }
}

#[deriving(Clone)]
struct Handle {
    ptr: Rc<RefCell<Node>>,
}

impl Handle {
    fn new(n: Node) -> Handle {
        Handle {
            ptr: Rc::new(RefCell::new(n)),
        }
    }
}

// Implement an object-identity Eq for use by position_elem().
impl Eq for Handle {
    fn eq(&self, other: &Handle) -> bool {
        // FIXME: This shouldn't really need to touch the borrow flags, right?
        (&*self.ptr.borrow() as *Node) == (&*other.ptr.borrow() as *Node)
    }
}

impl Deref<Rc<RefCell<Node>>> for Handle {
    fn deref<'a>(&'a self) -> &'a Rc<RefCell<Node>> {
        &self.ptr
    }
}

// Don't implement DerefMut, because of mozilla/rust#12825
// We don't need it anyway, because RefCell gives interior
// mutability from an immutable ref.

struct WeakHandle {
    ptr: Weak<RefCell<Node>>,
}

impl WeakHandle {
    fn new(h: Handle) -> WeakHandle {
        WeakHandle {
            ptr: h.downgrade(),
        }
    }

    fn upgrade(&self) -> Handle {
        Handle {
            ptr: self.ptr.upgrade().expect("dangling weak pointer!"),
        }
    }
}

struct Sink {
    root: Option<Handle>,
}

impl TreeSink<Handle> for Sink {
    fn create_element(&mut self, name: Atom) -> Handle {
        println!("creating {:s}", name);
        Handle::new(Node {
            name: name,
            children: vec!(),
            parent: None,
        })
    }

    fn create_html_element_set_as_root(&mut self) -> Handle {
        let h = self.create_element(StrBuf::from_str("html"));
        self.root = Some(h.clone());
        h
    }

    fn detach_from_parent(&mut self, child_hdl: Handle) {
        {
            let child = child_hdl.borrow();
            println!("detaching {:s}", child.name);

            let parent = (*child).parent();
            let mut parent = parent.borrow_mut();
            let i = parent.children.as_slice().position_elem(&child_hdl).expect("not found!");
            parent.children.remove(i).expect("not found!");
        }

        let mut child = child_hdl.borrow_mut();
        (*child).parent = None;
    }

    fn append_element(&mut self, parent_hdl: Handle, child_hdl: Handle) {
        let mut parent = parent_hdl.borrow_mut();
        let mut child = child_hdl.borrow_mut();

        println!("appending {:s} to {:s}", child.name, parent.name);
        (*child).parent = Some(WeakHandle::new(parent_hdl.clone()));
        parent.children.push(child_hdl.clone());
    }
}

fn walk(node: Handle, depth: uint) {
    let node = node.borrow();
    let spaces: StrBuf = Repeat::new(' ').take(depth).collect();

    println!("{:s}<{:s}>", spaces, node.name);
    for c in node.children.iter() {
        walk(c.clone(), depth+4);
    }
    println!("{:s}</{:s}>", spaces, node.name);
}

fn main() {
    let mut s = Sink {
        root: None,
    };

    {
        let mut b = TreeBuilder::new(&mut s);
        b.build();
    }

    walk(s.root.expect("no root!").clone(), 0);
}
