//! `dropck` (drop checker) ensures that for a generic type to soundly implement
//! `Drop`, its generics arguments must strictly outlive it.
//!
//! The compiler needs to know when a value is dropped, whether it should
//! consider it a use of any values contained within the type,
//!
//! Currently, the compiler conservatively forces all borrowed data in a value
//! to strictly outlive that value.

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

// The compiler assumes conservatively that dropping a `Foo<T>` will use a `T`
// if `Foo<T>` implements `Drop`, but using `#[may_dangle]` allows us to assert
// `T` is not used when dropping `Foo<T>`.
//
// But that doesn't say anything about dropping a `T` though, so when invoking
// the destructor, the compiler sees that `Foo` does not hold any `T`, only
// a `*mut T` (which it cannot assume anything about).
pub struct Foo<T> {
    // ptr: *mut T,
    //
    // `NonNull` used instead because it is covariant in `T` while providing
    // mutability, and supports niche-optimization.
    inner: NonNull<T>,
    // Using a `PhantomData<T>` tells the compiler we logically own `T` and will
    // drop it.
    _marker: PhantomData<T>,
}

impl<T> Foo<T> {
    pub fn new(val: T) -> Self {
        Self {
            // SAFETY: `Box::new` either returns a valid non-null pointer
            // or panics on OOM.
            inner: unsafe { NonNull::new_unchecked(Box::into_raw(Box::new(val))) },
            _marker: PhantomData,
        }
    }
}

impl<T> Deref for Foo<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: `ptr` is initialized through `Box`, which always returns a
        // non-null and aligned pointer to the allocation.
        unsafe { self.inner.as_ref() }
    }
}

impl<T> DerefMut for Foo<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: `ptr` is initialized through `Box`, which always returns a
        // non-null and aligned pointer to the allocation.
        //
        // `self` is accepted through a mutable reference so that ensures no
        // other references have been given out to `ptr`.
        unsafe { self.inner.as_mut() }
    }
}

// The unstable attribute `#[may_dangle]` can be used to assert (unsafely) that
// a generic type's `Drop` impl is guaranteed to not access any expired data,
// even if it is able to do so.
//
// Marked unsafe because the compiler is not checking the implicit assertion
// that no potentially expired data is accessed. The attribute can be applied to
// any number of lifetime and type parameters, here just being applied to `T`.
//
// https://doc.rust-lang.org/nomicon/dropck.html
unsafe impl<#[may_dangle] T> Drop for Foo<T> {
    fn drop(&mut self) {
        // SAFETY: `ptr` is initialized through `Box`, which always returns a
        // non-null and aligned pointer to the allocation.
        let _ = unsafe { Box::from_raw(self.inner.as_ptr()) };
    }
}

// impl<T> Drop for Foo<T> {
//     fn drop(&mut self) {
//         // SAFETY: `ptr` is initialized through `Box`, which always returns a
//         // non-null and aligned pointer to the allocation.
//         let _ = unsafe { Box::from_raw(self.ptr) };
//     }
// }

/// ```compile_fail
/// use std::fmt::Debug;
/// use crust_of_rust::dropck::Foo;
///
/// struct Touch<T: Debug>(T);
///
/// impl<T: Debug> Drop for Touch<T> {
///     fn drop(&mut self) {
///         // Accessing the inner `T` when dropping.
///         println!("{:?}", self.0);
///     }
/// }
///
/// let mut z = 42;
/// // let b = Foo::new(Touch(&mut z));
/// let b = Box::new(Touch(&mut z));
/// println!("{}", z);
/// ```
#[allow(dead_code)]
fn dropck_valid() {}

/// ```
/// use crust_of_rust::dropck::Foo;
///
/// fn foo_covariant<'a, T>(x: Foo<&'static T>) -> Foo<&'a T> {
///     x
/// }
/// ```
#[allow(dead_code)]
fn assert_properties() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foo_valid() {
        let x = 43;
        let b = Foo::new(x);
        println!("{}", *b);
    }

    #[test]
    fn test_foo_may_dangle() {
        let mut y = 42;
        let b = Foo::new(&mut y);
        println!("{}", y);
    }
}
