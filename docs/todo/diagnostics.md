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

## D4. Unused variable diagnostic

**Impact: Medium · Effort: Medium**

Flag variables that are assigned but never read. This is one of the
most common issues in PHP codebases and catches dead code, typos in
variable names, and forgotten refactoring leftovers.

PHPantom already has an undefined-variable diagnostic
(`undefined_variable` in `diagnostics/undefined_variables.rs`) that
tracks variable definitions and reads through scope analysis. The
unused-variable diagnostic is the dual: a variable that has a
definition site but zero read sites within the same scope.

**Severity:** Information (rendered as dimmed text). Assigned-but-
unread variables are not bugs per se (the code still runs), but they
are strong signals of dead code or typos. Information severity avoids
alarming users while still making the issue visible.

**Diagnostic code:** `unused_variable` (matches the planned CLI fix
rule FX2).

### Scope

1. **Local variables in function/method bodies.** A variable assigned
   inside a function or method body that is never read before the
   scope ends. Parameters count as assignments; an unused parameter
   in a non-abstract, non-interface method is flagged.
2. **Foreach bindings.** `foreach ($items as $key => $value)` where
   `$key` or `$value` is never read inside the loop body. Convention:
   variables named `$_` or starting with `$_` are exempt (intentional
   discard).
3. **Catch variables.** `catch (Exception $e)` where `$e` is never
   read. Same `$_` exemption applies.

### Exclusions

- Variables in the global scope (scripts, templates).
- Variables passed by reference (`&$var`) to functions, since the
  callee may use them as out-parameters.
- Variables used inside closures or arrow functions that capture them
  (explicit `use ($var)` or implicit capture).
- Compact() calls that reference the variable by string name.
- Variables used in string interpolation (`"Hello $name"`).
- Variables whose RHS has side effects (method calls, function calls)
  should still be flagged, but with a detail note that removing the
  assignment would also remove the side effect.

### PHPStan parallel

PHPStan does not have a built-in unused-variable rule, but third-party
rulesets (e.g. `phpstan-strict-rules`, `tomasvotruba/unused-public`)
report similar issues. When D4 ships, the PHPStan quick-fix
infrastructure should recognise our native `unused_variable` code so
that:

- The "Remove unused import" pattern can be extended to offer a
  "Remove unused variable" quick-fix (same code action kind).
- FX2 (`unused_variable` CLI fix rule) can consume our diagnostic
  directly without needing PHPStan.

### Implementation

1. Extend the scope collector (`scope_collector/mod.rs`) to track
   read sites per variable per frame (it already tracks definition
   sites for the undefined-variable diagnostic).
2. After processing a scope, iterate defined variables and flag any
   that have zero reads and are not in the exclusion list.
3. Emit diagnostics with `DiagnosticSeverity::HINT` and
   `DiagnosticTag::UNNECESSARY` so editors render unused variables
   as dimmed/faded text.
4. Add a code action (in `code_actions/`) to remove the assignment
   statement when the RHS is side-effect-free, or to prefix the
   variable with `$_` to suppress the diagnostic.

---

## D3. Deprecated rendering — chain subject resolution

**Impact: Low-Medium · Effort: Medium**

Chain subjects like `getHelper()->deprecatedMethod()` do not produce
a deprecated diagnostic because `resolve_subject_to_class_name` in
`diagnostics/deprecated.rs` returns `None` for non-variable,
non-keyword subjects (the `_ => None` arm). The function call return
type is never resolved, so the member deprecation check is skipped.

**Fix:** Route chain subjects through the completion/type-inference
pipeline to resolve the return type of the call before checking the
member for deprecation. The variable-resolution path already works
for `$var->deprecatedMethod()` via `resolve_variable_subject`; the
gap is function-call and method-call return types in subject position.

The following have been verified and are covered by tests:

- Deprecated class references in `new`, type hints, `extends`, and
  `implements` positions all render with strikethrough.
- Deprecated method calls, property accesses, and constants render
  with strikethrough (via both `$var->` and `ClassName::` subjects).
- Offset-based class resolution for `$this`/`self`/`static` resolves
  to the correct class in files with multiple class declarations.

---

## D5. Diagnostic suppression intelligence

**Impact: Medium · Effort: Medium**

When PHPantom proxies diagnostics from external tools, users need a way
to suppress specific warnings. Rather than forcing them to install a
separate extension or memorise each tool's suppression syntax, PHPantom
can offer **code actions to insert the correct suppression comment** for
the tool that produced the diagnostic.

PHPStan suppression is implemented: "Ignore PHPStan error" adds
`// @phpstan-ignore <identifier>` (appending to existing ignores when
present), and "Remove unnecessary @phpstan-ignore" cleans up unmatched
ignores reported by PHPStan. What remains:

### Remaining tools

- PHPCS: `// phpcs:ignore [Sniff.Name]` or `// phpcs:disable` /
  `// phpcs:enable` blocks.
- PHPMD (3.0): `#[SuppressWarnings(RuleName::class)]` as a PHP attribute.
- For PHPantom's own diagnostics: support `@suppress phpantom.*`
  in docblocks (matching PHP Tools' convention) and a config flag
  `phpantom.diagnostics.enabled: bool` (default `true`).

**Prerequisites:** Each tool needs a diagnostic proxy before its
suppression actions can be wired up.

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
- The diagnostic suppression code action (D5) should support PHPMD's
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

## D12. Mago linter integration (optional diagnostics)

**Impact: Medium · Effort: Medium**

PHPantom already depends on several mago crates (`mago-syntax`,
`mago-docblock`, `mago-names`, `mago-formatter`, `mago-span`). The
`mago-linter` crate provides ~159 lint rules covering redundancy,
best practices, clarity, consistency, correctness, and deprecation.
Integrating it as an optional diagnostics provider would give users
"PHPStan-lite" diagnostics without requiring PHPStan to be installed.

**Integration approach:** call `Linter::lint()` on the parsed AST
(already available), convert `IssueCollection` to LSP `Diagnostic`s,
convert `TextEdit` fixes to LSP `CodeAction`s. The linter is AST-only
(no type inference), so it is fast.

Offer as opt-in via `.phpantom.toml` configuration. Default to
disabled so it does not conflict with users who already run PHPStan
or Psalm. Mark with `source: "mago"` to distinguish from PHPantom's
own diagnostics.

**Notable rules:** `no-redundant-method-override`,
`str-contains`/`str-starts-with` modernization,
`prefer-arrow-function`, `constant-condition`, `no-self-assignment`,
`explicit-nullable-param`, `valid-docblock`.

## D13. Unify diagnostic subject resolution with completion/hover

`unknown_members.rs` has two secondary resolvers that run their own
independent type resolution when `resolve_target_classes_expr` returns
empty:

- `resolve_scalar_subject_type` (~130 lines) re-resolves variables,
  property chains, and call expressions to detect scalar types.
- `resolve_unresolvable_class_subject` (~80 lines) re-resolves
  variables and call expressions to detect class names that can't be
  loaded.

Both duplicate logic from `resolver.rs` and
`variable/resolution.rs` but can diverge, producing diagnostics for
types that completion and hover cannot see (or vice versa).

### Goal

The diagnostic path should use the same resolution result that
completion and hover use. All three consumers should see identical
outcomes for the same subject text at the same cursor position.

### Approach

Extend the shared resolver's return type (or add a secondary result)
to carry scalar type information and unresolvable class names
alongside the resolved `ClassInfo` list. The diagnostic collector
would then inspect this enriched result instead of running its own
resolution. This eliminates the secondary resolvers entirely.

### Files

- `src/diagnostics/unknown_members.rs` — remove
  `resolve_scalar_subject_type` and `resolve_unresolvable_class_subject`
- `src/completion/resolver.rs` — enrich the resolution result


