use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;

use crate::cell::Cell;

/// Single-threaded, reference-counted smart pointer for multiple shared
/// references to a value.
#[derive(Debug)]
pub struct Rc<T> {
    // Needs to be heap-allocated since it can be referenced from multiple
    // regions of code.
    inner: NonNull<RcInner<T>>,
    // Need to indicate to the compiler that we logically own `T`, since there
    // is only a pointer to `T`, which is non-owning. Enforces drop checking,
    // variance, and lifetimes accordingly.
    _marker: PhantomData<RcInner<T>>,
}

// Implied by `NonNull`, which is already `!Send` and `!Sync`.
//
// `Rc` is not `Send` because cloning it still shares the same internal state
// across threads without synchronization, which is not safe to transfer between
// threads.
//
// `Rc` is also not `Sync` because its reference count is updated using
// non-atomic operations, making concurrent access from multiple threads prone
// to data races.
//
// impl<T> !Send for Rc<T> {}
// impl<T> !Sync for Rc<T> {}

// `RcInner` enables the reference count to also be shared between cloned Rc's.
#[derive(Debug)]
struct RcInner<T> {
    value: T,
    ref_count: Cell<usize>,
}

impl<T> Rc<T> {
    pub fn new(value: T) -> Self {
        unsafe {
            Self {
                // SAFETY: `Box::new` either returns a valid non-null pointer
                // or panics on OOM.
                inner: NonNull::new_unchecked(Box::into_raw(Box::new(RcInner {
                    value,
                    // Creating an `Rc` counts as a reference.
                    ref_count: Cell::new(1),
                }))),
                _marker: PhantomData,
            }
        }
    }
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Self {
        // Increment the reference count.
        unsafe {
            let count = (*self.inner.as_ptr()).ref_count.get();
            (*self.inner.as_ptr()).ref_count.set(count + 1);
        }

        // `NonNull` implements Copy since it wraps a raw pointer.
        Self {
            inner: self.inner,
            _marker: PhantomData,
        }
    }
}

impl<T> Deref for Rc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: self.inner is allocated though a Box that is only
        // deallocated when the last `Rc` is dropped, but we currently have
        // an `Rc`.
        unsafe { &(*self.inner.as_ptr()).value }
    }
}

impl<T> Drop for Rc<T> {
    fn drop(&mut self) {
        unsafe {
            match (*self.inner.as_ptr()).ref_count.get() {
                // Drop the last Rc, since it is no longer referenced.
                1 => {
                    // Returned value is immediately dropped, `Box` handles
                    // deallocation.
                    let _ = Box::from_raw(self.inner.as_ptr());
                }
                n => (*self.inner.as_ptr()).ref_count.set(n - 1),
            }
        }
    }
}

/// ```compile_fail
/// use crust_of_rust::rc::Rc;
///
/// fn require_sync<T: Sync>(_: T) {}
///
/// fn main() {
///     require_sync(Rc::new(42));
/// }
/// ```
fn assert_non_sync() {}

/// ```compile_fail
/// use crust_of_rust::rc::Rc;
///
/// fn require_send<T: Send>(_: T) {}
///
/// fn main() {
///     require_send(Rc::new(42));
/// }
/// ```
fn assert_non_send() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;

    struct DropCounter<'a> {
        dropped: &'a Cell<bool>,
    }

    impl<'a> Drop for DropCounter<'a> {
        fn drop(&mut self) {
            self.dropped.set(true);
        }
    }

    #[test]
    // MIRI
    fn test_rc_mem_leak() {
        let rc = Rc::new(Box::new(10));
        assert_eq!(**rc, 10);
    }

    #[test]
    fn test_rc_clone() {
        let rc1 = Rc::new(String::from("hello"));
        let rc2 = rc1.clone();

        assert_eq!(&*rc1, "hello");
        assert_eq!(&*rc2, "hello");
    }

    #[test]
    fn test_rc_drop_deallocate() {
        let dropped = Cell::new(false);

        {
            let rc1 = Rc::new(DropCounter { dropped: &dropped });
            let rc2 = rc1.clone();
            let rc3 = rc2.clone();

            assert_eq!(dropped.get(), false);

            drop(rc3);
            drop(rc2);
            assert_eq!(dropped.get(), false);

            drop(rc1);
            assert_eq!(dropped.get(), true);
        }
    }

    #[test]
    fn test_rc_multiple_clones() {
        let rc = Rc::new(vec![1, 2, 3]);
        let clones: Vec<_> = (0..10).map(|_| rc.clone()).collect();

        for c in clones {
            assert_eq!(c.len(), 3);
            assert_eq!(c[0], 1);
        }

        assert_eq!(rc[2], 3);
    }
}
