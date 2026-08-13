# PHPantom — Diagnostics

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## Severity philosophy

PHPantom assigns diagnostic severity based on runtime consequences:

| Severity        | Criteria                                                                                                                                                                                                                                                                                                                                                                                     | Examples                                                                                                                                                                                                                                                                      |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Error**       | Would crash at runtime. The code is definitively wrong.                                                                                                                                                                                                                                                                                                                                      | Member access on a scalar type (`$int->foo()`). Calling a function that doesn't exist (`doesntExist()`).                                                                                                                                                                      |
| **Warning**     | Likely wrong but could work for reasons we can't verify statically. The types are poor but the code might be correct at runtime.                                                                                                                                                                                                                                                             | Accessing a member that doesn't exist on a non-final class (`$user->grantAccess()` where `User` has no such method but a subclass might). Unknown class in a type position (`Class 'Foo' not found`). Subject type resolved to an unknown class so members can't be verified. |
| **Hint**        | The codebase lacks type information. Off by default or very subtle. Poorly typed PHP is so common that showing these by default would be noise for most users. Anyone who does care about type safety is likely running PHPStan already. Unless our engine becomes very strong, these diagnostics either expose our own inference gaps or bother users who never opted into static analysis. | `mixed` subject member access (opt-in via `unresolved-member-access`). Deprecated symbol usage (rendered as strikethrough).                                                                                                                                                   |
| **Information** | Advisory. Something the developer might want to know.                                                                                                                                                                                                                                                                                                                                        | Unused `use` import (rendered as dimmed). Unresolved type in a PHPDoc tag.                                                                                                                                                                                                    |

---

## D5. External tool diagnostic suppression actions

**Impact: Low · Effort: Low (per tool, after proxy exists)**

PHPantom's own inline suppression (`// @phpantom-ignore code`) has
shipped. PHPStan suppression is also implemented ("Ignore PHPStan
error" / "Remove unnecessary @phpstan-ignore"). The PHPCS proxy itself
has also shipped (`src/diagnostics/external/phpcs.rs`, `[phpcs]` config
section), but nothing wires up a suppression action for it yet. What
remains is wiring up suppression actions for additional external tool
proxies:

- PHPCS: `// phpcs:ignore [Sniff.Name]` or `// phpcs:disable` /
  `// phpcs:enable` blocks. The proxy exists; only the suppression
  action is missing.
- PHPMD (3.0): `#[SuppressWarnings(RuleName::class)]` as a PHP
  attribute. Blocked on the proxy itself (D10).

---

## D6. Unreachable code diagnostic

**Impact: Low-Medium · Effort: Low**

Dim code that appears after unconditional control flow exits:
`return`, `throw`, `exit`, `die`, `continue`, `break`. This is a
Phase 1 (fast) diagnostic since it requires only AST structure, not
type resolution.

### Behaviour

| Scenario                                           | Rendering                           |
| -------------------------------------------------- | ----------------------------------- |
| Code after `return $x;` in same block              | Dimmed (DiagnosticTag::UNNECESSARY) |
| Code after `throw new \Exception()`                | Dimmed                              |
| Code after `exit(1)` or `die()`                    | Dimmed                              |
| Code after `continue` or `break` in a loop         | Dimmed                              |
| Code after `if (...) { return; } else { return; }` | Dimmed (both branches exit)         |

Severity: **Hint** with `DiagnosticTag::UNNECESSARY` so editors dim
the text rather than underlining it. This matches how unused imports
are rendered.

### Implementation

Walk the AST statement list. After encountering a statement that
unconditionally exits the current scope (return, throw, expression
statement containing `exit`/`die`), mark all subsequent statements in
the same block as unreachable. The span covers from the start of the
first unreachable statement to the end of the last statement in the
block.

Phase 1 only handles the simple single-block case. Whole-branch
analysis (both if/else branches exit) is a future refinement.

### Debugging value

When our type engine silently resolves a method to a `never` return
type (e.g. an incorrectly resolved overload), unreachable code after
the call becomes visible, signalling the bug.

---

## D10. PHPMD diagnostic proxy

**Impact: Low · Effort: Medium**

Proxy PHPMD (PHP Mess Detector) diagnostics into the editor, following
the same pattern as the existing PHPStan proxy. PHPMD 3.0 (once
released) is the target version. It will get a `[phpmd]` TOML section
with `command`, `timeout`, and tool-specific options mirroring the
`[phpstan]` schema.

### Prerequisites

- PHPMD 3.0 must be released. Current 2.x output formats and rule
  naming may change.
- The diagnostic suppression code action (D5) can add PHPMD's
  `@SuppressWarnings(PHPMD.[RuleName])` syntax once the proxy exists.

### Implementation

1. Add a `[phpmd]` section to the config schema in `src/config.rs`
   with `command` (default `"vendor/bin/phpmd"`), `timeout`, and
   an `enabled` flag.
2. Run PHPMD with XML or JSON output on the current file (or changed
   files) and parse the results into LSP diagnostics.
3. Map PHPMD rule names to diagnostic codes so that suppression
   actions (D5) can insert the correct `@SuppressWarnings` annotation.
4. Respect the same debounce and queueing logic used by the PHPStan
   proxy to avoid overwhelming the tool on rapid edits.

---

## D14. Tighten argument type mismatch diagnostic (Phase 2)

**Impact: Medium · Effort: Low**

`is_type_compatible` in `src/diagnostics/type_errors/compatibility.rs`
still silences two cases that are genuine bugs at runtime. Two other
gaps this item used to track — the any-member union threshold and the
reverse-hierarchy (supertype-to-subtype) acceptance — have since been
tightened (see "A partially-compatible union argument is now reported"
and the class-hierarchy comment noting the downcast direction "is now
reported" in the changelog).

### 1. Nullable arg → non-nullable param

Currently silenced with a MAYBE comment ("developer may have guarded
against null"). This is the #1 source of runtime `TypeError` in
PHP 8+. Both PHPStan and Psalm flag it. Should be reported at least
as **Warning** severity, since the null path may be unguarded.

### 2. `void` as argument

Currently silenced conservatively. Passing the return value of a
`void` function is always a bug — PHP 8 returns `null` but the call
site clearly misunderstands the API. Should be **Error** severity.

---

## D15. Unused parameter diagnostic

**Impact: Low · Effort: Low**

Flag function and method parameters that are never read inside the
body. This was intentionally excluded from D4 (unused variable
diagnostic) because false positives are common for callbacks, interface
implementations, and framework conventions (e.g. Laravel event
listeners) that require specific parameter signatures even when not
all parameters are used. Users can now silence false positives with
`// @phpantom-ignore unused_parameter`.

### Scope

1. Function and method parameters (including closures and arrow
   functions) that are never read inside their body.
2. Constructor parameters that are not promoted and never read.

### Exclusions

- Parameters named `$_` or starting with `$_` (intentional discard).
- Promoted constructor parameters (they are property assignments).
- Parameters in abstract methods and interface method signatures
  (no body to check).

---

## D16. `unreachable_match_arm` ignores literal subject types

**Impact: Low-Medium · Effort: Low**

`scalar_type_label` in `src/diagnostics/match_type_errors.rs` answers
`None` for a literal type (`'exception'`, `42`), so a subject the
resolver typed as one exact value never reaches the arm check and no
arm is ever reported unreachable. The comment there explains why: a
literal was as often what survived after resolution lost an
alternative it could not type as it was a genuine one-value subject,
and taking the claim without that evidence produced false positives.

The resolver no longer loses those alternatives. An unresolvable
branch now widens the union it belongs to instead of dropping out of
it, so a literal that reaches this diagnostic is a claim the resolver
stands behind.

**Fix:** Give `TypeKind::Literal` its scalar kind in
`scalar_type_label` and check the resulting arms. Cover the case the
old comment was guarding against with a test: a subject whose other
branch cannot be typed must still produce no diagnostic, because it
now resolves to `mixed` rather than to the surviving literal.



---

## D17. `docblock_native_mismatch` only judges nullability

**Impact: Low · Effort: Medium**

```php
/** @param int $name */
function greet(?string $name): void {}   // not flagged: int is not string at all

/** @param Foo $value */
function take(?Foo $value): void {}      // not flagged: `Foo` may be nullable
```

`src/diagnostics/docblock_native_mismatch.rs` compares a documented type
against its native hint on one axis only: whether the annotation denies a
`null` the signature accepts. Two shapes are therefore still silent.

The first is a documented type that is not a subtype of the native hint at
all (`@param int` on a `?string`, `@return array` on a `: string`). That is
the check PHPStan's `IncompatiblePhpDocTypeRule` performs, and the one the
existing `is_type_compatible` in `src/diagnostics/type_errors/compatibility.rs`
already has the machinery for.

The second is a bare class-like name, which `nullability_is_decidable` steps
around on purpose: `Foo` may be a `@template` parameter or an imported
`@psalm-type` alias that resolves to a nullable type, and the diagnostic has
no resolution step that would tell those apart from a class named `Foo`.
Resolving the name (against the declaration's own `@template` list, the
enclosing class's, and the file's `@psalm-type`/`@psalm-import-type` tags)
would let the nullability check cover the class-name case as well.

**Fix:** Resolve the documented type's names before comparing, then run the
comparison through `is_type_compatible` rather than the nullability test
alone. Both halves want the same measurement, so they are one change rather
than two.
