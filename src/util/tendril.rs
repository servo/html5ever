// Copyright 2015 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::prelude::v1::*;

use std::{mem, fmt, io, str, slice};
use std::raw::{self, Repr};
use std::ops::Deref;
use std::cmp::Ordering;
use std::error::FromError;

use iobuf::{Iobuf, ROIobuf, RWIobuf};

use util::str::AsciiCast;

use self::Tendril_::{Shared, Owned, Ascii};

#[derive(Clone)]
enum Tendril_ {
    Shared(ROIobuf<'static>),
    Ascii(u8),
    Owned(String),
}

/// html5ever's abstraction of strings.
///
/// A tendril either owns its content, or is a slice of a shared buffer.
/// These buffers are managed with non-atomic (thread-local) reference
/// counting, which is very fast.
///
/// Like `String`, `Tendril` implements `Deref<Target = str>`. So you can
/// call string slice methods on `Tendril`, or pass `&Tendril` to a function
/// expecting `&str`.
///
/// Accordingly, the content of a tendril is guaranteed to be valid UTF-8.
/// Take particular care of this when calling `unsafe` functions below!
///
/// The maximum size of a tendril is 1 GB. The safe methods below will
/// `panic!` if a tendril grows beyond that size.
#[derive(Clone)]
pub struct Tendril(Tendril_);

impl PartialEq for Tendril {
    #[inline]
    fn eq(&self, other: &Tendril) -> bool {
        &**self == &**other
    }

    #[inline]
    fn ne(&self, other: &Tendril) -> bool {
        &**self != &**other
    }
}

impl Eq for Tendril { }

impl PartialOrd for Tendril {
    #[inline]
    fn partial_cmp(&self, other: &Tendril) -> Option<Ordering> {
        (&**self).partial_cmp(other)
    }
}

impl Ord for Tendril {
    #[inline]
    fn cmp(&self, other: &Tendril) -> Ordering {
        (&**self).cmp(other)
    }
}

impl fmt::Display for Tendril {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        <str as fmt::Display>::fmt(&*self, fmt)
    }
}

impl fmt::Debug for Tendril {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(fmt, "Tendril[{}](", match self.0 {
            Shared(_) => "shared",
            Ascii(_) => "ascii",
            Owned(_) => "owned",
        }));
        try!(<str as fmt::Debug>::fmt(&*self, fmt));
        try!(write!(fmt, ")"));
        Ok(())
    }
}

impl Deref for Tendril {
    type Target = str;

    #[inline]
    fn deref<'a>(&'a self) -> &'a str {
        match self.0 {
            Shared(ref s) => unsafe {
                mem::transmute(s.as_window_slice())
            },
            Owned(ref s) => s,
            Ascii(ref b) => unsafe {
                str::from_utf8_unchecked(slice::ref_slice(b))
            },
        }
    }
}

/// Interpret the slice as a single ASCII codepoint, if possible.
#[inline(always)]
fn as_single_ascii(x: &str) -> Option<u8> {
    // &str is always valid UTF-8, so a one-byte &str must contain
    // an ASCII character.
    if x.len() == 1 {
        Some(unsafe { *x.as_bytes().get_unchecked(0) })
    } else {
        None
    }
}

/// The maximum size of a tendril is 1 GB.
pub const TENDRIL_MAX_LEN: u32 = 1 << 30;

impl Tendril {
    /// Create a new, empty tendril.
    #[inline]
    pub fn new() -> Tendril {
        Tendril(Owned(String::new()))
    }

    /// Create a tendril from any `IntoTendril` type.
    #[inline]
    pub fn from<T>(x: T) -> Tendril
        where T: IntoTendril,
    {
        x.into_tendril()
    }

    /// Create a tendril from a character.
    #[inline]
    pub fn from_char(c: char) -> Tendril {
        let n = c as usize;
        if n < 0x80 {
            Tendril(Ascii(n as u8))
        } else {
            Tendril(Owned(c.to_string()))
        }
    }

    /// Create a tendril from a `String`, without copying.
    #[inline]
    pub fn owned(s: String) -> Tendril {
        assert!(s.len() < (1 << 31));
        Tendril(Owned(s))
    }

    /// Copy a string to create a tendril which owns its content.
    #[inline]
    pub fn owned_copy(s: &str) -> Tendril {
        if let Some(n) = as_single_ascii(s) {
            Tendril(Ascii(n))
        } else {
            Tendril(Owned(String::from_str(s)))
        }
    }

    /// Copy a string to create a shared buffer which multiple
    /// tendrils can point into.
    ///
    /// See also `subtendril`.
    #[inline]
    pub fn shared_copy(s: &str) -> Tendril {
        Tendril(Shared(ROIobuf::from_str_copy(s)))
    }

    /// Does this tendril point into a shared buffer?
    #[inline]
    pub fn is_shared(&self) -> bool {
        match self.0 {
            Shared(_) => true,
            _ => false,
        }
    }

    /// Get the length of the tendril.
    #[inline]
    pub fn len32(&self) -> u32 {
        match self.0 {
            Shared(ref b) => b.len(),
            Owned(ref s) => s.len() as u32,
            Ascii(_) => 1,
        }
    }

    /// Count how many bytes at the beginning of the tendril
    /// either all match or all don't match the predicate,
    /// and also return whether they match.
    ///
    /// Returns `None` on an empty string.
    pub fn char_run<Pred>(&self, mut pred: Pred) -> Option<(u32, bool)>
        where Pred: FnMut(char) -> bool,
    {
        let (first, rest) = unwrap_or_return!(self.slice_shift_char(), None);
        let matches = pred(first);

        for (idx, ch) in rest.char_indices() {
            if matches != pred(ch) {
                return Some(((idx + first.len_utf8()) as u32, matches));
            }
        }
        Some((self.len32(), matches))
    }

    /// Promotes the tendril to owning its content, and get a
    /// mutable reference.
    ///
    /// This is unsafe because the user must not exceed the 1 GB
    /// size limit!
    #[inline]
    unsafe fn to_mut<'a>(&'a mut self) -> &'a mut String {
        match self.0 {
            Owned(ref mut s) => return s,
            _ => (),
        }

        self.0 = Owned(String::from_str(self));
        match self.0 {
            Owned(ref mut s) => s,
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn check_len(&self) {
        if self.len() > TENDRIL_MAX_LEN as usize {
            panic!("tendril exceeded 1 GB");
        }
    }

    /// Push a character onto the end of the tendril.
    #[inline]
    pub fn push(&mut self, c: char) {
        if self.is_empty() {
            if let Some(a) = c.to_ascii_opt() {
                self.0 = Ascii(a.to_u8());
                return;
            }
        }
        unsafe {
            self.to_mut().push(c);
        }
        self.check_len();
    }

    /// Push a string onto the end of the tendril.
    #[inline]
    pub fn push_str(&mut self, rhs: &str) {
        match rhs.len() {
            0 => return,
            1 if self.is_empty() => {
                if let Some(n) = as_single_ascii(rhs) {
                    self.0 = Ascii(n);
                    return;
                }
            }
            n if n > TENDRIL_MAX_LEN as usize => {
                panic!("attempted to extend tendril by more than 1 GB");
            }

            // Otherwise, 2 * TENDRIL_MAX_LEN does not overflow u32.
            _ => (),
        }
        unsafe {
            self.to_mut().push_str(rhs);
        }
        self.check_len();
    }

    /// Push another tendril onto the end of the tendril.
    #[inline]
    pub fn push_tendril(&mut self, rhs: Tendril) {
        if rhs.is_empty() {
            return;
        }

        if self.is_empty() {
            *self = rhs;
            return;
        }

        // Try to merge adjacent Iobufs.
        if let (&mut Tendril(Shared(ref mut a)), &Tendril(Shared(ref b)))
            = (&mut *self, &rhs)
        {
            if a.extend_with(b).is_ok() {
                return;
            }
        }

        if rhs.len() > TENDRIL_MAX_LEN as usize{
            panic!("attempted to extend tendril by more than 1 GB");
        }

        // Slow path: copy on write.
        unsafe {
            self.to_mut().push_str(&rhs);
        }
        self.check_len();
    }

    /// Truncate the tendril to an empty tendril, without discarding allocations.
    #[inline]
    pub fn clear(&mut self) {
        if let Owned(ref mut s) = self.0 {
            s.truncate(0);
            return;
        }
        self.0 = Owned(String::new());
    }

    /// Remove the front character, if it's `\n`.
    #[inline]
    pub fn pop_front_lf(&mut self) {
        match self.0 {
            Ascii(b'\n') => *self = Tendril::new(),
            Ascii(_) => (),
            Owned(ref mut s) => {
                if s.starts_with("\n") {
                    s.remove(0);
                }
            }
            Shared(ref mut b) => unsafe {
                if b.unsafe_peek_le(0) == b'\n' {
                    b.unsafe_sub_window_from(1);
                }
            }
        }
    }

    /// Slice a tendril.
    ///
    /// The new tendril encompasses bytes in the index range `[from, to)`.
    ///
    /// If possible, the new and old tendrils point into the same shared
    /// buffer.
    ///
    /// This method is `unsafe` because neither bounds checking nor UTF-8
    /// validity checking is guaranteed. If you violate these properties
    /// then all bets are off!
    ///
    /// html5ever uses `subtendril` in certain fast paths, just after
    /// finding a character boundary with a byte-wise scan.
    #[inline]
    pub unsafe fn subtendril(&self, from: u32, to: u32) -> Tendril {
        match *self {
            Tendril(Shared(ref a)) => {
                let mut b = a.clone();
                b.unsafe_sub_window(from, to - from);
                Tendril(Shared(b))
            }
            _ => {
                let b = self.slice_unchecked(from as usize, to as usize);
                Tendril::owned_copy(b)
            }
        }
    }
}

/// Types which can be converted into a `Tendril`.
///
/// The `Tendril` and `String` instances avoid copying the string data.
/// The other instances copy into a new owned buffer.
pub trait IntoTendril {
    fn into_tendril(self) -> Tendril;
}

impl IntoTendril for Tendril {
    #[inline(always)]
    fn into_tendril(self) -> Tendril {
        self
    }
}

impl IntoTendril for String {
    #[inline(always)]
    fn into_tendril(self) -> Tendril {
        Tendril::owned(self)
    }
}

impl<'a> IntoTendril for &'a str {
    #[inline(always)]
    fn into_tendril(self) -> Tendril {
        Tendril::owned_copy(self)
    }
}

impl IntoTendril for char {
    #[inline(always)]
    fn into_tendril(self) -> Tendril {
        Tendril::from_char(self)
    }
}

// Be very careful about overflow if you plan to use these functions in another context!
#[inline(always)]
unsafe fn unsafe_slice<'a>(buf: &'a [u8], from: u32, to: u32) -> &'a [u8] {
    let raw::Slice { data, len } = buf.repr();
    debug_assert!((from as usize) < len);
    debug_assert!((to as usize) <= len);
    slice::from_raw_parts(data.offset(from as isize), (to - from) as usize)
}

#[inline(always)]
unsafe fn unsafe_slice_mut<'a>(buf: &'a mut [u8], from: u32, to: u32) -> &'a mut [u8] {
    let raw::Slice { data, len } = buf.repr();
    debug_assert!((from as usize) < len);
    debug_assert!((to as usize) <= len);
    slice::from_raw_parts_mut(data.offset(from as isize) as *mut u8, (to - from) as usize)
}

// Return the number of bytes at the end of the buffer that make up an incomplete
// but possibly valid UTF-8 character.
//
// This does *not* check UTF-8 validity. Rather it's used to defer
// validity checking for the last few bytes of a buffer, when appropriate.
// However, it's safe to call on arbitrary byte buffers.
#[inline(always)]
fn incomplete_trailing_utf8(buf: &[u8]) -> u32 {
    let n = buf.len();
    if n < 1 { return 0; }

    // There are four patterns of valid-but-incomplete UTF-8:
    //
    //                   ... 110xxxxx
    //          ... 1110xxxx 10xxxxxx
    //          ... 11110xxx 10xxxxxx
    // ... 11110xxx 10xxxxxx 10xxxxxx

    #[inline(always)] fn is_cont(v: u8) -> bool    { v & 0b11_000000 == 0b10_000000 }
    #[inline(always)] fn is_start(v: u8) -> bool   { v & 0b11_000000 == 0b11_000000 }
    #[inline(always)] fn is_start_3(v: u8) -> bool { v & 0b1111_0000 == 0b1110_0000 }
    #[inline(always)] fn is_start_4(v: u8) -> bool { v & 0b11111_000 == 0b11110_000 }

    unsafe {
        let c = *buf.get_unchecked(n-1);
        if is_start(c) { return 1; }

        if is_cont(c) {
            if n <= 1 { return 0; }
            let b = *buf.get_unchecked(n-2);
            if is_start_3(b) || is_start_4(b) { return 2; }

            if is_cont(b) {
                if n <= 2 { return 0; }
                let a = *buf.get_unchecked(n-3);
                if is_start_4(a) { return 3; }
            }
        }
    }

    0
}

/// Iterator which produces tendrils by reading an input stream.
///
/// The tendrils will be backed by shared buffers. They support
/// slicing via `.subtendril()` without a copy.
pub struct TendrilReader<R> {
    dead: bool,
    chunk_size: u32,
    leftover: (u32, [u8; 3]),
    reader: R,
}

impl<R> TendrilReader<R>
    where R: io::Read,
{
    /// Read a UTF-8 input stream as a sequence of tendrils (or errors).
    ///
    /// Each read will attempt to fill a buffer of `chunk_size` bytes.
    ///
    /// # Panics
    ///
    /// If `chunk_size` is less than 4 bytes or greater than 1 GB.
    #[inline]
    pub fn from_utf8(chunk_size: u32, reader: R) -> TendrilReader<R> {
        // A chunk must be big enough to hold any UTF-8 character.
        // Also it must be small enough to fit in an Iobuf.
        // 1GB is only halfway to the Iobuf limit, so we don't worry
        // about going a few bytes over, e.g. when handling leftover
        // UTF-8 bytes.
        assert!(chunk_size >= 4);
        assert!(chunk_size <= TENDRIL_MAX_LEN);
        TendrilReader {
            dead: false,
            chunk_size: chunk_size,
            reader: reader,
            leftover: (0, [0; 3]),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TendrilReaderError {
    IoError(io::Error),
    Utf8Error(str::Utf8Error),
}

impl FromError<io::Error> for TendrilReaderError {
    #[inline]
    fn from_error(err: io::Error) -> TendrilReaderError {
        TendrilReaderError::IoError(err)
    }
}

impl FromError<str::Utf8Error> for TendrilReaderError {
    #[inline]
    fn from_error(err: str::Utf8Error) -> TendrilReaderError {
        TendrilReaderError::Utf8Error(err)
    }
}

impl<R> Iterator for TendrilReader<R>
    where R: io::Read,
{
    type Item = Result<Tendril, TendrilReaderError>;

    fn next(&mut self) -> Option<Result<Tendril, TendrilReaderError>> {
        if self.dead {
            return None;
        }

        let mut buf = RWIobuf::new(self.chunk_size as usize);

        // Copy some leftover bytes from a previous incomplete character,
        // if any.
        let mut size = match self.leftover {
            (0, _) => 0,
            (ref mut n, ref pfx) => {
                debug_assert!(*n <= 3);
                unsafe {
                    // chunk_size >= 4, which is checked in the
                    // TendrilReader constructor.
                    buf.unsafe_poke(0, unsafe_slice(pfx, 0, *n));
                }
                mem::replace(n, 0)
            }
        };

        unsafe {
            if size < self.chunk_size {
                let dest = unsafe_slice_mut(buf.as_mut_window_slice(), size, self.chunk_size);
                match self.reader.read(dest) {
                    Err(e) => return Some(Err(TendrilReaderError::from_error(e))),

                    Ok(0) => {
                        // EOF
                        self.dead = true;
                        return match size {
                            0 => None,
                            _ => Some(Err(TendrilReaderError::from_error(str::Utf8Error::TooShort))),
                        };
                    }

                    Ok(n) => size += n as u32,
                }
            }

            // Trim the window to exclude uninitialized bytes, and set the
            // limit to forbid un-doing this.
            buf.unsafe_sub_to(size);

            // Defer validity checking for the bytes making up an incomplete
            // UTF-8 character at the end, if any.
            let tail_len = incomplete_trailing_utf8(buf.as_window_slice());
            if tail_len > 0 {
                let rest = size - tail_len;
                self.leftover.0 = tail_len;
                buf.unsafe_peek(rest, unsafe_slice_mut(&mut self.leftover.1, 0, tail_len));
                buf.unsafe_sub_window_to(rest);
            }

            // Check UTF-8 validity for the remaining buffer.
            match str::from_utf8(buf.as_window_slice()) {
                Err(e) => Some(Err(TendrilReaderError::from_error(e))),
                Ok(_) => Some(Ok(Tendril(Shared(buf.read_only())))),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::prelude::v1::*;
    use std::{io, cmp};
    use std::slice::bytes;

    use util::str::is_ascii_whitespace;

    use super::{Tendril, Tendril_, TendrilReader, incomplete_trailing_utf8};

    #[test]
    fn tendril_create() {
        assert_eq!("", &*Tendril::new());

        for s in &["", "foo", "zzzzzzzzzzzzzzzzz", "fooő", "ꙮ"] {
            assert_eq!(*s, &*Tendril::owned(String::from_str(s)));
            assert_eq!(*s, &*Tendril::owned_copy(s));
            assert_eq!(*s, &*Tendril::shared_copy(s));
        }
    }

    #[test]
    fn tendril_from() {
        assert_eq!("x", &*Tendril::from('x'));
        assert_eq!("xyz", &*Tendril::from("xyz"));
        assert_eq!("xyz", &*Tendril::from(String::from_str("xyz")));
        assert_eq!("xyz", &*Tendril::from(Tendril::from("xyz")));
    }

    #[test]
    fn tendril_eq() {
        assert_eq!(Tendril::owned_copy("foo"), Tendril::owned_copy("foo"));
        assert_eq!(Tendril::owned_copy("foo"), Tendril::shared_copy("foo"));
        assert_eq!(Tendril::shared_copy("foo"), Tendril::shared_copy("foo"));
        assert!(Tendril::owned_copy("foo") != Tendril::owned_copy("bar"));
        assert!(Tendril::owned_copy("foo") != Tendril::shared_copy("bar"));
        assert!(Tendril::shared_copy("foo") != Tendril::shared_copy("bar"));
    }

    #[test]
    fn tendril_partial_ord() {
        assert!(Tendril::owned_copy("foo") > Tendril::owned_copy("bar"));
        assert!(Tendril::owned_copy("foo") > Tendril::shared_copy("bar"));
        assert!(Tendril::shared_copy("foo") > Tendril::shared_copy("bar"));
        assert!(Tendril::owned_copy("bar") < Tendril::owned_copy("foo"));
        assert!(Tendril::owned_copy("bar") < Tendril::shared_copy("foo"));
        assert!(Tendril::shared_copy("bar") < Tendril::shared_copy("foo"));
    }

    macro_rules! test_char_run ( ($name:ident, $input:expr, $expect:expr) => (
        test_eq!($name, Tendril::owned_copy($input).char_run(is_ascii_whitespace), $expect);
    ));

    test_char_run!(run_empty, "", None);
    test_char_run!(run_one_t, " ", Some((1, true)));
    test_char_run!(run_one_f, "x", Some((1, false)));
    test_char_run!(run_t, "  \t  \n", Some((6, true)));
    test_char_run!(run_f, "xyzzy", Some((5, false)));
    test_char_run!(run_tf, "   xyzzy", Some((3, true)));
    test_char_run!(run_ft, "xyzzy   ", Some((5, false)));
    test_char_run!(run_tft, "   xyzzy  ", Some((3, true)));
    test_char_run!(run_ftf, "xyzzy   hi", Some((5, false)));
    test_char_run!(run_multibyte_0, "中 ", Some((3, false)));
    test_char_run!(run_multibyte_1, " 中 ", Some((1, true)));
    test_char_run!(run_multibyte_2, "  中 ", Some((2, true)));
    test_char_run!(run_multibyte_3, "   中 ", Some((3, true)));

    #[test]
    fn push() {
        let mut t = Tendril::owned_copy("foo");
        t.push('x');
        assert_eq!("foox", &*t);
        t.push('y');
        assert_eq!("fooxy", &*t);

        let mut t = Tendril::shared_copy("foo");
        t.push('x');
        assert_eq!("foox", &*t);
        t.push('y');
        assert_eq!("fooxy", &*t);
    }

    #[test]
    fn push_str() {
        let mut t = Tendril::owned_copy("foo");
        t.push_str("xy");
        assert_eq!("fooxy", &*t);
        t.push_str("ab");
        assert_eq!("fooxyab", &*t);

        let mut t = Tendril::shared_copy("foo");
        t.push_str("xy");
        assert_eq!("fooxy", &*t);
        t.push_str("ab");
        assert_eq!("fooxyab", &*t);
    }

    #[test]
    fn push_tendril_simple() {
        let mut t = Tendril::owned_copy("foo");
        t.push_tendril(Tendril::owned_copy("xy"));
        assert_eq!("fooxy", &*t);
        t.push_tendril(Tendril::owned_copy("ab"));
        assert_eq!("fooxyab", &*t);

        let mut t = Tendril::owned_copy("foo");
        t.push_tendril(Tendril::shared_copy("xy"));
        assert_eq!("fooxy", &*t);
        t.push_tendril(Tendril::owned_copy("ab"));
        assert_eq!("fooxyab", &*t);

        let mut t = Tendril::shared_copy("foo");
        t.push_tendril(Tendril::owned_copy("xy"));
        assert_eq!("fooxy", &*t);
        t.push_tendril(Tendril::shared_copy("ab"));
        assert_eq!("fooxyab", &*t);

        let mut t = Tendril::shared_copy("foo");
        t.push_tendril(Tendril::shared_copy("xy"));
        assert_eq!("fooxy", &*t);
        t.push_tendril(Tendril::shared_copy("ab"));
        assert_eq!("fooxyab", &*t);
    }

    #[test]
    fn push_tendril_share() {
        let mut x = Tendril::new();
        x.push_tendril(Tendril::shared_copy("foo"));
        assert!(x.is_shared());

        let mut x = Tendril::owned_copy("");
        x.push_tendril(Tendril::shared_copy("foo"));
        assert!(x.is_shared());

        let mut x = Tendril::shared_copy("foo");
        x.push_str("");
        assert!(x.is_shared());

        let mut x = Tendril::shared_copy("foo");
        x.push_tendril(Tendril::owned_copy(""));
        assert!(x.is_shared());

        let mut x = Tendril::shared_copy("foo");
        x.push_tendril(Tendril::shared_copy(""));
        assert!(x.is_shared());
    }

    #[test]
    fn pop_front_lf() {
        let mut t = Tendril::new();
        t.pop_front_lf();
        assert_eq!("", &*t);

        let mut t = Tendril(Tendril_::Ascii(b'\n'));
        t.pop_front_lf();
        assert_eq!("", &*t);

        let mut t = Tendril(Tendril_::Ascii(b'x'));
        t.pop_front_lf();
        assert_eq!("x", &*t);

        let mut t = Tendril::owned_copy("\n");
        t.pop_front_lf();
        assert_eq!("", &*t);

        let mut t = Tendril::owned_copy("\nfoo");
        t.pop_front_lf();
        assert_eq!("foo", &*t);

        let mut t = Tendril::owned_copy("foo");
        t.pop_front_lf();
        assert_eq!("foo", &*t);

        let mut t = Tendril::shared_copy("\n");
        t.pop_front_lf();
        assert_eq!("", &*t);

        let mut t = Tendril::shared_copy("\nfoo");
        t.pop_front_lf();
        assert_eq!("foo", &*t);

        let mut t = Tendril::shared_copy("foo");
        t.pop_front_lf();
        assert_eq!("foo", &*t);
    }

    // FIXME: Test the coalescing of adjacent shared tendrils.

    #[test]
    fn clear() {
        let mut x = Tendril::owned_copy("foo");
        x.clear();
        assert!(x.is_empty());

        let mut x = Tendril::shared_copy("foo");
        x.clear();
        assert!(x.is_empty());
    }

    #[test]
    fn subtendril() {
        let x = Tendril::owned_copy("foo");
        let s = unsafe { x.subtendril(0, 1) };
        assert_eq!("f", &*s);

        let x = Tendril::shared_copy("foo");
        let s = unsafe { x.subtendril(0, 1) };
        assert_eq!("f", &*s);
        assert!(s.is_shared());

        let x = Tendril::shared_copy("\u{a66e}of");
        let s = unsafe { x.subtendril(0, 4) };
        assert_eq!("\u{a66e}o", &*s);
        assert!(s.is_shared());
    }

    // FIXME: Test scenarios where a tendril grows past the size limit.

    #[test]
    fn test_complete_trailing_utf8() {
        fn test(x: &str) {
            assert_eq!(0, incomplete_trailing_utf8(x.as_bytes()));
        }

        test("foobar");
        test("fooő");
        test("foo\u{a66e}");
        test("foo\u{1f4a9}");
    }

    #[test]
    fn test_incomplete_trailing_utf8() {
        assert_eq!(1, incomplete_trailing_utf8(b"foo\xC5"));
        assert_eq!(1, incomplete_trailing_utf8(b"foo\xEA"));
        assert_eq!(2, incomplete_trailing_utf8(b"foo\xEA\x99"));
        assert_eq!(1, incomplete_trailing_utf8(b"foo\xF0"));
        assert_eq!(2, incomplete_trailing_utf8(b"foo\xF0\x9F"));
        assert_eq!(3, incomplete_trailing_utf8(b"foo\xF0\x9F\x92"));
    }

    struct SliceChunks<'a> {
        slice: &'a [u8],
        idx: usize,
        chunk_size: usize,
    }

    impl<'a> io::Read for SliceChunks<'a> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let len = cmp::min(cmp::min(self.chunk_size, buf.len()),
                               self.slice.len() - self.idx);
            if len == 0 { return Ok(0); }
            let src = &self.slice[self.idx..][..len];
            bytes::copy_memory(buf, src);
            self.idx += len;
            Ok(src.len())
        }
    }

    fn test_tendril_reader(input: &str) {
        let mut chunk_sizes = vec![1, 2, 3, 4, 5, 6, 8, 15, 16, 17, 63, 64, 65, 255, 256, 257];
        if input.len() >= 5 {
            chunk_sizes.push(input.len() - 1);
            chunk_sizes.push(input.len());
            chunk_sizes.push(input.len() + 1);
        }

        for &source_chunk_size in &chunk_sizes {
            for &tendril_buf_size in &chunk_sizes {
                if tendril_buf_size < 4 { continue; }

                let reader = SliceChunks {
                    slice: input.as_bytes(),
                    idx: 0,
                    chunk_size: source_chunk_size,
                };

                let mut result = String::new();
                for tendril in TendrilReader::from_utf8(tendril_buf_size as u32, reader) {
                    let tendril = tendril.unwrap();
                    result.push_str(&tendril);
                }

                assert_eq!(input, &*result);
            }
        }
    }

    macro_rules! test_tendril_reader {
        ($( $n:ident => $e:expr, )*) => {$(
            #[test]
            fn $n() {
                test_tendril_reader($e);
            }
        )*}
    }

    test_tendril_reader! {
        reader_smoke_test => "Hello, world!",

        udhr_en => "All human beings are born free and equal in dignity and rights.
                    They are endowed with reason and conscience and should act
                    towards one another in a spirit of brotherhood.",

        udhr_hu => "Minden emberi lény szabadon születik és egyenlő méltósága és
                    joga van. Az emberek, ésszel és lelkiismerettel bírván,
                    egymással szemben testvéri szellemben kell hogy viseltessenek.",

        udhr_th => "เราทุกคนเกิดมาอย่างอิสระ เราทุกคนมีความคิดและความเข้าใจเป็นของเราเอง
                    เราทุกคนควรได้รับการปฏิบัติในทางเดียวกัน.",

        udhr_kr => "모든 인간은 태어날 때부터 자유로우며 그 존엄과 권리에 있어
                    동등하다. 인간은 천부적으로 이성과 양심을 부여받았으며 서로
                    형제애의 정신으로 행동하여야 한다.",

        udhr_jbo => "ro remna cu se jinzi co zifre je simdu'i be le ry. nilselsi'a
                     .e lei ry. selcru .i ry. se menli gi'e se sezmarde .i .ei
                     jeseki'ubo ry. simyzu'e ta'i le tunba",

        udhr_chr => "ᏂᎦᏓ ᎠᏂᏴᏫ ᏂᎨᎫᏓᎸᎾ ᎠᎴ ᎤᏂᏠᏱ ᎤᎾᏕᎿ ᏚᏳᎧᏛ ᎨᏒᎢ. ᎨᏥᏁᎳ ᎤᎾᏓᏅᏖᏗ ᎠᎴ ᎤᏃᏟᏍᏗ
                     ᎠᎴ ᏌᏊ ᎨᏒ ᏧᏂᎸᏫᏍᏓᏁᏗ ᎠᎾᏟᏅᏢ ᎠᏓᏅᏙ ᎬᏗ.",
    }

    // FIXME: test TendrilReader error handling
}
