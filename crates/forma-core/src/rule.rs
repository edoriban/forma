//! Custom validation rules that compose identically to built-in checks.
//!
//! Rules observe post-normalization values, return rejection data, and are
//! stored boxed inside schemas. Sync-only by signature (no async types), and
//! every `Rule` is `Send + Sync`: rules may be validated from any thread and
//! shared across threads via `Arc` without additional bounds.
//!
//! ```
//! use forma_core::error::IssueCode;
//! use forma_core::rule::{RefineRejection, Rule};
//! use std::borrow::Cow;
//!
//! #[derive(Debug)]
//! struct StartsWith(char);
//!
//! impl Rule<str> for StartsWith {
//!     fn name(&self) -> &str { "starts_with" }
//!     fn validate(&self, value: &str) -> Option<RefineRejection> {
//!         if value.starts_with(self.0) { None } else {
//!             Some(RefineRejection {
//!                 code: Some(IssueCode::Refine),
//!                 message: Cow::Borrowed("must start with the expected char"),
//!                 params: Vec::new(),
//!             })
//!         }
//!     }
//! }
//!
//! use forma_core::schema::Schema;
//! use forma_core::types::string;
//! let schema = string().min(2).rule(StartsWith('a'));
//! assert!(schema.parse(&"apple".to_string()).is_ok());
//! assert!(schema.parse(&"pear".to_string()).is_err());
//! ```

use std::borrow::Cow;
use std::fmt;

use crate::error::{IssueCode, IssueParams};

/// Rejection data returned by a rule; the owning kernel handles paths,
/// accumulation and fail-fast policy.
#[derive(Clone, Debug, PartialEq)]
pub struct RefineRejection {
    /// `None` maps to [`IssueCode::Refine`]; custom codes allowed.
    pub code: Option<IssueCode>,
    /// Human-readable rejection message.
    pub message: Cow<'static, str>,
    /// Ordered structured parameters.
    pub params: IssueParams,
}

/// Object-safe extension point stored boxed (`Box<dyn Rule<T>>`) inside schemas (SC-7).
///
/// Rules observe post-normalization values and run strictly after builtin
/// checks. Sync-only by signature (RF-3): no future/async types exist here.
///
/// The `Send + Sync` supertraits guarantee that every erased view
/// (`Box<dyn Rule<T>>`, `&dyn Rule<T>`, `Arc<dyn Rule<T>>`) is usable from
/// multiple threads. This is a breaking contract for hypothetical custom
/// impls holding `!Send`/`!Sync` state; pre-1.0 this may change in a minor
/// release.
pub trait Rule<T: ?Sized>: fmt::Debug + Send + Sync {
    /// Stable identifier surfaced in `shape()` and issue params. Borrowed
    /// from the rule itself so generated names need no `'static` interning.
    fn name(&self) -> &str;

    /// Returns rejection data on failure; `None` on pass.
    fn validate(&self, value: &T) -> Option<RefineRejection>;
}

/// Adapter wrapping a sync closure as a [`Rule`] (used by `.refine(...)`).
pub struct ClosureRule<T: ?Sized> {
    f: Box<dyn Fn(&T) -> bool + Send + Sync>,
    name: Cow<'static, str>,
}

/// Debug output shows only the stable name (closures are not printable).
impl<T: ?Sized> fmt::Debug for ClosureRule<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClosureRule")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl<T: ?Sized> ClosureRule<T> {
    /// Wraps a closure under the default name `"refine-0"`.
    #[must_use]
    pub fn new(f: impl Fn(&T) -> bool + Send + Sync + 'static) -> Self {
        Self {
            f: Box::new(f),
            name: Cow::Borrowed("refine-0"),
        }
    }

    /// Ordinal-based name (`"refine-3"`), stable within a schema instance.
    /// The formatted name is owned by the rule; nothing is leaked.
    #[must_use]
    pub fn with_ordinal(ordinal: usize, f: impl Fn(&T) -> bool + Send + Sync + 'static) -> Self {
        Self {
            f: Box::new(f),
            name: Cow::Owned(format!("refine-{ordinal}")),
        }
    }
}

impl<T: ?Sized> Rule<T> for ClosureRule<T> {
    fn name(&self) -> &str {
        &self.name
    }

    fn validate(&self, value: &T) -> Option<RefineRejection> {
        if (self.f)(value) {
            None
        } else {
            Some(RefineRejection {
                code: Some(IssueCode::Refine),
                message: Cow::Borrowed("failed refinement check"),
                params: Vec::new(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::error::IssueCode;
    use crate::rule::{ClosureRule, RefineRejection, Rule};
    use std::borrow::Cow;

    #[test]
    fn rf3_closure_rule_signature_is_sync() {
        fn assert_sync<T: Sync>() {}
        fn assert_send<T: Send>() {}
        assert_sync::<ClosureRule<fn(&str) -> bool>>();
        assert_send::<ClosureRule<fn(&str) -> bool>>();
    }

    #[test]
    fn closure_rule_pass_yields_none() {
        let r = ClosureRule::new(|_s: &str| true);
        assert!(r.validate("abc").is_none());
    }

    #[test]
    fn closure_rule_fail_yields_default_rejection() {
        let r = ClosureRule::new(|_s: &str| false);
        let rej: RefineRejection = r.validate("abc").unwrap();
        assert_eq!(rej.code, Some(IssueCode::Refine));
        assert_eq!(rej.message, Cow::Borrowed("failed refinement check"));
    }

    #[test]
    fn closure_rule_name_is_refine_0_by_default() {
        let r = ClosureRule::new(|_s: &str| false);
        assert_eq!(r.name(), "refine-0");
    }

    #[test]
    fn closure_rule_with_ordinal_names_stably() {
        let r = ClosureRule::with_ordinal(2, |_s: &str| false);
        assert_eq!(r.name(), "refine-2");
    }

    #[test]
    fn closure_rule_ordinal_name_owned_not_leaked() {
        // Arbitrary ordinals must render identically to the pre-Cow interned
        // form; the name now borrows from the rule instance itself.
        let r = ClosureRule::with_ordinal(4096, |_s: &str| true);
        assert_eq!(r.name(), "refine-4096");
    }

    #[test]
    fn custom_rule_implementor_participates() {
        #[derive(Debug)]
        struct StartsWith(char);
        impl Rule<str> for StartsWith {
            fn name(&self) -> &'static str {
                "starts_with"
            }
            fn validate(&self, value: &str) -> Option<RefineRejection> {
                if value.starts_with(self.0) {
                    None
                } else {
                    Some(RefineRejection {
                        code: None,
                        message: format!("must start with '{}'", self.0).into(),
                        params: Vec::new(),
                    })
                }
            }
        }
        let r = StartsWith('a');
        assert!(r.validate("apple").is_none());
        let rej = r.validate("pear").unwrap();
        assert_eq!(rej.code, None, "None means IssueCode::Refine at the kernel");
        assert_eq!(r.name(), "starts_with");
    }
}
