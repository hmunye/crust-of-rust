use std::cell::UnsafeCell;

/// `Cell` allows for interior mutability through a shared reference because no
/// other threads can have a reference to the same Cell and no reference to the
/// inner `T` is ever exposed.
#[derive(Debug)]
pub struct Cell<T> {
    value: UnsafeCell<T>,
}

// Implied by `UnsafeCell`, which is already `!Sync`.
// impl<T> !Sync for Cell<T> {}

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
/// use crust_of_rust::cell::Cell;
///
/// fn require_sync<T: Sync>(_: T) {}
///
/// fn main() {
///     require_sync(&Cell::new(42));
/// }
/// ```
fn assert_not_sync() {}

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

    #[test]
    fn test_cell_multiple_sets() {
        let x = Cell::new(0);
        for i in 1..10 {
            x.set(i);
            assert_eq!(x.get(), i);
        }
    }

    #[test]
    fn test_cell_copy_type() {
        #[derive(Clone, Copy, Debug, PartialEq)]
        struct Small(u8);

        let c = Cell::new(Small(7));
        assert_eq!(c.get(), Small(7));

        c.set(Small(99));
        assert_eq!(c.get(), Small(99));
    }

    #[test]
    fn test_cell_independent_instances() {
        let a = Cell::new(1);
        let b = Cell::new(2);

        assert_eq!(a.get(), 1);
        assert_eq!(b.get(), 2);

        a.set(10);
        b.set(20);

        assert_eq!(a.get(), 10);
        assert_eq!(b.get(), 20);
    }

    #[test]
    fn test_cell_replaces_previous_value() {
        let c = Cell::new(5);
        assert_eq!(c.get(), 5);

        c.set(999);
        assert_eq!(c.get(), 999);

        c.set(-3);
        assert_eq!(c.get(), -3);
    }
}
