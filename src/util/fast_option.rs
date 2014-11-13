//! Options of big things (like `Option<SingleChar>`) make rustc generate really
//! bad code with lots of moves. When rustc gets non-lexical borrows and NRVO,
//! this module will be unnecessary. In the mean time, this is an unsafe option
//! representation.
use core::mem;
use core::ops::Drop;
use core::ptr;
use core::intrinsics::move_val_init;

pub use self::OptValue::{Uninit, Full};

#[deriving(PartialEq, Eq, Show)]
pub enum OptValue {
    Uninit,
    Full,
}

/// An optional value, but without the option header. Functions using `FastOption`s
/// return `bool`, which is `true` if the option is valid, and `false` if it's not.
///
/// Every `FastOption` must be filled exactly once, with either a `None` or a `Some`,
/// and `take`n only once.
#[unsafe_no_drop_flag]
pub struct FastOption<T> {
    is_valid: bool,
    t: T,
}

#[inline(never)]
fn bad_usage() -> ! {
    panic!("Invalid use of `FastOption`.");
}

impl<T> FastOption<T> {
    #[inline(always)]
    pub fn new() -> FastOption<T> {
        unsafe {
            FastOption {
                is_valid: false,
                t:        mem::uninitialized(),
            }
        }
    }

    #[inline(always)]
    pub fn is_filled(&self) -> bool {
        self.is_valid
    }

    #[inline(always)]
    fn set_valid(&mut self, new_val: bool) {
        self.is_valid = new_val;
    }

    #[inline(always)]
    fn check_valid(&self) {
        if !self.is_valid { bad_usage() }
    }

    // This does a memcpy an da drop. Do it out of line.
    #[inline(never)]
    unsafe fn replace(&mut self, t: T) {
        debug_assert!(self.is_valid);
        self.t = t;
    }

    /// Only use this the first time a `FastOption` is filled. Otherwise, use
    /// `replace`.
    #[inline(always)]
    pub fn fill(&mut self, t: T) -> OptValue {
        unsafe {
            if self.is_valid {
                self.replace(t);
            } else {
                move_val_init(&mut self.t, t);
                self.set_valid(true);
            }
        }
        Full
    }

    #[inline(always)]
    pub fn as_ref(&self) -> &T {
        self.check_valid();
        &self.t
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> &mut T {
        self.check_valid();
        &mut self.t
    }

    #[inline(always)]
    pub fn take(&mut self) -> T {
        unsafe {
            self.check_valid();
            self.set_valid(false);
            ptr::read(&self.t)
        }
    }
}

#[unsafe_destructor]
impl<T> Drop for FastOption<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            if !self.is_valid {
                move_val_init(&mut self.t, mem::zeroed());
            }
        }
    }
}

#[test]
fn proper_usage() {
    use core::iter;

    let mut opt = FastOption::new();
    assert_eq!(opt.fill(vec!(42u)), Full);
    assert_eq!(*opt.as_ref(), vec!(42));
    assert_eq!(opt.take(), vec!(42));

    let mut opt = FastOption::new();

    // This will hopefully trash jemalloc if we mishandle memory.
    for i in iter::range(0u, 100000) {
        assert_eq!(opt.fill(vec!(i)), Full);
        assert_eq!(opt.as_ref(), &vec!(i));
        assert_eq!(opt.as_ref(), &vec!(i));
        if i % 2 == 0 {
            assert_eq!(opt.take(), vec!(i));
        } else {
        }
    }
}
