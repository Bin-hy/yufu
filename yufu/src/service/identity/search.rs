//! Shared bounded directory search helpers for operator-facing pickers.
use crate::domain::identity::error::IdentityError;

/// Normalizes an optional free-text query. Empty searches stay empty so the
/// caller can return the first page; oversized or control-character input is
/// rejected before it reaches the database.
pub(crate) fn normalize_query(query: Option<&str>) -> Result<Option<String>, IdentityError> {
    let Some(query) = query else {
        return Ok(None);
    };
    let query = query.trim();
    if query.is_empty() {
        return Ok(None);
    }
    if query.chars().count() > 100 || query.chars().any(char::is_control) {
        return Err(IdentityError::InvalidInput);
    }
    Ok(Some(query.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_bounds_queries() {
        assert_eq!(normalize_query(None).unwrap(), None);
        assert_eq!(normalize_query(Some("   ")).unwrap(), None);
        assert_eq!(
            normalize_query(Some(" 张伟 ")).unwrap(),
            Some("张伟".into())
        );
        assert!(normalize_query(Some(&"a".repeat(101))).is_err());
        assert!(normalize_query(Some("a\u{0}b")).is_err());
    }
}
