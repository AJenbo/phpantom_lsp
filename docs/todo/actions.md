# PHPantom — Code Actions

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

**Refactoring code actions overview:** A11 (ScopeCollector) is the
shared infrastructure that A2 (Extract Function), A4 (Inline Variable),
A5 (Extract Variable), A6 (Inline Function/Method), and A7 (Extract
Constant) all depend on. It provides forward-pass variable usage
tracking with byte offsets across function scopes.

## A1. Simplify with null coalescing / null-safe operator
**Impact: Medium · Effort: Medium**

Offer code actions to simplify common nullable patterns:

- `isset($x) ? $x : $default` → `$x ?? $default`
- `$x !== null ? $x : $default` → `$x ?? $default`
- `$x === null ? $default : $x` → `$x ?? $default`
- `$x !== null ? $x->foo() : null` → `$x?->foo()`
- `if ($x !== null) { return $x->foo(); } return null;` → `return $x?->foo();`

### Implementation

- Register as code actions with kind `quickfix` or `refactor.rewrite`.
- Pattern-match on ternary expressions and simple if-null-return blocks
  in the AST. The conditions are structural — no type resolution needed
  for the basic patterns (just checking for `=== null` / `!== null` /
  `isset()`).
- Generate replacement text preserving the original variable/expression
  names.
- Only offer `?->` suggestions when the project targets PHP 8.0+
  (check `self.php_version()`).

**Scope:** Start with ternary expressions (simplest AST match). The
if-statement patterns are a follow-up.

---

## A11. ScopeCollector infrastructure
**Impact: Medium · Effort: Medium**

A lightweight forward-pass AST walker that collects every variable
read and write with byte offsets across a function/method/closure body.
This is shared infrastructure used by A2 (Extract Function), A4 (Inline
Variable), A5 (Extract Variable), A6 (Inline Function/Method), A7
(Extract Constant), and D8 (undefined variable diagnostic). It could
also serve document highlights (`textDocument/documentHighlight`) since
it produces "all occurrences of variable X in this scope" as a natural
byproduct.

### Design

Today our variable resolution (`completion/variable/resolution.rs`)
walks backward from the cursor to find assignments, which is sufficient
for completion but not for refactoring. The refactoring toolkit needs a
**forward** pass that tracks _all_ variable definitions and usages
across the enclosing function body, not just the ones that lead to the
cursor.

Phpactor solves this with a "frame" model — a stack of scopes where each
scope records its own local variable assignments with byte offsets. The
key ideas worth borrowing:

- **Frame = scope boundary.** Each function body, closure, arrow
  function, and `catch` block opens a new frame. Variables defined inside
  a frame are local to it (closures capture via `use()`, arrow functions
  capture by value). A `foreach`, `if`, or `for` block does _not_ open a
  new frame in PHP — variables leak into the enclosing scope.

- **Assignment list with offsets.** Each frame stores a flat list of
  `(variable_name, byte_offset, type)` entries. Walking the AST in
  source order and recording every `$var = …`, parameter declaration,
  `foreach ($x as $k => $v)`, and `catch (E $e)` populates this.

- **Read set / write set per range.** Given a range `[start, end)`:
  - **Parameters** = variables _read_ inside `[start, end)` whose most
    recent assignment is _before_ `start`.
  - **Return values** = variables _written_ inside `[start, end)` that
    are _read after_ `end` in the enclosing scope.
  - **Locals** = variables whose entire lifetime (first write to last
    read) is contained within `[start, end)` — these stay inside the
    extracted function and do not become parameters or return values.

- **`$this` handling.** Track whether `$this`, `self::`, or `static::`
  appears in a given range.

- **Reference parameters (`&$var`).** Track by-reference writes so
  callers can decide whether to generate `&$param` or a return tuple.

We do _not_ need Phpactor's full per-expression-node resolver system for
this. Our existing variable resolution + type narrowing infrastructure
can resolve the type of each variable at extraction boundaries. The new
piece is the forward walk that collects the read/write sets.

### Implementation

Build a `ScopeCollector` struct that walks the enclosing function's AST
once and produces a `ScopeMap` containing:

- Every variable read and write with `(name, byte_offset, kind)` where
  kind is `Read` or `Write`.
- Frame boundaries (function, closure, arrow function, catch) so
  consumers can determine variable visibility.
- A query API: given a byte range `[start, end)`, return the
  parameter set, return value set, and local set as described above.

### Prerequisites

| Feature | What it contributes |
|---|---|
| Find References (see `lsp-features.md`) | Variable usage tracking across a scope — overlapping analysis |
| Implement missing methods (shipped) | Validates the code action + `WorkspaceEdit` plumbing |

---

## A2. Extract Function refactoring
**Impact: Low-Medium · Effort: High**

Select a range of statements inside a method/function and extract them into a
new function. With the `ScopeCollector` (A11) already in place, the remaining
work is:

1. **Statement boundary validation** — reject selections that split an
   expression or cross control-flow boundaries in invalid ways.
2. **Type annotation** — use variable type resolution to generate parameter and
   return type hints on the new function.
3. **Code generation** — produce a `WorkspaceEdit` that replaces the selection
   with a call and inserts the new function definition nearby.
4. **Range partitioning** — use the `ScopeCollector` to derive parameters,
   return values, and locals from the selected range.

### `$this` and method extraction

If the `ScopeCollector` reports that the selection reads `$this` (or
`self::`/`static::`), the extracted code must be a method on the same
class, not a standalone function.

### Prerequisites (build these first)

| Feature | What it contributes |
|---|---|
| ScopeCollector (A11) | Variable read/write tracking with byte offsets, range partitioning |
| Hover | "Resolve type at arbitrary position" — needed to type params |
| Document Symbols (see `lsp-features.md`) | AST range → symbol mapping — needed to find enclosing function and valid insertion points |
| Implement missing methods (shipped) | Builds the code action + `WorkspaceEdit` plumbing |

---

## A3. Switch → match conversion
**Impact: Low · Effort: Medium**

Offer a code action to convert a `switch` statement to a `match`
expression when the conversion is safe (PHP 8.0+).

### When the conversion is safe

- Every `case` body is a single expression statement (assignment to the
  same variable, or a `return`).
- No `case` body falls through to the next (every case ends with
  `break`, `return`, or `throw`).
- The switch subject is a simple expression (variable, property access,
  method call) — not something with side effects that shouldn't be
  evaluated multiple times.

### Implementation

- Walk the AST for `Statement::Switch` nodes.
- Check each arm against the safety criteria above.
- If all arms pass, build the `match` expression text:
  - Each `case VALUE:` becomes `VALUE =>`.
  - `default:` becomes `default =>`.
  - The body expression (minus the trailing `break;`) becomes the arm's
    RHS.
  - If all arms assign to the same variable, hoist the assignment:
    `$result = match ($x) { ... };`.
  - If all arms return, convert to `return match ($x) { ... };`.
- Offer as `refactor.rewrite` code action kind.
- Only offer when `php_version >= 8.0`.

**Note:** This is a structural AST transformation with no type
resolution dependency, but the safety checks for fall-through and
side-effect-free subjects require careful AST inspection. Not trivial,
but bounded in scope.

---

## A4. Inline Variable
**Impact: Medium · Effort: Medium**

Replace every occurrence of a variable with its right-hand-side
expression and delete the assignment. This is one of the most frequently
used refactorings in day-to-day editing.

### Behaviour

- **Trigger:** The cursor is on a variable assignment like
  `$name = $user->getName();`. The code action replaces every read of
  `$name` in the enclosing scope with `$user->getName()` and removes
  the assignment statement.
- **Code action kind:** `refactor.inline`.

### Safety checks

Before offering the action, verify:

1. **Single assignment.** The variable is assigned exactly once in the
   enclosing scope. If it is reassigned, inlining would change
   semantics. Reject and do not offer the action.
2. **Pure expression.** The RHS must be free of side effects if the
   variable is read more than once. Inlining `$x = getNextId()` into
   three call sites would call the function three times. Heuristic:
   treat method/function calls and `new` expressions as having side
   effects. Property accesses, constants, literals, and variable reads
   are safe.
   - If the RHS has side effects and there is exactly one read, the
     inline is still safe (evaluation count doesn't change).
   - If the RHS has side effects and there are multiple reads, reject.
3. **No intervening mutation.** Between the assignment and each read
   site, no statement mutates any variable or property referenced by the
   RHS. For the initial implementation, skip this check for simple
   RHS expressions (literals, single variable reads) and reject complex
   expressions where proving safety is hard.
4. **Parenthesisation.** When substituting the RHS into a larger
   expression, wrap in parentheses if needed to preserve precedence.
   For example, inlining `$x = $a + $b` into `$x * 2` must produce
   `($a + $b) * 2`.

### Scope

- Start with local variables inside function/method bodies.
- Properties (`$this->foo`) and class constants are out of scope for the
  initial implementation.

### Implementation

- Reuse the `ScopeCollector` infrastructure (A11). The collector
  already identifies all reads and writes of each variable with byte
  offsets.
- Collect all read sites for the target variable in the enclosing scope.
- Build a `WorkspaceEdit` that:
  1. Deletes the assignment statement (including trailing semicolon and
     newline).
  2. Replaces each read occurrence with the RHS text, adding parentheses
     where necessary.

### Prerequisites

| Feature | What it contributes |
|---|---|
| ScopeCollector (A11) | Variable read/write tracking with byte offsets |

---

## A5. Extract Variable
**Impact: Medium · Effort: Medium**

Select an expression and extract it into a new local variable assigned
just before the enclosing statement. This is the inverse of Inline
Variable.

### Behaviour

- **Trigger:** The user selects an expression (e.g. `$user->getName()`
  inside a longer statement). The code action introduces a new variable
  with a generated name, assigns the selected expression to it, and
  replaces the selection (and optionally all identical occurrences) with
  the new variable.
- **Code action kind:** `refactor.extract`.

### Name generation

Generate a default name from the expression:

- Method call: `$user->getName()` → `$getName` or `$name`.
- Property access: `$user->email` → `$email`.
- Static call: `Carbon::now()` → `$now`.
- Function call: `array_filter($items, ...)` → `$arrayFilter` or
  `$filtered`.
- Fallback: `$variable` with a numeric suffix if needed.

The user will typically rename the variable immediately after extraction
(editors select the name for in-place rename), so the generated name
just needs to be reasonable, not perfect.

### Duplicate replacement

When the same expression appears multiple times in the enclosing scope,
offer two variants:

1. **Extract (this occurrence only)** — replaces only the selected
   occurrence.
2. **Extract (all N occurrences)** — replaces every textually identical
   occurrence in the enclosing function body.

For the initial implementation, textual equality is sufficient. Semantic
equivalence (detecting that `$a->foo()` and `$a ->foo()` are the same)
is a follow-up.

### Insertion point

Insert the new assignment on the line immediately before the statement
that contains the selected expression, at the same indentation level.
If the expression is inside a control-flow header (e.g. `if ($x->isValid())`),
insert before the `if` statement, not inside the condition.

### Implementation

- Determine the selected expression's AST node and its text.
- Find the enclosing statement node to determine the insertion point.
- Generate the variable name from the expression.
- Build a `WorkspaceEdit` that:
  1. Inserts `$varName = <expression>;\n` before the enclosing statement.
  2. Replaces the selected expression with `$varName`.
  3. Optionally replaces other identical occurrences.

### Prerequisites

| Feature | What it contributes |
|---|---|
| ScopeCollector (A11) | Enclosing scope detection and variable name collision avoidance |

---

## A6. Inline Function/Method
**Impact: Medium · Effort: High**

Replace a function or method call with the body of the called function,
substituting parameters with the corresponding arguments.

### Behaviour

- **Trigger:** The cursor is on a function or method call. The code
  action replaces the call with the inlined body of the callee.
- **Code action kind:** `refactor.inline`.

### Simple case (single return statement)

When the callee body is a single `return <expr>;` statement:

- Replace the call expression with `<expr>`, substituting each parameter
  name with the corresponding argument expression.
- Add parentheses around substituted arguments where necessary to
  preserve precedence.

Example:

```php
function fullName(string $first, string $last): string {
    return $first . ' ' . $last;
}
// Before:
$name = fullName($user->first, $user->last);
// After:
$name = $user->first . ' ' . $user->last;
```

### Multi-statement body

When the callee has multiple statements:

- Replace the call statement with the full body of the callee, with
  parameter substitutions applied throughout.
- If the call site captures a return value (`$x = foo()`), replace the
  `return <expr>;` at the end of the inlined body with `$x = <expr>;`.
- If there are multiple `return` statements (early returns), the inline
  is significantly harder. For the initial implementation, reject
  functions with multiple return paths.

### Safety checks

1. **Resolvable callee.** The callee must resolve to a single known
   function or method definition. Dynamic calls (`$fn()`,
   `$obj->$method()`) are rejected.
2. **No recursion.** If the callee calls itself (directly or
   indirectly), reject. Detecting indirect recursion is hard, so start
   with direct recursion only.
3. **No `$this` / `self` / `static` leakage.** If inlining a method
   call and the method body references `$this`, `self::`, or `static::`,
   the inlined code must be placed in a context where those references
   still make sense (i.e. within the same class or a subclass). If the
   call site is a standalone function, reject.
4. **Variable name collisions.** Local variables in the callee body
   might collide with variables at the call site. Rename the callee's
   locals if they shadow call-site variables.
5. **By-reference parameters.** If a parameter is passed by reference,
   the corresponding argument must be a variable (not an expression).
   This is already enforced by PHP, so no extra check is needed.
6. **Single return.** For the initial implementation, reject callees
   with multiple `return` statements or `return` inside
   loops/conditionals.

### Scope

- Start with standalone functions and static methods (no `$this`
  complications).
- Instance methods where the call site is within the same class are a
  natural second step.
- Cross-file inlining (the callee is in a different file) requires
  loading the callee's source. The infrastructure for this exists
  (PSR-4 loader, `find_or_load_class`), but the callee needs to be
  loaded as raw source text, not just as parsed `ClassInfo`.

### Implementation

- Resolve the call to its definition using Go-to-Definition
  infrastructure.
- Read the callee's body text from the source file.
- Parse parameter names from the callee's signature.
- Build a substitution map: parameter name → argument expression text.
- Apply substitutions throughout the body text.
- Detect and rename colliding local variables.
- Build a `WorkspaceEdit` that replaces the call statement with the
  transformed body.

### Prerequisites

| Feature | What it contributes |
|---|---|
| Go-to-Definition | Resolves call site to the callee's definition location and source |
| ScopeCollector (A11) | Variable collision detection at the call site |

---

## A7. Extract Constant
**Impact: Medium · Effort: Medium**

Select a literal value (string, integer, float, boolean) inside a class
and extract it into a class constant. This pairs naturally with Extract
Variable (A5) and shares the same "select, name, replace" workflow.

### Behaviour

- **Trigger:** The user selects a literal expression inside a class
  method or property default. The code action introduces a new class
  constant with a generated name, assigns the literal value, and
  replaces the selection (and optionally all identical occurrences in
  the class) with `self::CONSTANT_NAME`.
- **Code action kind:** `refactor.extract`.

### What can be extracted

- String literals: `'pending'`, `"active"`.
- Integer literals: `200`, `0xFF`.
- Float literals: `3.14`.
- Boolean literals: `true`, `false` (less common but valid).
- Concatenated string expressions: `'prefix_' . 'suffix'` — extract the
  whole expression as a single constant.

Array literals and class instantiations are out of scope (PHP const
expressions are limited).

### Name generation

Generate a default name from the value:

- String: `'pending'` → `PENDING`. `'order_status'` → `ORDER_STATUS`.
- Number: `200` → `STATUS_200` or `VALUE_200`.
- Boolean: `true` → `IS_ENABLED` (weak heuristic, user will rename).
- Fallback: `CONSTANT` with a numeric suffix if needed.

Use `SCREAMING_SNAKE_CASE` per PHP convention. If the generated name
collides with an existing constant in the class, append a numeric suffix.

### Insertion point

Insert the new constant declaration at the top of the class body, after
any existing constant declarations (to keep constants grouped). Use the
visibility of the surrounding context as a hint: if the literal appears
in a public method, default to `public const`; otherwise `private const`.

### Duplicate replacement

Same approach as Extract Variable (A5): offer "this occurrence only"
and "all N occurrences in this class". Textual equality is sufficient
for literals.

### Implementation

- Verify the selection is a literal expression node inside a class body.
- Find the class declaration node and scan for existing constants.
- Generate the constant name and check for collisions.
- Determine the insertion point (after last existing constant, or at
  the top of the class body if none exist).
- Build a `WorkspaceEdit` that:
  1. Inserts `{visibility} const NAME = {value};\n` at the insertion
     point with correct indentation.
  2. Replaces the selected literal with `self::NAME`.
  3. Optionally replaces other identical literals in the class.

### Prerequisites

| Feature | What it contributes |
|---|---|
| ScopeCollector (A11) | Class body traversal and constant name collision detection |

---

## A8. Update Docblock to Match Signature
**Impact: Medium · Effort: Medium**

When a function or method signature changes (parameters added, removed,
reordered, or type hints updated), the docblock often falls out of sync.
This code action regenerates or patches the `@param`, `@return`, and
`@throws` tags to match the current signature.

### Behaviour

- **Trigger:** Cursor is on a function/method declaration that has an
  existing docblock. The code action appears when the docblock's `@param`
  tags don't match the signature's parameters (by name, count, or order),
  or when the `@return` tag contradicts the return type hint.
- **Code action kind:** `quickfix` (when tags are clearly wrong) or
  `source.fixAll.docblock` for a broader sweep.

### What gets updated

1. **`@param` tags:**
   - Add missing `@param` for parameters present in the signature but
     absent from the docblock.
   - Remove `@param` for parameters no longer in the signature.
   - Reorder `@param` tags to match signature order.
   - Update the type if the signature has a type hint and the docblock
     type contradicts it (e.g. docblock says `string`, signature says
     `int`). If the docblock type is _more specific_ than the signature
     (e.g. docblock says `non-empty-string`, signature says `string`),
     keep the docblock type (it's a refinement, not a contradiction).
   - Preserve existing descriptions after the type and variable name.

2. **`@return` tag:**
   - If the signature has a return type hint and the docblock `@return`
     contradicts it, update the type. Same refinement rule: keep the
     docblock type if it's more specific.
   - If the signature has a return type but no `@return` tag exists,
     do not add one (the type hint is sufficient). Only update or
     remove existing tags.
   - Remove `@return void` if redundant with a `: void` return type.

3. **Preserve other tags:** `@throws`, `@template`, `@deprecated`,
   `@see`, and any other tags are left untouched.

### Edge cases

- **Promoted constructor parameters:** Treat the same as regular
  parameters for `@param` purposes.
- **Variadic parameters:** `...$args` matches `@param type ...$args`.
- **No existing docblock:** This action only patches existing docblocks.
  PHPDoc generation on `/**` (F1) handles creating new ones.

### Implementation

- Parse the function signature to extract parameter names, types, and
  order, plus the return type.
- Parse the existing docblock to extract `@param` and `@return` tags
  with their positions, types, variable names, and descriptions.
- Diff the two lists to determine additions, removals, reorderings,
  and type updates.
- Build a `WorkspaceEdit` with targeted `TextEdit`s that modify only
  the changed lines within the docblock, preserving formatting,
  indentation, and unchanged tags.

### Prerequisites

| Feature | What it contributes |
|---|---|
| Docblock tag parsing (`docblock/tags.rs`) | Extracts existing `@param`/`@return` tags with positions |
| Parser (`parser/functions.rs`) | Extracts parameter names, types, and return type from the signature |

---

## A9. Change Visibility
**Impact: Low-Medium · Effort: Low**

Change the visibility of a method, property, constant, or class from the
cursor position. Offers all applicable alternatives as separate code
actions.

### Behaviour

- **Trigger:** Cursor is on (or inside) a method, property, constant, or
  promoted constructor parameter that has an explicit visibility modifier.
- **Code action kind:** `refactor.rewrite`.
- **Offered actions:** One action per alternative visibility. For a
  `public` method, offer "Make protected" and "Make private". For a
  `private` property, offer "Make protected" and "Make public".

### What can be changed

- Methods: `public` ↔ `protected` ↔ `private`.
- Properties (including promoted constructor parameters): same.
- Constants: `public` ↔ `protected` ↔ `private` (PHP 8.1+).
- Constructor visibility promotion: `public function __construct(private string $name)` —
  change `private` to `protected` or `public`.

### Scope

This is a single-file edit. It does **not** update call sites or
subclass overrides in other files. If a user makes a public method
private and there are external callers, they'll see errors from
diagnostics or their static analyser. Cross-file visibility propagation
is a possible follow-up but not required for the initial implementation.

### Implementation

- Find the visibility keyword token for the member under the cursor.
- Determine the current visibility.
- For each alternative visibility, create a code action whose
  `WorkspaceEdit` replaces the visibility keyword token with the new one.
- Handle the edge case of implicit public visibility (no keyword present)
  by inserting the keyword before the `function`/property token.

### Prerequisites

None beyond basic AST navigation. This is one of the simplest possible
code actions.

---

## A10. Generate Interface from Class
**Impact: Low-Medium · Effort: Medium**

Extract an interface from an existing class. The new interface contains
method signatures for all public methods in the class, and the class is
updated to implement it.

### Behaviour

- **Trigger:** Cursor is on a class declaration. The code action
  "Extract interface" appears.
- **Code action kind:** `refactor.extract`.
- **Result:** A new file is created containing the interface, and the
  original class is updated to add `implements InterfaceName`.

### What gets extracted

- All `public` methods (excluding the constructor) become interface
  method signatures: visibility, name, parameters with types and
  defaults, and return type.
- PHPDoc blocks from the extracted methods are copied to the interface
  (they often contain `@param`, `@return`, and `@template` tags that
  are essential for type information).
- Class-level `@template` tags are copied if any extracted method
  references those template parameters.
- Public constants are **not** extracted (interface constants have
  different semantics and this is rarely what users want).

### Naming

Default interface name: `{ClassName}Interface`. Place it in the same
namespace and directory as the class. If the file uses PSR-4, the
interface file path is derived from the namespace.

### Implementation

- Parse the class to collect public method signatures and their
  docblocks.
- Collect class-level `@template` tags if referenced by extracted
  methods.
- Generate the interface source: namespace declaration, use imports
  needed by the method signatures, interface declaration with method
  stubs.
- Build a `WorkspaceEdit` with two operations:
  1. `CreateFile` + `TextEdit` for the new interface file.
  2. `TextEdit` on the original class to add `implements InterfaceName`
     (and a `use` import if the interface is in a different file, though
     by default it's the same namespace).
- Format the generated interface to match the project's indentation
  style (detect from the source class).

### Edge cases

- **Class already implements interfaces:** Append to the existing
  `implements` list rather than replacing it.
- **Abstract methods:** Include them in the interface (they're already
  stubs).
- **Static methods:** Include them. Interfaces can declare static method
  signatures.
- **Generic classes:** If the class has `@template T` and a method
  returns `T`, the interface needs the same `@template` tag.

### Prerequisites

| Feature | What it contributes |
|---|---|
| Parser (`parser/classes.rs`) | Extracts public method signatures with full type information |
| Implement missing methods (shipped) | Shared infrastructure for generating method stubs and `implements` clause editing |