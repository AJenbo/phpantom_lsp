# PHPStan Code Actions

Code actions that respond to PHPStan diagnostics. Each action parses the PHPStan
error message, extracts the relevant information, and offers a quickfix that
modifies the source code to resolve the issue.


## Prerequisites — Infrastructure improvements

No outstanding items.

---

## Tier 1 — Trivial (no message parsing or simple static message)

No outstanding items.

---

## Tier 2 — Simple message parsing

### H6. `return.type` — Update return type to match actual returns

**Identifier:** `return.type`
**Messages:**
- `Method Foo::bar() should return {expected} but returns {actual}.`
- `Function foo() should return {expected} but returns {actual}.`
- `Anonymous function should return {expected} but returns {actual}.`

Parse `{actual}` from the message with regex:
`should return (.+) but returns (.+)\.$`

Offer two quickfixes:

1. **Update native return type** — find the `: Type` after the parameter list
   and replace `Type` with the actual type. Only offer this when the actual
   type is a valid native PHP type (scalars, class names, `null`, simple
   unions on PHP 8.0+, intersection types on PHP 8.1+).
2. **Update `@return` tag** — if a docblock with `@return` exists, replace the
   type. If no docblock exists, create one with `@return {actual}`.

Mark neither as `is_preferred` since the right fix might be to change the code
rather than the signature.

**Stale detection:** difficult to do precisely because PHPStan's type syntax
differs from source syntax. As a heuristic, check whether the return type
declaration or `@return` tag was modified since the diagnostic was issued
(i.e. the text on the relevant line no longer matches the `{expected}` type).
Or skip stale detection for this one — the `@phpstan-ignore` stale check
already covers the suppress-with-comment path.

---

### H7. `return.phpDocType` — Fix `@return` to match native type

**Identifier:** `return.phpDocType`
**Messages:**
- `PHPDoc tag @return with type {phpdoc} is incompatible with native type {native}.`
- `PHPDoc tag @return with type {phpdoc} is not subtype of native type {native}.`

Parse both types. Offer two quickfixes:

1. **Update `@return` to `{native}`** — replace the `@return` type in the
   docblock.
2. **Remove `@return` tag** — the native type is authoritative, so just
   remove the redundant/wrong docblock tag. Mark as `is_preferred`.

Reuse docblock editing from `update_docblock.rs`.

**Stale detection:** the `@return` tag no longer contains `{phpdoc}`, or no
`@return` tag exists.

---

### H8. `parameter.phpDocType` — Fix `@param` to match native type

**Identifier:** `parameter.phpDocType`
**Messages:**
- `PHPDoc tag @param for parameter $name with type {phpdoc} is incompatible with native type {native}.`
- `PHPDoc tag @param for parameter $name with type {phpdoc} is not subtype of native type {native}.`

Parse the parameter name and both types. Offer:

1. **Update `@param $name` to `{native}`**
2. **Remove `@param $name` tag** — mark as `is_preferred`.

Same docblock editing pattern as P7.

**Stale detection:** `@param` tag for `$name` no longer contains `{phpdoc}`,
or no such `@param` tag exists.

> **Implementation note:** P7, P8, and P9 share nearly identical logic
> (parse PHPDoc vs native mismatch, offer update-or-remove). Consider
> implementing them together with a shared "fix PHPDoc type mismatch" helper
> that is parameterised by the tag name (`@return`, `@param`, `@var`).

---

### H9. `property.phpDocType` — Fix property docblock type

**Identifier:** `property.phpDocType`
**Messages:**
- `{desc} for property Foo::$bar with type {phpdoc} is incompatible with native type {native}.`
- `{desc} for property Foo::$bar with type {phpdoc} is not subtype of native type {native}.`

Parse property name and both types. Offer to update or remove the `@var` tag
on the property docblock. Same pattern as P7/P8.

**Stale detection:** `@var` tag no longer contains `{phpdoc}`, or no `@var`
tag exists near the property.

---

### H10. `return.unusedType` — Remove unused type from return union

**Identifier:** `return.unusedType`
**Messages:**
- `Method Foo::bar() never returns {type} so it can be removed from the return type.`
- `Function foo() never returns {type} so it can be removed from the return type.`

Parse `{type}` from the message. Find the return type (native or `@return`),
parse the union/intersection, remove the unused member, and rewrite.

For native types: `string|null` with unused `null` becomes `string`.
For docblock types: same logic on the `@return` tag.
Handle intersection types too: `Foo&Bar` with unused `Bar` becomes `Foo`.

If removing the type would leave a single-member union, simplify
(e.g. `string|null` minus `null` becomes `string`).

**Stale detection:** the return type no longer contains `{type}` as a union
or intersection member.

---

### H11. `method.visibility` / `property.visibility` — Fix overriding visibility

**Identifiers:** `method.visibility`, `property.visibility`
**Messages:**
- `{Private|Protected} method Foo::bar() overriding public method Parent::bar() should also be public.`
- `Private method Foo::bar() overriding protected method Parent::bar() should be protected or public.`
- (equivalent patterns for properties)

Parse the required visibility from the message. Use the existing
`change_visibility.rs` infrastructure to change the visibility modifier,
but pre-select the correct target visibility instead of offering all three.

When the message says "should also be public", offer only `public`.
When the message says "should be protected or public", offer `protected`
(as `is_preferred`) and `public`.

Mark as `is_preferred` when there is only one correct answer.

**`ignorable: false`:** These errors are non-ignorable in PHPStan — the
"Add `@phpstan-ignore`" action would produce a broken ignore comment. This
is one of the motivations for prerequisite R1: once we parse `ignorable`,
the ignore action will be automatically suppressed.

**Stale detection:** the visibility keyword on the diagnostic line has changed
from the original value.

---

### H4. `assign.byRefForeachExpr` — Unset by-reference foreach variable

**Identifier:** `assign.byRefForeachExpr`
**Tip (in message):** `Unset it right after foreach to avoid this problem.`

The diagnostic is on a line that uses a variable that was previously bound as a
by-reference foreach variable. The fix is to insert `unset($var);` after the
foreach loop that created the binding.

**Implementation steps:**

1. The diagnostic line references the variable. Extract the variable name
   from the diagnostic line by finding the `$var` on that line (we can look
   for the first `$identifier` on the line, or parse the message — but the
   message doesn't include the variable name, so we must scan the source).
2. Search backward from the diagnostic line for a `foreach` statement
   containing `&$var` (the by-reference binding).
3. Find the closing `}` (or `endforeach;`) of that foreach.
4. Insert `unset($var);` on the line after the closing brace, with matching
   indentation.

This is trickier than the other Tier 1/2 items because of the need to locate
the foreach loop and its closing brace. Brace-matching is fragile without a
real parser, but a simple nesting-depth counter works for well-formatted code.

**Stale detection:** `unset($var)` appears between the foreach closing brace
and the diagnostic line.

---

### H12. `class.prefixed` — Fix prefixed class name

**Identifier:** `class.prefixed`
**Tip (in message):** `This is most likely unintentional. Did you mean to type {corrected}?`

Parse the corrected class name from the tip using:
`Did you mean to type (.+)\?$`

The tip is available in the diagnostic message after the `\n` separator
(see R3). Replace the prefixed name on the diagnostic line with the
corrected one.

**Caveat:** the diagnostic range currently covers the whole line
(`character: 0` to `character: MAX`). We need to find the actual prefixed
class name on the line — look for a backslash-prefixed version of the
corrected name (e.g. `\DateTime` when corrected is `DateTime`).

**Stale detection:** the prefixed class name no longer appears on the
diagnostic line.

---

## Tier 3 — Requires locating related code

### H13. `property.notFound` (same-class) — Declare missing property

**Identifier:** `property.notFound`
**Message:** `Access to an undefined property Foo::$bar.`

Parse class name and property name from the message:
`Access to an undefined property (.+)::\$(.+)\.$`

Scope to same-file only: when the diagnostic is on `$this->bar`, the fix
targets the current class. When it references a different class, skip.

Offer two quickfixes:

1. **Declare property** — insert a property declaration at the top of the
   class body, after existing property declarations. Use `private` visibility
   and `mixed` type by default. If the diagnostic is on an assignment like
   `$this->bar = expr;`, we might infer a better type later, but start with
   `mixed`.
2. **Add `@property` PHPDoc** — add `@property mixed $bar` to the class
   docblock. Better for classes that use `__get`/`__set`.

**Stale detection:** the class now declares `$bar` as a property, or the
class docblock contains `@property ... $bar`.

**Reference:** https://phpstan.org/blog/solving-phpstan-access-to-undefined-property

---

### H14. `throws.unusedType` (narrow) — Narrow `@throws` to actual thrown types

**Identifier:** `throws.unusedType`
**Tip (in message):** `You can narrow the thrown type with PHPDoc tag @throws {narrowed_type}.`

When the tip (after the `\n` separator in the message) contains
`You can narrow the thrown type`, parse the narrowed type using:
`@throws (.+)\.?$`

Offer to replace the existing `@throws` tag with the narrowed type. This is
different from the existing "Remove @throws" action: instead of removing the
tag entirely, it replaces it with a more precise type.

The existing `remove_throws.rs` already handles `throws.unusedType` for the
"remove" case. This new action should be offered alongside it, with
"Narrow @throws" marked as `is_preferred` (since it preserves documentation).

**Stale detection:** the `@throws` tag now matches the narrowed type.

---

### H15. Template bound from tip — Add `@template T of X`

**Identifiers:** various (`generics.*`, `phpDoc.*` — needs investigation)
**Tip (in message):** `Write @template T of X to fix this.`

Parse the `@template` declaration from the tip using:
`Write (@template .+ of .+) to fix this\.`

Insert the `@template` tag into the class or function docblock (create one
if needed). Same docblock insertion pattern as `add_throws.rs`.

**Stale detection:** the docblock now contains the extracted `@template` tag.

---

### H16. `match.unhandled` — Add missing match arms

**Identifier:** `match.unhandled`
**Message:** `Match expression does not handle remaining value(s): {types}`

Parse the remaining value(s) from the message:
`does not handle remaining value\(s\): (.+)$`

The value list is comma-separated. Each value can be:
- An enum case: `Foo::Bar` — generate `Foo::Bar => TODO`
- A string literal: `'foo'` — generate `'foo' => TODO`
- An int literal: `42` — generate `42 => TODO`
- A type name: `int` — generate `default => TODO` (catch-all)

Find the match expression on the diagnostic line. Locate its closing `}`.
Insert new arms before the closing `}` with correct indentation.

Use `throw new \LogicException('Unexpected value')` as the arm body, or
a `TODO` comment — configurable later.

**Stale detection:** difficult without re-parsing the match. Skip for now.

---

## Tier 4 — Requires body analysis

### H17. `missingType.iterableValue` (return type) — Add `@return` with iterable type

**Identifier:** `missingType.iterableValue`
**Messages:**
- `Method Foo::bar() return type has no value type specified in iterable type array.`
- `Function foo() return type has no value type specified in iterable type array.`

Only handle the "return type" variant (not parameter/property). Parse the
iterable type name (`array`, `iterable`, `Traversable`, etc.) from the message.

**Simplest approach (start here):**

Offer to add `@return array<mixed>` (or `list<mixed>`, `iterable<mixed>`, etc.
matching the native type). This silences the PHPStan error while being explicit.
PHPStan's documentation recommends this as the quick fix:
> "If you just want to make this error go away, replace array with mixed[]
> or array<mixed>."

**Enhanced approach (later):**

Walk the function body for `return` statements and infer element types from
array literals using the existing `infer_element_type` /
`infer_array_literal_raw_type` logic. If all return expressions are array
literals with consistent value types, offer `@return array<ValueType>`.

**Stale detection:** a `@return` tag exists with a generic array type
(contains `<` or `[]`).

**Reference:** https://phpstan.org/blog/solving-phpstan-no-value-type-specified-in-iterable-type

---

## Tier 5 — Lower priority / more complex

### H18. `deadCode.unreachable` — Remove unreachable code

**Identifier:** `deadCode.unreachable`
**Message:** `Unreachable statement - code above always terminates.`

Delete the unreachable statement. Start with single-statement removal (delete
from the diagnostic line to the next `;`). Multi-statement dead code removal
is hard without an AST.

**Stale detection:** the diagnostic line is now empty or a `}`.

---

### H19. `property.unused` / `method.unused` / `classConstant.unused` — Remove unused member

**Identifiers:** `property.unused`, `method.unused`, `classConstant.unused`
**Messages:**
- `Property Foo::$bar is unused.`
- `Method Foo::bar() is unused.`
- `Constant Foo::BAR is unused.`

Find and delete the entire member declaration. Destructive action, so mark
as non-preferred and only offer when the identifier exactly matches
(not on `property.onlyRead`, `property.onlyWritten`, etc. where the member
is partially used).

For methods, need to find the full extent (from docblock through closing `}`).
For properties/constants, just the line (plus docblock above).

**Stale detection:** the member name no longer appears as a declaration in the
class.

---

### H20. `generics.callSiteVarianceRedundant` — Remove redundant variance annotation

**Identifier:** `generics.callSiteVarianceRedundant`
**Tip (in message):** `You can safely remove the call-site variance annotation.`

Strip `covariant` or `contravariant` keywords from generic type arguments
in the docblock. Requires parsing PHPDoc generic syntax
(e.g. `Collection<covariant Foo>` becomes `Collection<Foo>`).

**Stale detection:** no `covariant`/`contravariant` in the PHPDoc on the
diagnostic line.

---

### H21. `return.void` — Remove return value from void function

**Identifier:** `return.void`
**Message:** `{desc} with return type void returns {type} but should not return anything.`

Replace `return {expr};` with `return;` on the diagnostic line.

**Stale detection:** the diagnostic line contains `return;` (no expression).

---

### H22. `return.empty` — Add return value or change return type to void

**Identifier:** `return.empty`
**Message:** `{desc} should return {type} but empty return statement found.`

Offer two quickfixes:
1. **Change return type to `void`** — replace the native return type and
   remove any `@return` tag.
2. **Add placeholder return** — `return null;` — only valid if `{type}`
   includes `null`.

---

### H23. `instanceof.alwaysTrue` — Remove redundant instanceof check

**Identifier:** `instanceof.alwaysTrue`
**Message:** `Instanceof between {type} and {class} will always evaluate to true.`

Offer to simplify: remove the `instanceof` check and keep only the truthy
branch. Complex because it requires understanding the control flow (if/else,
ternary, match arm). Consider deferring this indefinitely — the user can just
`@phpstan-ignore` it.

---

### H24. `catch.neverThrown` — Remove unnecessary catch clause

**Identifier:** `catch.neverThrown`
**Message:** `Dead catch - {exception} is never thrown in the try block.`

Remove the catch clause for the exception that is never thrown. If it is the
only catch clause, the entire try/catch block should be unwrapped (keep just
the try body). This requires careful brace matching.

Start with the multi-catch case: if the catch has multiple exception types
(`catch (FooException | BarException $e)`), just remove the dead type from
the list.

---

## Suggested implementation order

Based on effort-to-value ratio and shared infrastructure:

1. **R1, R2, R3** — prerequisites (small, unblocks everything)
2. **H7, H8, H9** — PHPDoc type mismatch family (implement together)
3. **H11** — visibility fix (leverages existing `change_visibility.rs`)
4. **H14** — narrow `@throws` (extends existing `remove_throws.rs`)
5. **H6** — return type update
6. **H10** — remove unused union member
7. **H12** — prefixed class name
8. **H4** — unset by-ref foreach variable
9. Everything else based on user demand

---

## Implementation notes

### Message parsing

All message parsing should use regex with named capture groups for clarity.
Create a shared helper module (e.g. `code_actions/phpstan_message.rs`) for
common patterns like extracting class names, method names, types, and property
names from PHPStan messages. Example:

```rust
use regex::Regex;

/// Extract the "actual" type from a return.type diagnostic message.
pub fn extract_return_type_actual(message: &str) -> Option<&str> {
    let re = Regex::new(r"should return .+ but returns (?P<actual>.+)\.$").ok()?;
    re.captures(message)?.name("actual").map(|m| m.as_str())
}
```

### Tip extraction

Tips are appended to `Diagnostic.message` after a `\n` by
`parse_phpstan_message()` in `phpstan.rs`. To access the tip:

```rust
let (message, tip) = match diag.message.split_once('\n') {
    Some((m, t)) => (m, Some(t)),
    None => (diag.message.as_str(), None),
};
```

Actions that depend on tip text (H4, H12, H14, H15, H20) should use this
pattern. The tip text has ANSI/HTML tags already stripped by `strip_ansi_tags`.

### Stale diagnostic detection

Each new action should have a corresponding check in
`is_stale_phpstan_diagnostic()` in `diagnostics/mod.rs` so that the diagnostic
is eagerly cleared after the user applies the fix, without waiting for the
next PHPStan run.

The function currently handles:
- `@phpstan-ignore` coverage (all identifiers)
- `throws.unusedType` / `throws.notThrowable` — tag removed
- `missingType.checkedException` — tag added

New actions should add branches to the `match identifier { ... }` block.

### Testing

Each action needs tests following the existing pattern:
- Unit tests for pure helper functions (regex extraction, edit building)
- Integration tests that construct `CodeActionParams` with mock diagnostics
  and call `collect_*_actions` directly
- Stale detection tests that construct `Diagnostic` objects and call
  `is_stale_phpstan_diagnostic`

### Attribute insertion pattern (H3, H5) — Implemented

H3 (`remove_override.rs`) and H5 (`add_return_type_will_change.rs`) are now
implemented. Each module contains its own `find_method_insertion_point` and
attribute detection helpers, following the same pattern as `add_override.rs`.
Future attribute-related actions can reference any of these three modules.

### PHPDoc type mismatch pattern (H7, H8, H9)

These three actions share the same structure:
1. Parse `{phpdoc}` type and `{native}` type from the message
2. Offer to update the PHPDoc tag to match native
3. Offer to remove the PHPDoc tag entirely

Implement a generic helper parameterised by tag name (`@return`, `@param`,
`@var`) and the tag's identifying context (e.g. `$paramName` for `@param`).