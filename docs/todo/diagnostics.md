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

## D1. Scalar member access diagnostic — remaining gaps

**Impact: High · Effort: Low**

Member access on a scalar type is always a runtime error. The
`scalar_member_access` diagnostic (Error severity) is implemented for
bare variables, property chains, and call expression returns. Duplicate
suppression (dropping the `unresolved_member_access` hint when a more
specific diagnostic overlaps) is also done.

### Remaining gaps

| Scenario                   | Current | Expected                                                |
| -------------------------- | ------- | ------------------------------------------------------- |
| `$user->getName()->trim()` | Silent  | **Error**: Cannot access method 'trim' on type 'string' |
| `$user->getAge()->value`   | Silent  | **Error**: Cannot access property 'value' on type 'int' |

These are method-return-chain subjects where the return type is scalar.
The `resolve_scalar_subject_type` function handles `CallExpr` callee
types for standalone functions and static methods, but method call
chains where the intermediate callee is itself a call expression are
not yet covered.

---

## D2. Chain error propagation (flag only the first broken link)

**Impact: Medium · Effort: Medium**

In a fluent chain like `$m->callHome()->callMom()->callDad()`, only the
first broken link should be flagged. Subsequent links in the chain
depend on the return type of the broken call, so flagging them adds
noise without actionable information.

### Current state

- Fluent chains on `mixed` subjects: only the first link is flagged
  (the chain members have no `MemberAccess` spans because the parser
  can't resolve the subject). This works by accident.
- Fluent chains on known types where the first method is unknown:
  only the first is flagged (same reason, parser stops). Also works
  by accident.
- Scalar chains (`$user->getAge()->value->deep`): the scalar member
  access at `->value` should be flagged but `->deep` should be silent.
  Currently `->value` is not flagged at all (see D1).

### Desired behavior

- `$m->callHome()->callMom()->callDad()` — flag only `callHome`.
- `$m->callHome(); $m->callMom();` — flag both (separate statements).
- `$user->fakeMethod()->next()->deep()` — flag only `fakeMethod`.
- `$user->getAge()->value->deep` — flag only `->value` (scalar error).

### Cross-assignment propagation (nice to have)

```
$home = $m->callHome();  // flagged
$home->callMom();        // ideally silent
```

This is harder because it requires tracking that `$home` was assigned
from an already-flagged expression. Acceptable if not implemented in
the first pass.

---

## D3. Deprecated rendering

**Impact: Low-Medium · Effort: Low**

Deprecated class references (e.g. `new OldHelper()`) currently show a
hint message but the `DiagnosticTag::DEPRECATED` tag (which renders as
strikethrough in most editors) may not be applied correctly for all
deprecated symbol types. Verify that:

- Deprecated class references in `new`, type hints, `extends`, and
  `implements` positions all render with strikethrough.
- Deprecated method calls render with strikethrough.
- Deprecated property accesses render with strikethrough.
- The deprecated diagnostic resolver uses offset-based class resolution
  (not "first class in file") for `$this`/`self`/`static` subjects, so
  that files with multiple classes resolve correctly.
- Chain subjects (`getHelper()->deprecatedMethod()`) resolve through
  the full completion pipeline, not the hand-rolled
  `resolve_subject_to_class_name` helper that can't handle chains.

---

## D4. Unresolved type in PHPDoc

**Impact: Medium · Effort: Medium**

A `@return`, `@param`, `@var`, `@throws`, `@mixin`, or `@extends` tag
references a class that cannot be resolved. This is advisory (the code
may still work if the type is only used for static analysis), so it
should be **Information** severity.

| Scenario                                                                    | Expected                                                        |
| --------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `@return SomeAlias` where SomeAlias is not a class, type alias, or template | **Info**: Type 'SomeAlias' in @return could not be resolved     |
| `@param NonExistent $x`                                                     | **Info**: Type 'NonExistent' in @param could not be resolved    |
| `@throws FakeException`                                                     | **Info**: Type 'FakeException' in @throws could not be resolved |

### Implementation notes

This partially overlaps with `unknown_classes.rs` which already flags
`ClassReference` spans in docblock type positions. The remaining gap is
PHPDoc tags that reference types which are not emitted as
`ClassReference` spans by the symbol map. Audit which docblock type
positions produce `ClassReference` spans and which don't.

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

- Psalm: `/** @psalm-suppress [IssueType] */` on the line or above
  the function/class.
- PHPCS: `// phpcs:ignore [Sniff.Name]` or `// phpcs:disable` /
  `// phpcs:enable` blocks.
- PHPMD: `// @SuppressWarnings(PHPMD.[RuleName])` in a docblock.
- For PHPantom's own diagnostics: support `@suppress PHPxxxx`
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

## D7. Implementation error diagnostic

**Impact: Medium · Effort: Medium**

Flag concrete classes that fail to implement all required methods from
their interfaces or abstract parents. PHPantom already has the
"implement missing methods" code action that detects this condition.
Surfacing it as a diagnostic makes the problem visible without the
user needing to trigger quick-fix.

### Behaviour

| Scenario                                                | Severity | Message                                                                     |
| ------------------------------------------------------- | -------- | --------------------------------------------------------------------------- |
| Class implements interface but misses a method          | Error    | Class 'Foo' must implement method 'bar()' from interface 'Baz'              |
| Class extends abstract class but misses abstract method | Error    | Class 'Foo' must implement abstract method 'bar()' from class 'AbstractBaz' |
| Non-abstract class has abstract method                  | Error    | Non-abstract class 'Foo' contains abstract method 'bar()'                   |

### What we already have

The `collect_implement_methods_actions` function in
`code_actions/implement_methods.rs` already:

1. Loads the class and its full inheritance chain.
2. Collects all abstract methods from interfaces and abstract parents.
3. Checks which ones are missing from the concrete class.

The diagnostic can reuse this exact logic, just emitting a diagnostic
instead of (or in addition to) offering a code action.

### Implementation

1. In the Phase 2 diagnostic collector, for each `ClassDeclaration`
   span in the symbol map, load the class via `find_or_load_class`.
2. Skip abstract classes, interfaces, traits, and enums.
3. Run the same missing-method detection as the code action.
4. Emit an Error-severity diagnostic on the class name span for each
   missing method. Group into a single diagnostic with all missing
   method names listed if there are multiple.
5. Pair the diagnostic with the existing "implement missing methods"
   code action so the quick-fix button appears inline.

### Debugging value

When our inheritance resolution misses a parent class or trait, the
implementation error diagnostic fires unexpectedly (flagging methods
that are actually implemented via a trait that we failed to resolve).
This makes inheritance resolution bugs immediately visible.

---

## D8. Undefined variable diagnostic

**Impact: High · Effort: Medium**

Flag reads of variables that have no prior assignment or definition in
the reachable scope. This is the single most impactful diagnostic
PHPantom is missing compared to Intelephense (P1008). Every PHP
developer has been bitten by a variable name typo.

### Behaviour

| Scenario                                      | Severity | Message                                                          |
| --------------------------------------------- | -------- | ---------------------------------------------------------------- |
| `echo $nmae;` where only `$name` was assigned | Error    | Undefined variable '$nmae'                                       |
| `$x = $y + 1;` where `$y` was never assigned  | Error    | Undefined variable '$y'                                          |
| Variable used only inside one branch of an if | Error    | Undefined variable '$result' (if not assigned on the path taken) |

Severity is **Error** because accessing an undefined variable is a
runtime notice/warning (and `ErrorException` in strict setups).

### What counts as a definition

A variable is considered defined if any of the following occur before
the read, in the same or an enclosing scope:

- Direct assignment: `$x = ...;`, `$x += ...;`, `$x[] = ...;`
- Parameter: `function foo($x)`
- `foreach` binding: `foreach ($items as $key => $value)`
- `for` initialiser: `for ($i = 0; ...)`
- `catch` variable: `catch (Exception $e)`
- `list()` / `[...]` destructuring on the left-hand side of `=`
- `global $x;` statement
- `static $x;` statement
- Closure `use ($x)` clause
- Match arm binding (if PHP adds it in the future)
- `$this` inside a non-static method or closure bound to `$this`

### Superglobals (always defined)

`$_GET`, `$_POST`, `$_SERVER`, `$_REQUEST`, `$_SESSION`, `$_COOKIE`,
`$_FILES`, `$_ENV`, `$GLOBALS`, `$argc`, `$argv`, `$http_response_header`,
`$php_errormsg`.

### Suppression / false-positive avoidance

The following patterns should suppress the diagnostic for a variable:

- **`isset($var)` or `empty($var)`** — the variable is being guarded,
  not used.
- **`compact('var')`** — `$var` is referenced by string name.
- **`extract($array)`** — any variable could be defined; suppress all
  undefined-variable diagnostics in the function after an `extract()`
  call.
- **`$$dynamic`** — variable variables make static analysis unsound;
  suppress diagnostics in functions that use variable variables.
- **`@$var`** — the error suppression operator signals intentional use
  of a potentially undefined variable.
- **`unset($var)`** — marks the variable as undefined from that point
  forward, but `unset()` itself should not be flagged.
- **PHPDoc `@var Type $var`** on the preceding line — the developer is
  asserting the variable exists.

### Scope rules

- Function/method bodies are independent scopes. A variable assigned
  in one function is not visible in another.
- Closures capture variables only via `use ()` or `$this`.
- Arrow functions (`fn() =>`) capture all variables from the enclosing
  scope implicitly.
- Variables assigned inside an `if`/`else`/`while`/`for`/`switch`
  branch are potentially defined (Phase 1 can treat any assignment
  anywhere in the function as a definition; Phase 2 can do proper
  branch analysis).
- Global scope (outside any function) is a single scope.

### Implementation

Phase 1 (conservative, low false-positive rate):

1. Walk each function/method/closure body.
2. Collect all variable definitions (assignments, parameters, foreach,
   catch, list, global, static, use clause).
3. For each variable read (`$var` in an expression), check whether the
   name appears in the definitions set.
4. If not, and the name is not a superglobal, emit the diagnostic.
5. If the function contains `extract()` or `$$dynamic`, skip the
   entire function.

This is deliberately simple: it treats any assignment anywhere in the
function as sufficient, regardless of control flow. This avoids false
positives from branch-dependent definitions at the cost of missing
some genuinely undefined variables that are only assigned in one
branch. This is the same approach Intelephense takes.

Phase 2 (future, optional):

- Track definitions per control-flow branch.
- Flag variables that are defined in some branches but not all before
  the read point.
- This is significantly more complex and can be deferred.

### Existing infrastructure

The variable resolution code in `completion/variable/resolution.rs`
already walks AST scopes and tracks variable assignments for type
inference. The undefined-variable diagnostic needs the same scope
walking but with a simpler question: "was this name ever assigned?"
rather than "what type is it?". Much of the traversal logic can be
shared or adapted.

---

## D9. Syntax error diagnostic

**Impact: High · Effort: Low**

Surface parse errors from the Mago parser as LSP diagnostics. This is
the most fundamental diagnostic any language server can provide. Without
it, a user with a typo like `function { broken` gets no feedback until
they try to run the code.

### Behaviour

| Scenario                                            | Severity | Message                          |
| --------------------------------------------------- | -------- | -------------------------------- |
| `function { broken`                                 | Error    | Expected `(`, found `{`          |
| `$x = ;`                                            | Error    | Unexpected token `;`             |
| `echo "unclosed string`                             | Error    | Unclosed double-quoted string    |
| Deeply nested expressions exceeding recursion limit | Error    | Maximum recursion depth exceeded |

Severity is always **Error**. Syntax errors prevent execution.

### What Mago already provides

The `Program` struct returned by `mago_syntax::parser::parse_file_content`
has an `errors: Vec<ParseError>` field. Each `ParseError` variant
carries a `Span` (with byte offsets) and implements `Display` with
human-readable messages:

- `ParseError::SyntaxError(SyntaxError)` — lexer-level errors
  (unexpected token byte, unrecognised token byte, unexpected EOF).
- `ParseError::UnexpectedToken(expected, found, span)` — parser
  expected one of `expected` token kinds, found `found`.
- `ParseError::UnexpectedEndOfFile(expected, file_id, position)` —
  parser reached EOF while expecting one of `expected`.
- `ParseError::UnclosedLiteralString(kind, span)` — unclosed single-
  or double-quoted string.
- `ParseError::RecursionLimitExceeded(span)` — parser hit its
  recursion depth limit.

All variants implement `HasSpan`, so converting to LSP `Range` is
straightforward.

### Implementation

1. In the Phase 1 diagnostic collector (the fast, no-resolution
   phase), parse the file with `parse_file_content`.
2. Iterate `program.errors`.
3. For each error, convert the `Span` to an LSP `Range` using the
   existing byte-offset-to-position helpers.
4. Emit an Error-severity diagnostic with the `Display` output as the
   message.

This can share the parse call that `update_ast` already performs. If
the AST update succeeded (no panic), the `Program` is available and
its `errors` field can be read directly. If the parse panicked
(`catch_unwind` triggered), emit a single diagnostic on line 0
saying "Parse failed (internal error)".

### Phase 1 placement

Syntax errors should be the very first diagnostics emitted, before
any other Phase 1 checks. They appear instantly on every keystroke.
When syntax errors are present, Phase 2 diagnostics (which require
type resolution) can optionally be skipped entirely since the AST is
likely incomplete, though this is not required for correctness since
the downstream diagnostics will simply produce fewer results on a
partial AST.

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
