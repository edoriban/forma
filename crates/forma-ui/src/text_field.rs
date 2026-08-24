//! `<TextField>` and its pure id/pushback seams.

use crate::use_form::try_form_controller;
use forma_signals::FieldHandle;
use leptos::prelude::*;

/// Derives the deterministic input id from a field path string: every
/// non-alphanumeric run collapses to a single `'-'`, prefixed with `forma-`.
/// Alphanumeric Unicode passes through untouched.
pub(crate) fn sanitize_id(path: &str) -> String {
    let mut core = String::with_capacity(path.len());
    let mut prev_sep = false;
    for ch in path.chars() {
        if ch.is_alphanumeric() {
            core.push(ch);
            prev_sep = false;
        } else if !prev_sep {
            core.push('-');
            prev_sep = true;
        }
    }
    format!("forma-{}", core.trim_matches('-'))
}

/// Conditional-pushback predicate: the DOM value is rewritten only when it
/// differs from the signal-derived string. Identical values must NOT rewrite,
/// preserving caret position (FU-TF-2).
pub(crate) fn should_push(dom: &str, sig: &str) -> bool {
    dom != sig
}

/// One bound string field: bidirectional value bridge over the handle's
/// `get_str`/`set_str` convenience layer, touch-on-blur via the resolved
/// controller, gated error rendering, and conditional pushback.
///
/// # IME / caret caveat (FU-TF-2)
///
/// The edit side writes UP only (`on:input` → `set_str`); the display effect
/// rewrites the DOM value ONLY when it differs from the signal string, so
/// identical-value re-renders never move the caret and mid-composition IME
/// state is not clobbered by same-value writes. Residual composition edge
/// cases vary by engine and remain a known limitation — verify with a manual
/// browser smoke test.
///
/// # No-provider blur behavior (L-1)
///
/// When no ancestor called [`crate::use_form`], blur resolution yields
/// `None` and marking is skipped: headless usage still renders and binds
/// correctly, it simply never marks touched.
#[component]
// reason: typed-builder consumes props by value; component fns are never called directly
#[allow(
    clippy::needless_pass_by_value,
    reason = "leptos typed-builder prop convention"
)]
pub fn TextField(
    field: FieldHandle,
    #[prop(optional)] label: Option<String>,
    #[prop(optional)] id: Option<String>,
    #[prop(optional)] placeholder: Option<String>,
) -> impl IntoView {
    let input_id = id.unwrap_or_else(|| sanitize_id(&field.path().to_string()));
    let input_ref = NodeRef::<leptos::html::Input>::new();

    let field_for_effect = field.clone();
    let input_id_for_label = input_id.clone();
    // display side: conditional pushback (FU-TF-2)
    Effect::new(move |_| {
        let sig_str = field_for_effect.get_str().unwrap_or_default();
        if let Some(el) = input_ref
            .get()
            .filter(|el| should_push(&el.value(), &sig_str))
        {
            el.set_value(&sig_str);
        }
    });

    let field_for_input = field.clone();
    let field_for_blur = field.clone();
    let visible = field.visible_errors();

    view! {
        <div class="forma-field">
            {move || {
                let for_id = input_id_for_label.clone();
                label.clone().map(move |text| {
                    view! { <label for=for_id>{text}</label> }
                })
            }}
            <input
                type="text"
                id=input_id.clone()
                placeholder=placeholder
                node_ref=input_ref
                on:input=move |ev| field_for_input.set_str(&event_target_value(&ev))
                on:blur=move |_| {
                    if let Some(controller) = try_form_controller() {
                        controller.mark_touched(field_for_blur.path());
                    }
                }
            />
            {move || {
                let issues = visible.get();
                (!issues.is_empty()).then(move || {
                    let items: Vec<_> = issues
                        .into_iter()
                        .map(|issue| {
                            let message = issue.message.to_string();
                            view! { <li>{message}</li> }
                        })
                        .collect();
                    view! { <ul class="forma-errors">{items}</ul> }
                })
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_key_gets_prefixed_id() {
        assert_eq!(sanitize_id("email"), "forma-email");
    }

    #[test]
    fn dotted_path_becomes_single_dashes() {
        assert_eq!(sanitize_id("user.email"), "forma-user-email");
    }

    #[test]
    fn dash_runs_collapse_to_one_dash() {
        assert_eq!(sanitize_id("a--b  c"), "forma-a-b-c");
    }

    #[test]
    fn unicode_alphanumerics_pass_through() {
        assert_eq!(sanitize_id("café"), "forma-café");
    }

    #[test]
    fn should_push_equal_is_false() {
        assert!(!should_push("abc", "abc"));
        assert!(!should_push("", ""));
    }

    #[test]
    fn should_push_divergent_is_true() {
        assert!(should_push("", "abc"));
        assert!(should_push("abc", ""));
        assert!(should_push("old", "new"));
    }
}
