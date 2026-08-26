# Changelog

All notable changes to the formars workspace are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Breaking changes

- **core**: `#[derive(FormSchema)]`-generated companion structs are now
  `Clone` (mirroring `ObjectSchema: Clone`, also new); hand-written
  companions used as nested children must add `Clone`.
- **derive**: `#[form(rename = "")]` is now a hard compile error spanned on
  the empty literal ("the wire key would be the empty string"); previously it
  was silently accepted, producing a wire key of `""`. Non-empty and dotted
  renames (`rename = "a.b"`) remain legal.
- **derive**: raw identifier fields (`r#type`) now derive their wire key as
  `type` (the `r#` prefix is Rust syntax, never part of the object key);
  previously the wire key was `"r#type"`.
- **core**: `FieldPath`'s `Display` now quotes keys containing `.`, `[`, `]`,
  a backtick, or the empty key with backtick-wrapping and doubled embedded
  backticks, so structurally different paths can no longer render identically
  (e.g. key `a.b` inside `user` no longer renders like nested `user.a.b`).
  Separator-free keys render byte-identically to before; ROOT still renders
  as the empty string.
- **core**: the phantom `std` feature was removed from `formars-core`
  (declared, defaulted, referenced by zero `cfg`s). Explicit
  `--features std` now fails with "package does not have feature `std`";
  default-feature consumers are unaffected.
- **derive**: duplicate attribute errors name the offending key
  ("duplicate `rename` attribute"); unknown multi-segment attribute keys
  render their full source text (`unknown attribute \`foo::bar\``) instead of
  a bare `::`; unit-struct rejection now spans the struct's identifier.

### Added

- **core**: the prelude additionally exports `Object`, `ToValue`,
  `RefineRejection`, and `FieldMeta` — one-import programs can now build
  objects, implement custom rules, and name introspection return types. The
  facade's explicit export list is unchanged; its glob surface grows
  additively.
- **signals**: `apply_server_errors` captures known paths AND each field's
  current value under ONE registry acquisition, eliminating the window where
  a concurrent edit could anchor a server baseline to a value torn against a
  stale grouping snapshot; all signal writes remain outside the lock.

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

### Changed

- **core**: `NumberSchema`'s `Debug` output gains a `rules` field for parity
  with `StringSchema` (`Debug` output is non-contractual).
