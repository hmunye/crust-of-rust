//! Macros (declarative) provide a way to define code patterns and substitutions
//! using pattern matching over input syntax (Rust syntax trees).
//!
//! The input must be valid Rust syntax but isn’t constrained further. The
//! compiler decides if the resulting substitution forms valid Rust code.
//!
//! Identifiers defined within macros are `hygienic`, meaning they don't
//! interfere with the surrounding scope at the invocation site, and the
//! surrounding scope doesn't interfere with them.

// Makes the macro available from the root of the crate.
#[macro_export]
macro_rules! vector {
    // Matches any pattern with no input tokens.
    () => {
        ::std::vec::Vec::new()
    };
    // Matches one or more expressions separated by commas, followed by zero or
    // one trailing comma.
    ($($elem:expr),+ $(,)?) => {
        // The inner block allows multiple expressions in the macro body. The
        // outer delimiter (braces) are required by `macro_rules!` syntax.
        {
            // Determine the number of matched elements at compile time.
            const COUNT: usize = $crate::count!(@COUNT, $($elem)+);

            let mut vec = ::std::vec::Vec::with_capacity(COUNT);
            // Executes the inner statement once for each matched element,
            // repeating zero or more times.
            $(vec.push($elem);)+
            vec
        }
    };
    ($elem:expr; $count:expr) => {{
        // Evaluate the expressions once to support complex types
        // (e.g., Option<T>) and avoid repeated evaluation.
        let count = $count;

        let mut vec = ::std::vec::Vec::with_capacity(count);
        vec.extend(::std::iter::repeat($elem).take(count));
        // Another valid option.
        // vec.resize(count, $elem);
        vec
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! count {
    // Pattern counts the number of matched `$elem` expressions while avoiding
    // recursion that could lead to a stack overflow by constructing a flat
    // array literal. Each `$elem` is replaced with the zero-sized type `()`.
    // The count is obtained by taking the length of this array using `len`,
    // which the compiler can evaluate at compile-time.
    (@COUNT, $($elem:expr)+) => {
        <[()]>::len(&[$($crate::count!(@SUB, $elem ())),+])
    };
    // Replaces an element with the provided substitute expression.
    (@SUB, $_elem:tt $sub:expr) => { $sub };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_vec_macro_empty() {
        let vec: Vec<i32> = vector![];
        assert!(vec.is_empty());
    }

    #[test]
    fn test_vec_macro_single() {
        let vec: Vec<i32> = vector![2];
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0], 2);
    }

    #[test]
    fn test_vec_macro_multi() {
        let vec: Vec<i32> = vector![2, 3, 4, 5];
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 4);
        assert_eq!(vec[0], 2);
        assert_eq!(vec[3], 5);
    }

    #[test]
    fn test_vec_macro_trailing() {
        let vec: Vec<i32> = vector![2, 3, 4, 5,];
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 4);
        assert_eq!(vec[0], 2);
        assert_eq!(vec[3], 5);
    }

    #[test]
    fn test_vec_macro_clone() {
        let vec: Vec<i32> = vector![2; 10];
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 10);
        assert_eq!(vec[0], 2);
        assert_eq!(vec[9], 2);
    }
}
