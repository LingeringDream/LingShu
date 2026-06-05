//! Helpers for correct PATCH semantics with nullable fields.
//!
//! Standard `Option<T>` in serde cannot distinguish between "field absent from
//! JSON" and "field explicitly set to `null`" — both deserialize as `None`.
//! This module provides a custom deserializer that splits `Option<Option<T>>`
//! into three states:
//!
//! | JSON                | Rust type            | Meaning                       |
//! |---------------------|----------------------|-------------------------------|
//! | field absent        | `None`               | Keep current DB value         |
//! | `"field": null`     | `Some(None)`         | Set DB column to NULL         |
//! | `"field": "value"`  | `Some(Some(value))`  | Update to the given value     |
//!
//! ## Usage
//!
//! ```ignore
//! #[derive(Deserialize)]
//! struct UpdateThingRequest {
//!     pub title: Option<String>, // non-nullable field — unwrap_or is fine
//!
//!     #[serde(default, deserialize_with = "crate::patch::nullable")]
//!     pub assignee_id: Option<Option<Uuid>>, // nullable field — can be cleared
//! }
//! ```
//!
//! Then in the handler:
//!
//! ```ignore
//! let assignee_id = match req.assignee_id {
//!     Some(v) => v,           // explicit null or value
//!     None => current.assignee_id, // omitted — keep current
//! };
//! ```

use serde::de::{Deserialize, Deserializer, Visitor};
use std::fmt;
use std::marker::PhantomData;

/// Custom deserializer for nullable fields in PATCH requests.
///
/// - JSON `null` → `Some(None)`  (clear the column)
/// - JSON value  → `Some(Some(v))` (set to value)
/// - Field absent → `None`       (keep current)
///
/// Attach to a field with:
/// `#[serde(default, deserialize_with = "crate::patch::nullable")]`
pub fn nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct NullableVisitor<T>(PhantomData<T>);

    impl<'de, T: Deserialize<'de>> Visitor<'de> for NullableVisitor<T> {
        type Value = Option<Option<T>>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a value, null, or nothing")
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            // JSON `null` → outer Some, inner None → clear the column
            Ok(Some(None))
        }

        fn visit_some<D2>(self, deserializer: D2) -> Result<Self::Value, D2::Error>
        where
            D2: Deserializer<'de>,
        {
            // JSON value present → outer Some, inner Some(v)
            T::deserialize(deserializer).map(|v| Some(Some(v)))
        }
    }

    deserializer.deserialize_option(NullableVisitor(PhantomData))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestRequest {
        #[serde(default, deserialize_with = "nullable")]
        pub value: Option<Option<String>>,
    }

    #[test]
    fn field_absent_is_none() {
        let req: TestRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(req.value, None);
    }

    #[test]
    fn field_null_is_some_none() {
        let req: TestRequest = serde_json::from_str(r#"{"value": null}"#).unwrap();
        assert_eq!(req.value, Some(None));
    }

    #[test]
    fn field_present_is_some_some() {
        let req: TestRequest = serde_json::from_str(r#"{"value": "hello"}"#).unwrap();
        assert_eq!(req.value, Some(Some("hello".to_string())));
    }
}
