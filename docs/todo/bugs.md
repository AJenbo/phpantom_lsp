# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

Each entry below carries an **Impact · Complexity** rating using the same
scale defined in [`docs/todo.md`](../todo.md); that table is also where
each bug's row lives in the current sprint/backlog.

Bugs land here from wherever they surface: found while working on another
task, or sweeps of the sample projects under `projects/`. Entries are
grouped by the mechanism that has to change, not by the symptom that
surfaced: one entry is one root cause, however many shapes it shows up in.

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

No outstanding items.

## Reachability

No outstanding items.

## Narrowing

No outstanding items.

## Arithmetic

No outstanding items.

## Symbol resolution

No outstanding items.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.

## Miscellaneous

## B2. A `blade_directive` directive shifts every component tag's bound attributes

**Impact: Medium · Complexity: Medium**

`extract_component_call_site_vars` in
`src/blade/call_site_inference.rs` pairs a component tag's bound
attributes with the expressions they pass by *counting*:
`scan_component_tag_calls` in `src/blade/component_tags.rs` numbers the
bound attributes of a template in document order, and
`BladeDirectiveWalker` collects every `blade_directive(EXPR)` call of the
same template's virtual PHP in document order, then index N of the first
list reads `calls[N]`.

The count only lines up if a bound attribute is the *only* thing that
emits that marker, and it is not. `translate_directive` lowers
`@class`, `@style`, `@json`, `@dump`, `@js`, `@dd`, `@slot`, `@props`,
`@aware`, `@lang`, `@choice`, `@vite` and `@fonts` to the same
`blade_directive`, so a template writing any of them ahead of a tag
shifts the pairing by one per occurrence:

```blade
@class(['featured' => $flag])
<x-alert :title="$heading" />
```

lowers to `blade_directive(['featured' => $flag]); … blade_directive($heading);`,
so `alert.blade.php` is told its `$title` is `array{featured: bool}`.
Every attribute after the first such directive is typed from the wrong
expression, or from none at all once the count runs past the end.

**Fix:** Give the bound-attribute marker a name of its own, the way
`@can`, `@each`, `@section` and the view directives already have theirs
(`blade_can_directive`, `blade_each_directive`, `blade_section_directive`,
`blade_view_directive`), and count only that one. Then the two scans
cannot be desynchronised by a directive that happens to share a marker.
A test in `tests/integration/blade_call_site_inference.rs` asserting a
tag's bound attribute still types correctly with a `@class` above it is
what would have caught this.
