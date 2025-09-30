use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};

use crate::cell::Cell;

/// `RefCell` allows for interior mutability through a shared reference with
/// dynamic borrow-checking and ensures no other threads can have a reference to
/// the same `RefCell` and the inner `T` is not mutably aliased.
pub struct RefCell<T> {
    // Only `safe` way in Rust to perform interior mutability through a shared
    // reference.
    inner: UnsafeCell<T>,
    // Wrapped in `Cell` so updates can occur through a shared reference.
    references: Cell<isize>,
}

// Implied by `UnsafeCell`, which is already `!Sync`.
// impl<T> !Sync for RefCell<T> {}

impl<T> RefCell<T> {
    /// Sentinel value indicating a mutable borrow is live.
    const MUTABLE_BORROW: isize = -1;

    pub fn new(value: T) -> Self {
        Self {
            inner: UnsafeCell::new(value),
            references: Cell::new(0),
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn borrow(&self) -> Ref<'_, T> {
        assert!(
            self.references.get() != Self::MUTABLE_BORROW,
            "RefCell is already borrowed mutably"
        );

        let prev = self.references.get();
        self.references.set(prev + 1);

        // SAFETY: No mutable references to `T` have been given out.
        Ref { parent: self }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        assert!(self.references.get() == 0, "RefCell is already borrowed");
        self.references.set(Self::MUTABLE_BORROW);

        // SAFETY: No other references to `T` have been given out.
        RefMut { parent: self }
    }
}

/// Essentially a smart pointer that transparently points to the inner `T`
/// (`Deref`), and has additional semantics when dropping.
pub struct Ref<'a, T> {
    parent: &'a RefCell<T>,
}

impl<T> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: `Ref` is only created when no mutable references to `T` have
        // been given out.
        unsafe { &*self.parent.inner.get() }
    }
}

impl<T> Drop for Ref<'_, T> {
    fn drop(&mut self) {
        let refs = self.parent.references.get();
        self.parent.references.set(refs - 1);
    }
}

/// Essentially a smart pointer that transparently points to the inner `T`
/// (`Deref` and `DerefMut`), and has additional semantics when dropping.
pub struct RefMut<'a, T> {
    parent: &'a RefCell<T>,
}

impl<T> Deref for RefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: `RefMut` is only created when no other references to `T`
        // have been given out.
        unsafe { &*self.parent.inner.get() }
    }
}

impl<T> DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: `RefMut` is only created when no other references to `T`
        // have been given out.
        unsafe { &mut *self.parent.inner.get() }
    }
}

impl<T> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        // Since `RefMut` should be the only reference to the `RefCell`, after
        // dropping there should be no more references.
        self.parent.references.set(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refcell_borrow() {
        let cell = RefCell::new(42);
        let val = cell.borrow();
        assert_eq!(*val, 42);
    }

    #[test]
    fn test_refcell_mut_borrow() {
        let mut cell = RefCell::new(100);
        {
            let mut val = cell.borrow_mut();
            *val += 23;
        } // mutable borrow ends here
        assert_eq!(*cell.borrow(), 123);
    }

    #[test]
    fn test_refcell_multiple_borrows() {
        let cell = RefCell::new(77);
        let a = cell.borrow();
        let b = cell.borrow();
        assert_eq!(*a, 77);
        assert_eq!(*b, 77);
    }

    #[test]
    #[should_panic]
    fn test_refcell_mutable_alias() {
        let cell = RefCell::new(55);
        let _shared = cell.borrow();
        let _mut_borrow = cell.borrow_mut();
    }

    #[test]
    #[should_panic]
    fn test_refcell_multiple_mut() {
        let mut cell = RefCell::new(99);
        let _first = cell.borrow_mut();
        let _second = cell.borrow_mut();
    }

    #[test]
    #[should_panic]
    fn test_refcell_mut_then_share() {
        let mut cell = RefCell::new(88);
        let _mut_ref = cell.borrow_mut();
        let _shared_ref = cell.borrow();
    }

    #[test]
    fn test_refcell_borrow_drop() {
        let mut cell = RefCell::new(200);

        {
            let _shared1 = cell.borrow();
            let _shared2 = cell.borrow();
        } // both shared borrows dropped

        {
            let mut mut_borrow = cell.borrow_mut();
            *mut_borrow += 1;
        }

        assert_eq!(*cell.borrow(), 201);
    }
}
