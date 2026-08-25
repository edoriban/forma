# Changelog

All notable changes to the formars workspace are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Breaking changes

- **derive**: `#[derive(FormSchema)]` now emits a compile error when two
  non-skipped fields resolve to the SAME wire key (via `rename` or the field
  identifier). Previously such collisions compiled silently and corrupted
  lookups through last-write-wins object semantics; pre-1.0 this accepted
  break was chosen deliberately. Rename one of the colliding fields with
  `#[form(rename = "..")]`.
- **ui**: `<TextField>`'s SSR `value` attribute now renders canonical strings
  for numeric/bool initials (`Value::I64(7)` → `"7"`, `Value::Bool(true)` →
  `"true"`); previously any non-string initial rendered as an empty input.

### Fixed

- Submit-attempt acquisition (`begin_attempt`) is now a single atomic
  compare-and-update over the in-flight flag: among concurrent callers on a
  shared controller exactly one wins; losing callers leave no flag writes and
  no reactive traces (previously a get-then-set race allowed multiple
  simultaneous "winners" across threads).
- `FormController::reset()` now also clears form-level (unmatched) server
  issues, so ghost errors no longer survive a reset, and its docs state
  explicitly that the `submitted` flag is caller-owned and deliberately not
  cleared.
- Non-string scalar values render canonically in inputs via the new single
  display seam `FieldHandle::display_str()` — programmatic
  `set_i64`/`set_f64`/`set_bool` followed by render shows `"42"`/`"0.1"`/
  `"true"` instead of blanking the field.
- `TextField` resolves its form controller once at build time and moves the
  captured value into the blur handler; event-time context owner-restoration
  is no longer relied upon (no-provider blur remains a silent no-op).
- Qualified primitive paths (`::std::string::String`,
  `::core::primitive::u32`) as field types now produce a targeted derive
  diagnostic naming the field and remedies, instead of surfacing as trait-bound
  noise suggesting `#[derive(FormSchema)]` on the primitive; the derive crate
  docs now describe name-based mapping truthfully (aliases fall through to
  nested composition).
