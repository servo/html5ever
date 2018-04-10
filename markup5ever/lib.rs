// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate string_cache;
extern crate phf;
pub extern crate tendril;

/// Create a [`SmallCharSet`], with each space-separated number stored in the set.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate markup5ever;
/// # fn main() {
/// let set = small_char_set!(12 54 42);
/// assert_eq!(set.bits,
///            0b00000000_01000000_00000100_00000000_00000000_00000000_00010000_00000000);
/// # }
/// ```
///
/// [`SmallCharSet`]: struct.SmallCharSet.html
#[macro_export]
macro_rules! small_char_set ( ($($e:expr)+) => (
    $ crate ::SmallCharSet {
        bits: $( (1 << ($e as usize)) )|+
    }
));

include!(concat!(env!("OUT_DIR"), "/generated.rs"));


pub mod data;
#[macro_use] pub mod interface;
pub mod rcdom;
pub mod serialize;
mod util {
    pub mod smallcharset;
    pub mod buffer_queue;
}

pub use util::*;
pub use interface::{ExpandedName, QualName, Attribute};
pub use util::smallcharset::SmallCharSet;



#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use tendril::SliceExt;

    use super::util::buffer_queue::{BufferQueue, FromSet, NotFromSet};

    #[test]
    fn smoke_test() {
        let mut bq = BufferQueue::new();
        assert_eq!(bq.peek(), None);
        assert_eq!(bq.next(), None);

        bq.push_back("abc".to_tendril());
        assert_eq!(bq.peek(), Some('a'));
        assert_eq!(bq.next(), Some('a'));
        assert_eq!(bq.peek(), Some('b'));
        assert_eq!(bq.peek(), Some('b'));
        assert_eq!(bq.next(), Some('b'));
        assert_eq!(bq.peek(), Some('c'));
        assert_eq!(bq.next(), Some('c'));
        assert_eq!(bq.peek(), None);
        assert_eq!(bq.next(), None);
    }

    #[test]
    fn can_unconsume() {
        let mut bq = BufferQueue::new();
        bq.push_back("abc".to_tendril());
        assert_eq!(bq.next(), Some('a'));

        bq.push_front("xy".to_tendril());
        assert_eq!(bq.next(), Some('x'));
        assert_eq!(bq.next(), Some('y'));
        assert_eq!(bq.next(), Some('b'));
        assert_eq!(bq.next(), Some('c'));
        assert_eq!(bq.next(), None);
    }

    #[test]
    fn can_pop_except_set() {
        let mut bq = BufferQueue::new();
        bq.push_back("abc&def".to_tendril());
        let mut pop = || bq.pop_except_from(small_char_set!('&'));
        assert_eq!(pop(), Some(NotFromSet("abc".to_tendril())));
        assert_eq!(pop(), Some(FromSet('&')));
        assert_eq!(pop(), Some(NotFromSet("def".to_tendril())));
        assert_eq!(pop(), None);
    }

    #[test]
    fn can_eat() {
        // This is not very comprehensive.  We rely on the tokenizer
        // integration tests for more thorough testing with many
        // different input buffer splits.
        let mut bq = BufferQueue::new();
        bq.push_back("a".to_tendril());
        bq.push_back("bc".to_tendril());
        assert_eq!(bq.eat("abcd", u8::eq_ignore_ascii_case), None);
        assert_eq!(bq.eat("ax", u8::eq_ignore_ascii_case), Some(false));
        assert_eq!(bq.eat("ab", u8::eq_ignore_ascii_case), Some(true));
        assert_eq!(bq.next(), Some('c'));
        assert_eq!(bq.next(), None);
    }
}
