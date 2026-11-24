//! Zero-storage RAII smart pointer (`DOC-103`).
//!
//! Implements the **RAII Smart Pointer** and **Type-Level Erasure** patterns from
//! `docs/14_COMPLIANCE_AND_NON_STORING.md` §5.1:
//!
//! - Transparent access through [`Deref`] / [`DerefMut`] (`Target = T`).
//! - Memory scrubbing on drop via [`Zeroize`] ([`ZeroizeOnDrop`] marker).
//! - **Never persisted**: [`Serialize`] always emits `null`, and
//!   [`NonStored::always_skip`] lets containing structs omit the field entirely.
//! - **Transparently deserialized** from LLM JSON output so in-memory validation can run.
//! - [`Debug`] output is redacted so values never leak into logs.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::{Deref, DerefMut};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Ephemeral value container that is zeroed on drop and excluded from persistence.
///
/// # Examples
///
/// ```
/// use docula_core::NonStored;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct Payroll {
///     employee_name: String,
///     #[serde(default, skip_serializing_if = "NonStored::always_skip")]
///     verification_token: NonStored<String>,
/// }
///
/// let llm_json = r#"{"employee_name":"Ada","verification_token":"OTP-1234"}"#;
/// let payroll: Payroll = serde_json::from_str(llm_json).unwrap();
/// assert_eq!(payroll.verification_token.as_str(), "OTP-1234"); // in-memory validation
///
/// let archived = serde_json::to_string(&payroll).unwrap();
/// assert_eq!(archived, r#"{"employee_name":"Ada"}"#); // never persisted
/// ```
#[derive(Clone, Default, PartialEq, Eq)]
pub struct NonStored<T: Zeroize>(pub T);

impl<T: Zeroize> NonStored<T> {
    /// Wraps an ephemeral value.
    #[inline]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// `skip_serializing_if` predicate that always omits the field from serialized output.
    ///
    /// Use as `#[serde(default, skip_serializing_if = "NonStored::always_skip")]`.
    #[inline]
    pub fn always_skip(_: &Self) -> bool {
        true
    }

    /// Extracts the inner value, leaving a zeroizable default in its place.
    ///
    /// The caller takes over responsibility for scrubbing the returned value.
    #[inline]
    pub fn into_inner(mut self) -> T
    where
        T: Default,
    {
        std::mem::take(&mut self.0)
    }
}

impl<T: Zeroize> From<T> for NonStored<T> {
    #[inline]
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T: Zeroize> Deref for NonStored<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Zeroize> DerefMut for NonStored<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: Zeroize> Drop for NonStored<T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl<T: Zeroize> ZeroizeOnDrop for NonStored<T> {}

impl<T: Zeroize> fmt::Debug for NonStored<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NonStored(<redacted>)")
    }
}

impl<T: Zeroize> Serialize for NonStored<T> {
    /// Always serializes to `null`; the wrapped value is never emitted.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_none()
    }
}

impl<'de, T: Zeroize + Deserialize<'de>> Deserialize<'de> for NonStored<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_is_redacted() {
        let secret = NonStored::new("hunter2".to_owned());
        let rendered = format!("{secret:?}");
        assert_eq!(rendered, "NonStored(<redacted>)");
        assert!(!rendered.contains("hunter2"));
    }

    #[test]
    fn test_into_inner_returns_value() {
        let secret = NonStored::new("abc".to_owned());
        assert_eq!(secret.into_inner(), "abc");
    }
}
