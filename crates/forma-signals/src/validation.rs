//! Root-path stamping seam and per-field issue grouping.
//!
//! v0 primitives report issues at [`FieldPath::ROOT`]; this module owns the
//! translation to per-field addressing so v1 object nesting inherits correct
//! behavior from one isolated boundary.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use reactive_graph::computed::ArcMemo;
use reactive_graph::signal::ArcRwSignal;
use reactive_graph::traits::Get;

use forma_core::error::{FieldPath, FormaIssue};

use crate::field::ValidateOn;

/// Insertion-order-preserving registry key wrapping a [`FieldPath`].
///
/// `Hash`, `Eq`, and ordering are all forwarded to the display string so the
/// controller's `IndexMap` keeps declaration order while lookups stay exact.
#[derive(Clone, Debug)]
pub(crate) struct OrderedPath(FieldPath);

impl OrderedPath {
    pub(crate) fn new(path: FieldPath) -> Self {
        Self(path)
    }
}

impl PartialEq for OrderedPath {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_string() == other.0.to_string()
    }
}

impl Eq for OrderedPath {}

impl Ord for OrderedPath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.to_string().cmp(&other.0.to_string())
    }
}

impl PartialOrd for OrderedPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for OrderedPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_string().hash(state);
    }
}

pub(crate) fn stamp_issues(raw: Vec<FormaIssue>, owner: &FieldPath) -> Vec<FormaIssue> {
    raw.into_iter()
        .map(|mut issue| {
            if issue.path == FieldPath::ROOT {
                issue.path = owner.clone();
            }
            issue
        })
        .collect()
}

/// Builds the mode-dependent display-gate memo for one field.
///
/// `Change | Blur` reveal issues once the field is touched or the form has
/// seen a submit attempt; `Submit` reveals on a submit attempt alone.
pub(crate) fn display_gate(
    touched: &ArcRwSignal<bool>,
    submitted: &ArcRwSignal<bool>,
    mode: ValidateOn,
) -> ArcMemo<bool> {
    match mode {
        ValidateOn::Submit => {
            let submitted = submitted.clone();
            ArcMemo::new(move |_| submitted.get())
        }
        ValidateOn::Change | ValidateOn::Blur => {
            let touched = touched.clone();
            let submitted = submitted.clone();
            ArcMemo::new(move |_| touched.get() || submitted.get())
        }
    }
}

pub(crate) fn group_issues(
    stamped: Vec<FormaIssue>,
    known: &[FieldPath],
) -> (BTreeMap<OrderedPath, Vec<FormaIssue>>, Vec<FormaIssue>) {
    let mut per_field: BTreeMap<OrderedPath, Vec<FormaIssue>> = BTreeMap::new();
    let mut unmatched = Vec::new();
    for issue in stamped {
        match known.iter().find(|p| *p == &issue.path) {
            Some(p) => {
                per_field
                    .entry(OrderedPath::new(p.clone()))
                    .or_default()
                    .push(issue);
            }
            None => unmatched.push(issue),
        }
    }
    (per_field, unmatched)
}

#[cfg(test)]
mod tests {
    use super::{OrderedPath, group_issues, stamp_issues};
    use forma_core::error::{FieldPath, FormaIssue, IssueCode};

    fn root_issue(code: IssueCode, msg: &'static str) -> FormaIssue {
        FormaIssue {
            path: FieldPath::ROOT,
            code,
            message: msg.into(),
            params: Vec::new(),
        }
    }

    #[test]
    fn fsp2_stamp_root_issue_gains_owner_path() {
        let owner = FieldPath::key("email");
        let out = stamp_issues(vec![root_issue(IssueCode::Email, "bad email")], &owner);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, owner);
    }

    #[test]
    fn fsp2_non_root_paths_pass_through_unchanged() {
        let owner = FieldPath::key("email");
        let addressed = FormaIssue {
            path: FieldPath::key("other"),
            code: IssueCode::Min,
            message: "min".into(),
            params: Vec::new(),
        };
        let out = stamp_issues(
            vec![root_issue(IssueCode::Email, "bad email"), addressed],
            &owner,
        );
        assert_eq!(out[0].path, owner);
        assert_eq!(out[1].path, FieldPath::key("other"));
    }

    #[test]
    fn fsp2_stamp_preserves_order_and_content() {
        let owner = FieldPath::key("email");
        let raw = vec![
            root_issue(IssueCode::Min, "min"),
            root_issue(IssueCode::Email, "email"),
        ];
        let out = stamp_issues(raw.clone(), &owner);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].code, IssueCode::Min);
        assert_eq!(out[1].code, IssueCode::Email);
        assert_eq!(out[0].message, raw[0].message);
        assert_eq!(out[1].message, raw[1].message);
    }

    #[test]
    fn fsp2_group_partitions_each_field_exactly() {
        let a = FieldPath::key("a");
        let b = FieldPath::key("b");
        let stamped = vec![
            {
                let mut i = root_issue(IssueCode::Min, "a-min");
                i.path = a.clone();
                i
            },
            {
                let mut i = root_issue(IssueCode::Email, "a-email");
                i.path = a.clone();
                i
            },
            {
                let mut i = root_issue(IssueCode::Max, "b-max");
                i.path = b.clone();
                i
            },
        ];
        let (per_field, unmatched) = group_issues(stamped, &[a.clone(), b.clone()]);
        assert_eq!(unmatched.len(), 0);
        assert_eq!(
            per_field.get(&OrderedPath::new(a.clone())).unwrap().len(),
            2
        );
        assert_eq!(per_field.get(&OrderedPath::new(b)).unwrap().len(), 1);
    }

    #[test]
    fn fsp2_group_unmatched_collected_separately() {
        let ghost = FormaIssue {
            path: FieldPath::key("ghost"),
            code: IssueCode::Refine,
            message: "ghost".into(),
            params: Vec::new(),
        };
        let (per_field, unmatched) = group_issues(vec![ghost], &[FieldPath::key("known")]);
        assert!(per_field.is_empty());
        assert_eq!(unmatched.len(), 1);
        assert_eq!(unmatched[0].path, FieldPath::key("ghost"));
    }
}
