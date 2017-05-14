use std::fmt;
use std::marker::PhantomData;

/// A copy of std::ptr::Shared that can be used on stable compilers
pub struct Shared<T: ?Sized> {
    pointer: *const T,
    _marker: PhantomData<T>,
}

impl<T: ?Sized> fmt::Pointer for Shared<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.as_ptr(), f)
    }
}

impl<T: ?Sized> Clone for Shared<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Shared<T> { }

impl<T: ?Sized> Shared<T> {
    /// Creates a new `Shared`.
    ///
    /// # Safety
    ///
    /// `ptr` must be non-null.
    #[inline]
    pub unsafe fn new(ptr: *mut T) -> Self {
        Shared { pointer: ptr, _marker: PhantomData }
    }

    /// Acquires the underlying `*mut` pointer.
    #[inline]
    pub fn as_ptr(self) -> *mut T {
        self.pointer as *mut T
    }

    /// Dereferences the content.
    ///
    /// The resulting lifetime is bound to self so this behaves "as if"
    /// it were actually an instance of T that is getting borrowed. If a longer
    /// (unbound) lifetime is needed, use `&*my_ptr.ptr()`.
    #[inline]
    pub unsafe fn as_ref(&self) -> &T {
        &*self.as_ptr()
    }

    /// Mutably dereferences the content.
    ///
    /// The resulting lifetime is bound to self so this behaves "as if"
    /// it were actually an instance of T that is getting borrowed. If a longer
    /// (unbound) lifetime is needed, use `&mut *my_ptr.ptr_mut()`.
    #[inline]
    pub unsafe fn as_mut(&mut self) -> &mut T {
        &mut *self.as_ptr()
    }
}
