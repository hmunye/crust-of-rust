//! Lifetimes define regions of reference validity, spans of code where
//! references must remain valid, as determined by the borrow checker through
//! static analysis. This prevents dangling references and use-after-frees.

/// Need two distinct generic lifetimes so split values can have their lifetime
/// tied to `remainder` rather than both fields.
///
/// `Option` used to denote the remainder no longer has any contents to return.
/// Otherwise, the final empty string would not be yielded if the `delimiter` is
/// found as the last part of `remainder`.
#[derive(Debug)]
pub struct StrSplit<'a, 'b> {
    remainder: Option<&'a str>,
    delimiter: &'b str,
}

impl<'a, 'b> StrSplit<'a, 'b> {
    pub fn new(haystack: &'a str, delimiter: &'b str) -> Self {
        Self {
            remainder: Some(haystack),
            delimiter,
        }
    }
}

impl<'a, 'b> Iterator for StrSplit<'a, 'b> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        // Will terminate the iterator once the empty value is yielded.
        let remainder = self.remainder?;

        if let Some(idx) = remainder.find(self.delimiter) {
            self.remainder = Some(&remainder[(idx + self.delimiter.len())..]);
            Some(&remainder[..idx])
        } else {
            // Returns the contents of remainder, replacing it with None so the
            // next call ends the iterator.
            self.remainder.take()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_valid() {
        let haystack = "a b c d e";
        let letters: Vec<_> = StrSplit::new(haystack, " ").collect();

        assert_eq!(letters, vec!["a", "b", "c", "d", "e"])
    }

    #[test]
    fn test_split_tail() {
        let haystack = "a b c d ";
        let letters: Vec<_> = StrSplit::new(haystack, " ").collect();

        assert_eq!(letters, vec!["a", "b", "c", "d", ""])
    }
}
