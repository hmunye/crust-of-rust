//! Variance describes what types are `subtypes` of other types and when a
//! subtype can be used in place of a `supertype`.
//!
//! A type `T` is a subtype of another type `U` if `T` is at least as useful as
//! `U`. Variance is the reason why a `&'static str` can be used in the place of
//! `&'a str`. `'static` is a subtype of `'a` because a `'static` lives at least
//! as long as any `'a` and so is more useful. If a lifetime `'b: 'a`, then `'b`
//! is a subtype of `'a`.
//!
//! All types have a variance, defining what other similar types can be used in
//! its place. The kinds of variance include:
//!
//! - `Covariant`: A subtype can be used in place of the type (e.g, `&'a T`)
//! - `Invariant`: No subtyping is allowed for the given type (e.g, `&'a mut T`)
//! - `Contravariant`: A less useful type can be used in place of the type
//!
//! ```no_run
//! let x: &'static str; // more useful, lives longer
//! let x: &'a str;      // less useful, lives shorter
//!
//! fn take_func1(&'static str) // stricter, less useful
//! fn take_func2(&'a str)      // relaxed, more useful
//! ```

struct ValidMutStr<'a, 'b> {
    s: &'a mut &'b str,
}

struct InvalidMutStr<'a> {
    s: &'a mut &'a str,
}

pub fn will_compile() {
    let mut x = "hello"; // &'static str

    // Creates a temporary struct with a mutable borrow of `x`.
    // The field `s` has type `&'a mut &'b str`, which is invariant over `'a`.
    // However, since `'a` (the lifetime of the mutable borrow) and `'b`
    // (the lifetime of the inner reference) are distinct, the compiler can
    // infer a short lifetime for `'a` without affecting `'b`.
    //
    // This means the mutable borrow of `x` only needs to live for this
    // statement, so it doesn't "leak" into the surrounding region of code
    // defined by the lifetime of `x`.
    *ValidMutStr { s: &mut x }.s = "world";

    // At this point, the compiler attempts to shorten the lifetime of the
    // mutable borrow of `s` so that it ends before `x` is used again.
    // Because `'a` and `'b` are define distinct regions of code, it succeeds.
    //
    // `x` can be used here.
    println!("{x}")
}

pub fn wont_compile() {
    let mut x = "hello"; // &'static str

    // Creates a temporary struct with a mutable borrow of `x`.
    // In this case, both the mutable borrow and the inner reference share
    // the same lifetime `'a`, so the type is `&'a mut &'a str`.
    // Due to invariance, the compiler must treat both borrows as having the
    // same region, they cannot be shortened independently.
    //
    // Since `x` is a `&'static str`, the compiler unifies `'a` with `'static`,
    // and assumes the mutable borrow lasts for the entire `'static` region.
    //
    // Even though the temporary struct is dropped immediately after this line,
    // the compiler still considers `x` to be mutably borrowed. From the source
    // code, it *looks like* the borrow should be over, but due to lifetime
    // unification and invariance, the mutable borrow of `s` "leaks" into the
    // surrounding region of code, in this case `'static`.
    *InvalidMutStr { s: &mut x }.s = "world";

    // At this point, the compiler attempts to shorten the lifetime of the
    // mutable borrow of `s` so that it ends before `x` is used again, but fails
    // because both references share the same lifetime `'a`, so it would need to
    // shorten the inner reference too, which it can't do.
    //
    // Error: cannot borrow `x` as immutable because it is also borrowed as
    // mutable
    println!("{x}")
}
