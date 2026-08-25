//! The `use_form` ownership/lifetime seam and context model (FU-HK-1/2/3).

use formars_signals::{FormController, ValidateOn};
use reactive_graph::owner::{expect_context, provide_context, use_context};

/// Returned by [`use_form`]; the hook also provisions the controller as
/// Leptos context so descendants can resolve it.
///
/// Register/mutate through `controller` — clones share the same underlying
/// signals (`Arc`-backed), so there is exactly one source of truth per field.
#[derive(Clone)]
pub struct UseForm {
    /// The constructed controller. `register*` take `&mut self`, so keep a
    /// local `mut` binding; event-handler closures capture cheap clones.
    pub controller: FormController,
}

/// Constructs a controller with `default_validate_on`, provides it as
/// context (`provide_context(controller.clone())`), and returns it.
///
/// Call this in a component body (Leptos 0.8 bodies run once per mount, so
/// the owned controller lives for the component lifetime).
#[must_use]
pub fn use_form(default_validate_on: ValidateOn) -> UseForm {
    let controller = FormController::new(default_validate_on);
    provide_context(controller.clone());
    UseForm { controller }
}

/// Resolves the ancestor-provided form controller.
///
/// # Panics
///
/// Panics when no ancestor called [`use_form`] (deterministic documented
/// behavior, FU-HK-2). Use [`try_form_controller`] for the graceful variant.
#[must_use]
#[track_caller]
pub fn use_form_controller() -> FormController {
    expect_context::<FormController>()
}

/// Graceful variant of [`use_form_controller`]: `None` when no ancestor
/// called [`use_form`].
#[must_use]
pub fn try_form_controller() -> Option<FormController> {
    use_context::<FormController>()
}
