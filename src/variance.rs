//! Variance describes what types are `subtypes` of other types and when a
//! subtype can be used in place of a `supertype`.
//!
//! Informally, a type `T` is a subtype of another type `U` if `T` is at least
//! as useful as `U`. Variance is the reason why a `&'static str` can be used in
//! the place of `&'a str`. `'static` is a subtype of `'a` because a `'static`
//! lives at least as long as any `'a` and so is more useful. If a lifetime
//! `'b: 'a`, then `'b` is a subtype of `'a`.
//!
//! All types have a variance, defining what other similar types can be used in
//! its place. The kinds of variance include:
//!
//! - `Covariant`: A subtype can be used in place of the type
//!     - `&'a T`: Covariant in `'a` and `T`
//!
//! - `Invariant`: No subtyping is allowed for the given type
//!     - `&'a mut T`: Covariant in `'a`, invariant in `T`
//!
//! - `Contravariant`: A less useful type can be used in place of the type
//!     - `fn(T) -> U`: Contravariant in `T`, covariant in `U`
//! ```

pub fn will_compile() {
    struct ValidMutStr<'a, 'b> {
        s: &'a mut &'b str,
    }

    let mut x = "hello"; // &'static str

    // Creates a temporary struct with a mutable borrow of `x`. The field `s`
    // has type `&'a mut &'b str`, which is covariant in `'a` and invariant in
    // `'b str`. Since `'a` (the lifetime of the mutable borrow) and `'b`
    // (the lifetime of the inner reference) are distinct, the compiler can
    // shorten the lifetime of `'a` without affecting `'b`.
    //
    // This means the mutable borrow of `x` only needs to live for this
    // statement, so it doesn't "leak" into the surrounding region of code
    // defined by the lifetime of `x`. Leak here is used loosely to indicate the
    // mutable borrow does not last longer to the compiler than how it appears
    // in the source code.
    *ValidMutStr { s: &mut x }.s = "world";

    // At this point, the compiler attempts to shorten the lifetime of the
    // mutable borrow of `s` so that it ends before `x` is used again.
    // Because `'a` and `'b` define distinct regions of code, it can be
    // shortened.
    println!("{x}")
}

pub fn wont_compile() {
    struct InvalidMutStr<'a> {
        s: &'a mut &'a str,
    }

    let mut x = "hello"; // &'static str

    // Creates a temporary struct with a mutable borrow of `x`. In this case,
    // both the mutable borrow and the inner reference share the same lifetime
    // `'a`, so the type is `&'a mut &'a str`. Due to invariance, the compiler
    // must treat both borrows as defining the same region, they cannot be
    // shortened independently.
    //
    // Since `x` is a `&'static str`, the compiler unifies `'a` with `'static`,
    // and assumes the mutable borrow lasts for the entire `'static` region of
    // `x`.
    //
    // Even though the temporary struct is dropped immediately after this line,
    // the compiler still considers `x` to be mutably borrowed. From the source
    // code, it *looks like* the borrow should be over, but due to lifetime
    // unification and invariance, the mutable borrow of `s` "leaks" into the
    // surrounding region of code, in this case `'static`. Leak here is used
    // loosely to indicate the mutable borrow lasts longer to the compiler than
    // how it appears in the source code.

    /*
     * *InvalidMutStr { s: &mut x }.s = "world";
     *
     */

    // At this point, the compiler attempts to shorten the lifetime of the
    // mutable borrow of `s` so that it ends before `x` is used again, but fails
    // because both references share the same lifetime `'a`, so it would need to
    // shorten the inner reference too, which it can't do because of invariance.
    //
    // Error: cannot borrow `x` as immutable because it is also borrowed as
    // mutable
    println!("{x}")
}

// Two distinct generic lifetimes are needed here so the mutable borrow's
// lifetime is not tied to the lifetime of `s`. With this approach, the mutable
// borrow can be shortened instead of lasting as long as `s`, which is invariant
// in `'a str`.
//
// The mutable reference's lifetime is elided, and the compiler will
// automatically give it a distinct lifetime when analyzing.
pub fn strtok<'a>(s: &'_ mut &'a str, delim: char) -> &'a str {
    if let Some(i) = s.find(delim) {
        let prefix = &s[..i];
        let suffix = &s[i + delim.len_utf8()..];
        *s = suffix;
        prefix
    } else {
        let prefix = *s;
        *s = "";
        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::strtok;

    #[test]
    fn test_strtok_valid() {
        let mut x = "hello world";
        let hello = strtok(&mut x, ' ');
        assert_eq!(hello, "hello");
        assert_eq!(x, "world");
    }
}
