# PHPantom — Code Actions

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

**Refactoring code actions overview:** A2 (Extract Function), A6
(Inline Function/Method), and A7 (Extract Constant) depend on
forward-pass variable usage tracking with byte offsets across function
scopes.

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

| Feature              | What it contributes                                               |
| -------------------- | ----------------------------------------------------------------- |
| Go-to-Definition     | Resolves call site to the callee's definition location and source |
| ScopeCollector (A11) | Variable collision detection at the call site                     |

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

| Feature              | What it contributes                                        |
| -------------------- | ---------------------------------------------------------- |
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

| Feature                                   | What it contributes                                                 |
| ----------------------------------------- | ------------------------------------------------------------------- |
| Docblock tag parsing (`docblock/tags.rs`) | Extracts existing `@param`/`@return` tags with positions            |
| Parser (`parser/functions.rs`)            | Extracts parameter names, types, and return type from the signature |

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

| Feature                             | What it contributes                                                               |
| ----------------------------------- | --------------------------------------------------------------------------------- |
| Parser (`parser/classes.rs`)        | Extracts public method signatures with full type information                      |
| Implement missing methods (shipped) | Shared infrastructure for generating method stubs and `implements` clause editing |


## A16. Snippet Placeholder for Extracted Method Name

**Impact: Medium · Effort: Low-Medium**

> **Blocked:** Requires `SnippetTextEdit` support in `lsp-types`.
> Upstream issue: [gluon-lang/lsp-types#310](https://github.com/gluon-lang/lsp-types/issues/310).
> The current `lsp-types` (0.94, pinned by `tower-lsp` 0.20) only
> covers LSP 3.17. `SnippetTextEdit` is an LSP 3.18 proposed feature.
> Revisit once the upstream issue is resolved and `tower-lsp` picks up
> the new version.

After an Extract Function/Method code action is applied, let the user
immediately rename the generated name by placing a snippet tab-stop on
it.  The contextual name (`createUsers`, `validateGuard`, …) serves as
the default, but the cursor lands directly on it so the user can type
over it without an extra rename step.

### Behaviour

- **Trigger:** User applies "Extract method 'createUsers'" (or any
  extract function/method action).
- **Result:** The workspace edit uses a `SnippetTextEdit` with
  `${1:createUsers}` for the method name at both the definition site
  and every call site.  The editor enters snippet mode and the user
  can type a new name that updates all locations simultaneously.
- **Fallback:** When the client does not advertise
  `workspace.workspaceEdit.snippetEditSupport`, emit a regular
  `TextEdit` (current behaviour — no snippet, no cursor placement).

### Implementation

1. **Store client capabilities at initialisation.**  In `initialize`,
   save the `InitializeParams.capabilities` (or at least the snippet
   edit flag) on the `Backend` struct.

2. **Check the flag in `collect_extract_function_actions`.**  When
   the client supports snippet edits, build the workspace edit with
   `DocumentChanges::Operations` containing `SnippetTextEdit` entries
   instead of plain `TextEdit`.  The new-text for the method name
   uses `${1:name}` syntax.

3. **Linked edit ranges (optional enhancement).**  If the client
   supports `workspace.workspaceEdit.changeAnnotationSupport` or
   linked edit groups, use those so that editing the name at the
   definition also updates the call site in real time.

### Prerequisites

| Feature                          | What it contributes                                       |
| -------------------------------- | --------------------------------------------------------- |
| Client capability storage        | Need to know whether the client supports snippet edits    |
| `SnippetTextEdit` in tower-lsp   | Verify tower-lsp exposes the snippet edit type            |
| Extract Function (shipped)       | The code action that this enhances                        |




