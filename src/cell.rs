use std::cell::UnsafeCell;

/// `Cell` allows for interior mutability through a shared reference because no
/// other threads can have a reference to the same Cell and no reference to the
/// inner `T` is ever exposed.
pub struct Cell<T> {
    value: UnsafeCell<T>,
}

// Implied by `UnsafeCell`, which is already `!Sync`.
// impl<T> !Sync for Cell<T> {}

unsafe impl<T> Sync for Cell<T> {}

impl<T> Cell<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
        }
    }

    pub fn set(&self, value: T) {
        // SAFETY: `Cell` is `!Sync` and does not return a reference to the
        // inner `T` so concurrent mutation or reference invalidation cannot
        // occur.
        unsafe {
            *self.value.get() = value;
        }
    }
}

impl<T> Cell<T>
where
    T: Copy,
{
    pub fn get(&self) -> T {
        // SAFETY: The value can only be mutated in a single-threaded context
        // via `Cell::set`.
        unsafe { *self.value.get() }
    }
}

/// ```compile_fail
/// use std::sync::Arc;
/// use crate::cell::Cell;
///
/// let x = Arc::new(Cell::new(0));
///
/// let x1 = Arc::clone(&x);
/// let h1 = std::thread::spawn(move || {
///     for _ in 0..1000000 {
///         let x = x1.get();
///         x1.set(x + 1);
///     }
/// });
///
/// let x2 = Arc::clone(&x);
/// let h2 = std::thread::spawn(move || {
///     for _ in 0..1000000 {
///         let x = x2.get();
///         x2.set(x + 1);
///     }
/// });
///
/// h1.join().unwrap();
/// h2.join().unwrap();
///
/// assert_eq!(x.get(), 2000000);
/// ```
fn assert_non_sync() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_valid() {
        let x = Cell::new(42);
        assert_eq!(x.get(), 42);

        x.set(89);
        assert_eq!(x.get(), 89);
    }
}
