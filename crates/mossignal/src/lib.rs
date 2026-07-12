//! Mossignal starter library.
//!
//! # Examples
//!
//! ```
//! use mossignal::project_name;
//! assert_eq!(project_name(), "mossignal");
//! ```

/// Returns the project name.
#[must_use]
pub const fn project_name() -> &'static str {
    "mossignal"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_name() {
        assert_eq!(project_name(), "mossignal");
    }
}
