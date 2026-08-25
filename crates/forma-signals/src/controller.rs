//! The form controller: an insertion-ordered registry of erased field cells.

use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};

use reactive_graph::computed::ArcMemo;
use reactive_graph::signal::ArcRwSignal;
use reactive_graph::traits::{Get, Set, Update};

use forma_core::error::{FieldPath, FormaError, FormaIssue};
use forma_core::schema::DynSchema;
use forma_core::value::Value;

use crate::field::{FieldCell, FieldHandle, ValidateOn};
use crate::submit::{FormSnapshot, SubmitError};
use crate::validation::{OrderedPath, display_gate, group_issues, stamp_issues};

/// Error returned by [`FormController::register`].
#[derive(Clone, Debug, PartialEq)]
pub enum RegisterError {
    /// The path is already registered; the existing field was left untouched.
    Duplicate {
        /// The offending path.
        path: FieldPath,
    },
}

pub(crate) struct Inner {
    pub(crate) default_validate_on: ValidateOn,
    pub(crate) submitted: ArcRwSignal<bool>,
    pub(crate) is_submitting: ArcRwSignal<bool>,
    pub(crate) unmatched_server: ArcRwSignal<Vec<FormaIssue>>,
    /// Bumped on every registration so memos reading the (non-reactive)
    /// registry mutex re-run when fields are added after their first read.
    pub(crate) registration_epoch: ArcRwSignal<usize>,
    pub(crate) fields: Mutex<Vec<(FieldPath, FieldCell, FieldHandle)>>,
}

struct FieldMemos {
    errors: ArcMemo<Vec<FormaIssue>>,
    visible: ArcMemo<Vec<FormaIssue>>,
}

/// RAII bracket for `is_submitting`: true on creation, false on drop, so a
/// cancelled (dropped) submit future never leaves the flag stuck true.
struct SubmittingGuard(ArcRwSignal<bool>);

impl SubmittingGuard {
    fn engage(flag: ArcRwSignal<bool>) -> Self {
        flag.set(true);
        Self(flag)
    }
}

impl Drop for SubmittingGuard {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

struct ResetTargets {
    value: ArcRwSignal<Value>,
    initial: Value,
    touched: ArcRwSignal<bool>,
    server: ArcRwSignal<Vec<FormaIssue>>,
    baseline: ArcRwSignal<Value>,
}

/// Controller-owned registry of form fields.
///
/// Owns every value signal and state cell; callers only receive
/// [`FieldHandle`] accessors. Clones share the same underlying signals and
/// memos, so there is exactly one source of truth per field value. Memos
/// created owner-less (headless) are never disposed — acceptable because the
/// controller lives for the form's lifetime.
///
/// v0 fields anchor their state to a `Value` snapshot taken at registration;
/// that snapshot anchors [`FieldHandle::dirty`], [`FormController::reset`],
/// and the server-baseline comparison. Default registration snapshots the
/// empty-string `Value`; [`FormController::register_initial`] and
/// [`FormController::register_initial_with`] snapshot a caller-supplied
/// value instead (for prefilled fields). The snapshot is immutable after
/// registration.
#[derive(Clone)]
pub struct FormController {
    pub(crate) inner: Arc<Inner>,
    pub(crate) form_errors: ArcMemo<Vec<FormaIssue>>,
}

impl FormController {
    /// Creates a controller with the given default validation timing.
    ///
    /// # Panics
    ///
    /// Only if the registry mutex is poisoned by a panic in another thread
    /// holding it (never in normal use).
    #[must_use]
    pub fn new(default_validate_on: ValidateOn) -> Self {
        let inner = Arc::new(Inner {
            default_validate_on,
            submitted: ArcRwSignal::new(false),
            is_submitting: ArcRwSignal::new(false),
            unmatched_server: ArcRwSignal::new(Vec::new()),
            registration_epoch: ArcRwSignal::new(0),
            fields: Mutex::new(Vec::new()),
        });
        let for_aggregate = inner.clone();
        let form_errors = ArcMemo::new(move |_| {
            // Track the epoch so later registrations invalidate the aggregate;
            // the mutex read below is invisible to the reactive graph.
            let _epoch = for_aggregate.registration_epoch.get();
            let visible_per_field: Vec<ArcMemo<Vec<FormaIssue>>> = {
                let fields = for_aggregate
                    .fields
                    .lock()
                    .expect("forma-signals registry mutex poisoned");
                fields
                    .iter()
                    .map(|(_, c, _)| c.visible_errors.clone())
                    .collect()
            };
            let mut out = Vec::new();
            for visible in visible_per_field {
                out.extend(visible.get());
            }
            out.extend(for_aggregate.unmatched_server.get());
            out
        });
        Self { inner, form_errors }
    }

    /// Registers a field using the controller-default timing mode, rejecting
    /// duplicate paths (fail-fast at the bug site: a double render or
    /// copy-paste build surfaces here instead of forking state invisibly).
    ///
    /// # Errors
    ///
    /// Returns [`RegisterError::Duplicate`] if `path` is already registered.
    pub fn register(
        &mut self,
        path: FieldPath,
        schema: Box<dyn DynSchema>,
    ) -> Result<FieldHandle, RegisterError> {
        self.register_inner(path, schema, None, Value::from(""))
    }

    /// Registers a field with an explicit timing mode overriding the
    /// controller default.
    ///
    /// # Errors
    ///
    /// Returns [`RegisterError::Duplicate`] if `path` is already registered.
    pub fn register_with(
        &mut self,
        path: FieldPath,
        schema: Box<dyn DynSchema>,
        validate_on: ValidateOn,
    ) -> Result<FieldHandle, RegisterError> {
        self.register_inner(path, schema, Some(validate_on), Value::from(""))
    }

    /// Registers a prefilled field using the controller-default timing mode,
    /// with `initial` as the dirty/reset/server-baseline anchor instead of
    /// the empty-string default. The snapshot is immutable after
    /// registration — no re-anchoring API exists.
    ///
    /// `initial` is NOT validated against the field's schema at registration
    /// time; if it violates constraints, the error memo reports those issues
    /// immediately (identical to registering then setting the value).
    ///
    /// # Errors
    ///
    /// Returns [`RegisterError::Duplicate`] if `path` is already registered;
    /// the existing field is left untouched.
    pub fn register_initial(
        &mut self,
        path: FieldPath,
        schema: Box<dyn DynSchema>,
        initial: Value,
    ) -> Result<FieldHandle, RegisterError> {
        self.register_inner(path, schema, None, initial)
    }

    /// Same as [`FormController::register_initial`] plus an explicit timing
    /// mode overriding the controller default, symmetric with
    /// [`FormController::register_with`].
    ///
    /// `initial` is NOT validated against the field's schema at registration
    /// time; if it violates constraints, the error memo reports those issues
    /// immediately (identical to registering then setting the value).
    ///
    /// # Errors
    ///
    /// Returns [`RegisterError::Duplicate`] if `path` is already registered;
    /// the existing field is left untouched.
    pub fn register_initial_with(
        &mut self,
        path: FieldPath,
        schema: Box<dyn DynSchema>,
        validate_on: ValidateOn,
        initial: Value,
    ) -> Result<FieldHandle, RegisterError> {
        self.register_inner(path, schema, Some(validate_on), initial)
    }

    fn register_inner(
        &self,
        path: FieldPath,
        schema: Box<dyn DynSchema>,
        override_on: Option<ValidateOn>,
        initial: Value,
    ) -> Result<FieldHandle, RegisterError> {
        let value = ArcRwSignal::new(initial.clone());
        let touched = ArcRwSignal::new(false);
        let server = ArcRwSignal::new(Vec::<FormaIssue>::new());
        let server_baseline = ArcRwSignal::new(initial.clone());
        let schema: Arc<dyn DynSchema> = schema.into();
        let validate_on = override_on.unwrap_or(self.inner.default_validate_on);

        let dirty_value = value.clone();
        let dirty_initial = initial.clone();
        let dirty = ArcMemo::new(move |_| dirty_value.get() != dirty_initial);

        let errors_schema = Arc::clone(&schema);
        let errors_value = value.clone();
        let errors_path = path.clone();
        let errors = ArcMemo::new(move |_| {
            stamp_issues(
                errors_schema.validate_value(&errors_value.get()),
                &errors_path,
            )
        });

        let gate = display_gate(&touched, &self.inner.submitted, validate_on);

        let visible_gate = gate.clone();
        let visible_errors_memo = errors.clone();
        let visible_value = value.clone();
        let visible_baseline = server_baseline.clone();
        let visible_server = server.clone();
        let visible_errors = ArcMemo::new(move |_| {
            let mut out = if visible_gate.get() {
                visible_errors_memo.get()
            } else {
                Vec::new()
            };
            if visible_value.get() == visible_baseline.get() {
                out.extend(visible_server.get());
            }
            out
        });

        let handle = FieldHandle {
            path: path.clone(),
            value: value.clone(),
            touched: touched.clone(),
            dirty,
            errors,
            visible_errors: visible_errors.clone(),
            validate_on,
        };
        let cell = FieldCell {
            initial,
            value,
            touched,
            schema,
            validate_on,
            server,
            server_baseline,
            visible_errors,
        };

        let mut fields = self.lock_fields();
        if fields.iter().any(|(p, _, _)| *p == path) {
            return Err(RegisterError::Duplicate { path });
        }
        fields.push((path, cell, handle.clone()));
        drop(fields);
        self.inner.registration_epoch.update(|n| *n += 1);
        Ok(handle)
    }

    /// Looks up the handle for a registered path.
    ///
    /// The returned handle is a cheap clone sharing the same underlying cells.
    #[must_use]
    pub fn field(&self, path: &FieldPath) -> Option<FieldHandle> {
        let fields = self.lock_fields();
        fields
            .iter()
            .find(|(p, _, _)| p == path)
            .map(|(_, _, h)| h.clone())
    }

    /// The resolved validation-timing mode for `path`, or `None` when the
    /// path is not registered.
    #[must_use]
    pub fn effective_validate_on(&self, path: &FieldPath) -> Option<ValidateOn> {
        let fields = self.lock_fields();
        fields
            .iter()
            .find(|(p, _, _)| p == path)
            .map(|(_, c, _)| c.validate_on)
    }

    /// Marks the field touched — the blur seam driving visibility gates.
    pub fn mark_touched(&self, path: &FieldPath) {
        let touched = {
            let fields = self.lock_fields();
            fields
                .iter()
                .find(|(p, _, _)| p == path)
                .map(|(_, c, _)| c.touched.clone())
        };
        if let Some(touched) = touched {
            touched.set(true);
        }
    }

    /// Forces synchronous recomputation of the field's error memos within the
    /// call — no await, no scheduling (FSV-2).
    pub fn revalidate(&self, path: &FieldPath) {
        let memos: Option<FieldMemos> = {
            let fields = self.lock_fields();
            fields
                .iter()
                .find(|(p, _, _)| p == path)
                .map(|(_, c, h)| FieldMemos {
                    errors: h.errors.clone(),
                    visible: c.visible_errors.clone(),
                })
        };
        if let Some(FieldMemos { errors, visible }) = memos {
            let _ = errors.get();
            let _ = visible.get();
        }
    }

    /// The form-level submit-attempt flag feeding every display gate.
    #[must_use]
    pub fn submitted(&self) -> ArcRwSignal<bool> {
        self.inner.submitted.clone()
    }

    /// The in-flight submit flag: true between the snapshot gate and the
    /// handler future settling, across success and failure alike.
    #[must_use]
    pub fn is_submitting(&self) -> ArcRwSignal<bool> {
        self.inner.is_submitting.clone()
    }

    /// Reactive aggregate of every field's display-gated issues plus
    /// form-level (unmatched) server issues.
    #[must_use]
    pub fn form_errors(&self) -> ArcMemo<Vec<FormaIssue>> {
        self.form_errors.clone()
    }

    /// Merges server-side issues into field state. Issues addressed to
    /// registered paths REPLACE that field's server cells; unknown or ROOT
    /// paths land in the form-level unmatched collection. A subsequent edit
    /// to a field hides its stale server issues until the next apply.
    pub fn apply_server_errors(&self, error: &FormaError) {
        let known: Vec<FieldPath> = {
            let fields = self.lock_fields();
            fields.iter().map(|(p, _, _)| p.clone()).collect()
        };
        let (per_field, unmatched) = group_issues(error.issues.clone(), &known);

        let updates: Vec<(ArcRwSignal<Vec<FormaIssue>>, ArcRwSignal<Value>, Value)> = {
            let fields = self.lock_fields();
            fields
                .iter()
                .map(|(_, c, _)| (c.server.clone(), c.server_baseline.clone(), c.value.get()))
                .collect()
        };
        for (path, (server, baseline, current)) in known.into_iter().zip(updates) {
            let matched = per_field
                .get(&OrderedPath::new(path))
                .cloned()
                .unwrap_or_default();
            server.set(matched);
            baseline.set(current);
        }
        self.inner.unmatched_server.set(unmatched);
    }

    /// Validates the whole form synchronously against each field's schema.
    ///
    /// Independent of display gates — a hidden (untouched) violation still
    /// fails submission.
    ///
    /// # Errors
    ///
    /// Returns every violated field's issues, in declaration order, when any
    /// registered field fails.
    pub fn validate(&self) -> Result<(), FormaError> {
        let checks: Vec<(FieldPath, Arc<dyn DynSchema>, Value)> = {
            let fields = self.lock_fields();
            fields
                .iter()
                .map(|(p, c, _)| (p.clone(), Arc::clone(&c.schema), c.value.get()))
                .collect()
        };
        let mut issues = Vec::new();
        for (path, schema, value) in checks {
            issues.extend(stamp_issues(schema.validate_value(&value), &path));
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(FormaError { issues })
        }
    }

    /// Takes a point-in-time copy of all field values in declaration order.
    #[must_use]
    pub fn snapshot(&self) -> FormSnapshot {
        let entries: Vec<(FieldPath, Value)> = {
            let fields = self.lock_fields();
            fields
                .iter()
                .map(|(p, c, _)| (p.clone(), c.value.get()))
                .collect()
        };
        FormSnapshot { entries }
    }

    /// Composes the submit boundary as a plain future: whole-form sync
    /// validation first (a failure resolves with [`SubmitError::Validation`]
    /// and never constructs `handler`), then a [`FormSnapshot`] gate,
    /// `is_submitting` bracketing the attempt via a drop guard — the flag
    /// resets on success, handler error, validation failure, AND when the
    /// composed future is dropped (cancelled) mid-flight. No spawner
    /// required; the caller owns scheduling.
    pub fn on_submit<T, E, F, Fut>(
        &self,
        handler: F,
    ) -> impl Future<Output = Result<T, SubmitError<E>>>
    where
        F: FnOnce(FormSnapshot) -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let this = self.clone();
        async move {
            // Engaged before validation so a synchronous pre-flip (see the
            // UI-side double-submit guard) is always reset exactly once, on
            // every exit path including cancellation.
            let _guard = SubmittingGuard::engage(this.inner.is_submitting.clone());
            if let Err(err) = this.validate() {
                return Err(SubmitError::Validation(err));
            }
            let form = this.snapshot();
            let result = handler(form).await;
            result.map_err(SubmitError::Handler)
        }
    }

    /// Restores every field to its pristine registered state.
    pub fn reset(&self) {
        let restored: Vec<ResetTargets> = {
            let fields = self.lock_fields();
            fields
                .iter()
                .map(|(_, c, _)| ResetTargets {
                    value: c.value.clone(),
                    initial: c.initial.clone(),
                    touched: c.touched.clone(),
                    server: c.server.clone(),
                    baseline: c.server_baseline.clone(),
                })
                .collect()
        };
        for ResetTargets {
            value,
            initial,
            touched,
            server,
            baseline,
        } in restored
        {
            value.set(initial.clone());
            touched.set(false);
            server.set(Vec::new());
            baseline.set(initial);
        }
    }

    pub(crate) fn lock_fields(&self) -> MutexGuard<'_, Vec<(FieldPath, FieldCell, FieldHandle)>> {
        self.inner
            .fields
            .lock()
            .expect("forma-signals registry mutex poisoned")
    }
}
