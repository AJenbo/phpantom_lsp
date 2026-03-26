# PHPantom — Mago Crate Migration

This document describes the migration from hand-rolled PHP parsing
subsystems to upstream Mago crates. The goal is to replace fragile,
maintenance-heavy internal code with well-tested, upstream-maintained
libraries — improving correctness and robustness while reducing the
long-term maintenance burden.

> **Guiding principle:** Correctness and robustness win over raw
> performance. We accept modest overhead from structured
> representations in exchange for eliminating entire classes of
> edge-case bugs in string-based type manipulation.

## Crates to adopt

| Crate              | Replaces                                               | Effort      |
| ------------------ | ------------------------------------------------------ | ----------- |
| `mago-docblock`    | `src/docblock/tags.rs` + tag extraction logic           | Medium      |
| `mago-type-syntax` | `src/docblock/{type_strings,generics,shapes,callable_types,conditional}.rs` + string-based type pipeline | Very High |
| `mago-names`       | `src/parser/use_statements.rs` + `use_map` resolution   | Medium-High |

A fifth crate, `mago-reporting`, comes in as a transitive dependency
of `mago-semantics` and `mago-names`. It does not replace any
PHPantom code but will appear in `Cargo.toml`.

### Crates explicitly ruled out

| Crate              | Reason                                                                   |
| ------------------ | ------------------------------------------------------------------------ |
| `mago-codex`       | Replaces `ClassInfo` model with one that cannot carry `LaravelMetadata`. |
| `mago-semantics`   | 12K false positives on Laravel; no way to inject our type context.       |
| `mago-linter`      | Same problem; `Integration::Laravel` is surface-level only.              |
| `mago-fingerprint` | Requires `mago-names` for limited value; `signature_eq` already works.   |

---

## Sprint placement

The migration goes between Sprint 4 (Refactoring toolkit) and
Sprint 5 (Polish for office adoption). This is the latest safe
point: Sprint 5 introduces workspace symbols, auto-import,
diagnostics for unknown classes/members, and implement-interface —
all of which touch name resolution, docblock parsing, and type
resolution. Sprint 6 (Type intelligence depth) directly manipulates
type strings. Sprint 7 (Laravel excellence) and Sprint 8 (Blade
support) both depend heavily on the subsystems being replaced.

Building any of those features on the old string-based code and then
migrating afterward would mean rewriting them twice. Building them on
the new foundation means they benefit from structured types from day
one.

```
Sprint 3  — Bug fixes                    (no migration dependency)
Sprint 4  — Refactoring toolkit           (no migration dependency)
Sprint 4a — Mago Foundation: Composer + Docblock + Names  ← NEW
Sprint 4b — Mago Foundation: Type Syntax  ← NEW
Sprint 5  — Polish for office adoption    (built on new foundation)
Sprint 6  — Type intelligence depth       (built on new foundation)
Sprint 7  — Laravel excellence            (built on new foundation)
Sprint 8  — Blade support                 (built on new foundation)
```

Sprint 4a and 4b are separated because type-syntax is a
significantly larger migration that depends on docblock and names
being in place first. Each sub-sprint should be roughly 1–2 weeks.

Add to `docs/todo.md` after Sprint 4:

```
## Sprint 4a — Mago foundation: Composer + Docblock + Names

| #   | Item                                                        | Impact | Effort      |
| --- | ----------------------------------------------------------- | ------ | ----------- |
|     | Clear [refactoring gate](todo/refactor.md)                  | —      | —           |
| M2  | [Migrate to mago-docblock](todo/mago.md#m2-mago-docblock)   | High   | Medium      |
| M3  | [Migrate to mago-names](todo/mago.md#m3-mago-names)         | High   | Medium-High |

## Sprint 4b — Mago foundation: Type Syntax

| #   | Item                                                              | Impact   | Effort    |
| --- | ----------------------------------------------------------------- | -------- | --------- |
|     | Clear [refactoring gate](todo/refactor.md)                        | —        | —         |
| M4  | [Migrate to mago-type-syntax](todo/mago.md#m4-mago-type-syntax)  | Critical | Very High |
```

---

## M2. Migrate to `mago-docblock`

**What it replaces:** Tag extraction logic in `src/docblock/tags.rs`,
`src/docblock/templates.rs`, and `src/docblock/virtual_members.rs`
that used line-by-line string scanning.

**What it does NOT replace:** The type expression parsing inside tag
descriptions (that is M4). After this step, we still extract the raw
description string from tags and process it with our existing
string-based type code. The structured type migration happens in M4.

### Remaining

These are optional follow-up items that would benefit from being
rewritten to use mago-docblock but are not blocking M3 or M4.

6. **Migrate `extract_conditional_return_type` in `conditional.rs`.**
   Still uses manual `/** */` stripping and line-by-line scanning in
   `extract_raw_return_content()` to collect multi-line `@return`
   content. Could use `parse_docblock_for_tags` + `TagKind::Return`
   to get the description directly (mago-docblock already joins
   multi-line tag content). The conditional expression parser itself
   (`parse_conditional_expr`) would stay unchanged.

7. **Migrate `get_docblock_text_for_node` callers to use
   `parse_docblock_trivia` directly.**
   Currently ~15 call sites in `parser/classes.rs`,
   `parser/functions.rs`, `parser/mod.rs`, and
   `code_actions/generate_constructor.rs` follow the pattern:
   `get_docblock_text_for_node(trivia, content, node)` →
   `extract_*(raw_text)`. Each call parses the docblock twice: once
   to find the trivia token, once inside the extraction function.
   These could call `parse_docblock_trivia` once and pass the
   `DocblockInfo` to extraction functions, or the extraction
   functions could accept `&DocblockInfo` directly. This is a
   performance improvement and API cleanup, not a correctness fix.

8. **Update `DocblockCtx` to carry parsed `DocblockInfo`.**
   `DocblockCtx` in `src/parser/mod.rs` carries trivia, content, and
   context for docblock extraction during AST walks. It could carry a
   lazily-parsed `DocblockInfo` so that multiple tag extractions from
   the same docblock share a single parse. This pairs naturally with
   item 7.

9. **Source-scanning functions are out of scope.**
   `find_inline_var_docblock`, `find_var_raw_type_in_source`,
   `find_iterable_raw_type_in_source`, and
   `find_enclosing_return_type` scan backward through raw source
   text looking for docblocks by byte offset. These are not
   tag-extraction functions and cannot benefit from mago-docblock.
   Type-level helpers (`should_override_type`,
   `resolve_effective_type`, `is_compatible_refinement`) are type
   logic, not parsing.

### Notes

- mago-docblock joins multi-line tag descriptions with `\n` rather
  than spaces. The migrated functions normalise `\n` → space where
  the old code joined continuation lines with spaces (descriptions,
  type extraction). This preserves existing behaviour.

- `@phpstan-extends`, `@phpstan-implements`, and `@phpstan-use` are
  not recognised as dedicated `TagKind` variants by mago-docblock
  (they become `TagKind::Other`). `extract_generics_tag` handles
  this with a name-based fallback that checks `tag.name`.

- `@removed` is similarly `TagKind::Other`; matched by name in
  `extract_removed_version`.

### Performance note

`mago-docblock` allocates into a bumpalo arena. For the current
approach (parse then immediately flatten to owned strings), we create
and drop an arena per docblock. This is fine — bumpalo arena creation
is a pointer bump, and the arena is dropped at the end of each
extraction call. When M4 introduces structured types, the arena
lifetime may be extended to match the type's lifetime. The follow-up
items (7, 8) would reduce redundant parsing by sharing a single
`DocblockInfo` across multiple extractions from the same docblock.

---

## M3. Migrate to `mago-names`

**What it replaces:** `src/parser/use_statements.rs` (130 lines),
the `use_map: DashMap<String, HashMap<String, String>>` on `Backend`,
and the lazy name-resolution helpers in `src/resolution.rs` that
manually look up the use map.

**Why:** `mago-names` resolves every identifier in a PHP file to its
fully-qualified name in a single pass. This is more correct than our
lazy approach for edge cases: names that resolve differently depending
on whether they appear in a type hint vs. a `new` expression vs. a
function call (PHP's different name resolution rules for classes,
functions, and constants). It also provides `is_imported()` which
tells us whether a name came from a `use` statement — useful for
auto-import code actions and unused-import diagnostics.

**What it does NOT replace:** Cross-file resolution
(`find_or_load_class`, PSR-4 resolution, classmap lookup, stub
loading). Those stay in `src/resolution.rs`. `mago-names` handles
only the within-file syntactic resolution (use statements + namespace
context → FQN).

**Risk:** Medium-high. The `use_map` is read from many places. The
arena lifetime for `ResolvedNames` must outlive the consumers.
Requires restructuring how we store per-file name resolution data.

### Steps

1. **Add `mago-names` to `Cargo.toml`.**
   This also brings in `foldhash` as a transitive dependency.

2. **Run the name resolver in `update_ast_inner`.**
   After parsing the `Program`, call
   `mago_names::resolver::NameResolver::new(&arena).resolve(program)`
   to produce a `ResolvedNames`. This happens in the same arena as
   the parse.

3. **Store resolved names per file.**
   `ResolvedNames<'arena>` borrows from the arena, but our arenas
   are dropped at the end of `update_ast_inner`. Two options:

   **Option A — Copy to owned storage.** Extract the resolved names
   into an owned `HashMap<u32, (String, bool)>` (offset → FQN +
   imported flag) and store that on `Backend` in a new
   `DashMap<String, Arc<OwnedResolvedNames>>`. This is the simpler
   approach and keeps the existing lifetime model.

   **Option B — Keep arenas alive.** Store the `Bump` arena
   alongside the `ResolvedNames` in an `Arc`-wrapped struct. This
   avoids the copy but requires more careful lifetime management.

   Start with Option A. It's simpler to reason about and the copy
   cost is bounded (one `HashMap` insert per identifier per file,
   done once per re-parse). Optimise to Option B later if profiling
   shows it matters.

4. **Build an `OwnedResolvedNames` wrapper.**
   Create a `src/names.rs` module with a struct that mirrors the
   `ResolvedNames` API but owns its data:

   ```
   pub struct OwnedResolvedNames {
       names: HashMap<u32, (String, bool)>,
   }

   impl OwnedResolvedNames {
       pub fn get(&self, offset: u32) -> Option<&str>;
       pub fn is_imported(&self, offset: u32) -> bool;
   }
   ```

   Populate it from `ResolvedNames` at the end of `update_ast_inner`.

5. **Replace `use_map` reads incrementally.**
   The `use_map` is read in:
   - `src/resolution.rs` — `resolve_class_name`, `resolve_function_name`
   - `src/diagnostics/unknown_classes.rs`
   - `src/diagnostics/unknown_functions.rs`
   - `src/diagnostics/unknown_members.rs`
   - `src/diagnostics/unused_imports.rs`
   - `src/completion/` (various modules)
   - `src/definition/` (various modules)
   - `src/references/`
   - `src/rename/`
   - `src/code_actions/import_class.rs`

   For each call site:
   - If the call site has access to the AST node's byte offset, use
     `resolved_names.get(offset)` to get the FQN directly. This
     eliminates the manual "look up short name in use_map, prepend
     namespace" dance.
   - If the call site only has a string name (no offset), keep the
     existing `resolve_class_name` / `resolve_function_name` helper
     but rewrite it to query `OwnedResolvedNames` instead of the raw
     use map.

   Do this incrementally — one module per commit.

6. **Deprecate and remove `use_map`.**
   Once all consumers use `OwnedResolvedNames`, remove the
   `use_map: DashMap<String, HashMap<String, String>>` from
   `Backend`. Also remove `extract_use_items` and
   `extract_use_statements_from_statements` from
   `src/parser/use_statements.rs`.

7. **Keep `namespace_map` for now.**
   The per-file namespace is still needed for PSR-4 resolution and
   class index construction. `mago-names` doesn't expose the file's
   namespace as a standalone value, so keep `namespace_map` or extract
   the namespace from the AST directly (it's trivial — first
   `Statement::Namespace` node).

8. **Update unused-import diagnostics.**
   `mago-names` provides `is_imported()` for each resolved name. An
   unused import is a `use` statement whose imported names never
   appear in `ResolvedNames` with `imported = true`. This may
   simplify the current `unused_imports.rs` logic.

9. **Run the full test suite.**

### Interaction with M2

M2 (mago-docblock) and M3 (mago-names) are independent — neither
depends on the other. They can be done in parallel or in either
order. M3 is listed second because it touches more call sites and
has a larger blast radius.

### Interaction with M4

M4 (mago-type-syntax) does NOT depend on mago-names. Type expression
parsing is purely syntactic — it takes a string and returns a type
AST. However, once both M3 and M4 are complete, the combination
enables a powerful pattern: resolve an identifier's FQN via
mago-names, then parse its docblock type via mago-type-syntax, and
work with fully-resolved structured types throughout. This is
especially valuable for the Laravel provider, where a relationship
return type like `HasMany<Post, $this>` needs both FQN resolution
(what is `Post`?) and type structure (what are the generic args?).

---

## M4. Migrate to `mago-type-syntax`

**What it replaces:** The string-based type pipeline — approximately
4,700 lines across:

- `src/docblock/type_strings.rs` (584 lines) — `clean_type`,
  `split_type_token`, `base_class_name`, `strip_nullable`,
  `normalize_nullable`, `strip_generics`, `is_scalar`, etc.
- `src/docblock/generics.rs` (228 lines) — `extract_generic_value_type`,
  `extract_generic_key_type`, `extract_iterable_element_type`
- `src/docblock/shapes.rs` (372 lines) — `parse_array_shape`,
  `parse_object_shape`, `extract_array_shape_value_type`
- `src/docblock/callable_types.rs` (290 lines) —
  `extract_callable_return_type`, `extract_callable_param_types`
- `src/docblock/conditional.rs` (214 lines) —
  `extract_conditional_return_type`, conditional type tree parsing
- `src/completion/types/resolution.rs` (463 lines) —
  `type_hint_to_classes`, `apply_generic_args`, generic substitution
- `src/completion/types/conditional.rs` (519 lines) — conditional
  return type resolution
- `src/completion/types/narrowing.rs` (1,906 lines) — `is_subtype_of`,
  type comparison, instanceof narrowing (type manipulation portions)
- `src/inheritance.rs` (1,094 lines) — `apply_substitution`,
  `apply_substitution_to_method`, `apply_substitution_to_property`

Plus scattered string-type manipulation in the Laravel virtual member
providers.

**Why:** This is the highest-maintenance, most bug-prone code in
PHPantom. Every new PHPStan type feature (conditional types, template
constraints, array shapes with spreads, `key-of`, `value-of`) requires
extending hand-written string parsers. `mago-type-syntax` is a proper
lexer + parser that handles the full PHPStan/Psalm type expression
grammar including edge cases we don't support today (int ranges,
`properties-of`, indexed access types, literal types, negated types).

**This is the big one.** It touches nearly every module and changes
the fundamental type representation from strings to AST nodes. The
migration must be done in carefully ordered phases to keep the test
suite green at every step.

**Risk:** High — largest blast radius of any migration. Mitigated by
the phased approach and the fact that every phase is independently
testable.

### Phase 1: Introduce the type representation

**Goal:** Define PHPantom's internal type representation backed by
`mago-type-syntax`, with bidirectional conversion to/from strings.
No existing code changes behaviour.

1. **Add `mago-type-syntax` to `Cargo.toml`.**

2. **Create `src/types/php_type.rs` — the internal type enum.**
   Define a PHPantom-owned type representation that wraps or mirrors
   the `mago_type_syntax::ast::Type` variants we care about. This is
   an owned, arena-free type that can live in `ClassInfo`,
   `MethodInfo`, `PropertyInfo`, etc.

   Start with a minimal enum covering the types PHPantom already
   handles:

   - `Scalar(ScalarKind)` — int, string, float, bool, null, void, never, mixed
   - `Reference { fqn: String, generic_args: Vec<PhpType> }`
   - `Union(Vec<PhpType>)`
   - `Intersection(Vec<PhpType>)`
   - `Nullable(Box<PhpType>)`
   - `Array { key: Option<Box<PhpType>>, value: Option<Box<PhpType>> }`
   - `Shape { entries: Vec<ShapeEntry>, sealed: bool }`
   - `Callable { params: Vec<CallableParam>, return_type: Option<Box<PhpType>> }`
   - `Conditional { param: String, condition: ParamCondition, then: Box<PhpType>, otherwise: Box<PhpType> }`
   - `Slice(Box<PhpType>)` — `T[]`
   - `Variable { name: String, scope: TemplateScope }` — `$this`,
     template variables. The scope distinguishes class-level `T` from
     method-level `T` on the same class. PHPStan identifies template
     types by name + scope (`TemplateTypeScope`); without this,
     a method declaring its own `@template T` that shadows the
     class's `T` would produce incorrect substitutions.
   - `ClassString(Option<Box<PhpType>>)` — `class-string<T>`
   - `KeyOf(Box<PhpType>)`, `ValueOf(Box<PhpType>)`
   - `Static`, `Self_`, `Parent`
   - `Literal(LiteralKind)` — literal ints, strings, bools

   This does not need to cover every `mago_type_syntax::ast::Type`
   variant on day one. Unknown variants map to a `Raw(String)`
   fallback that preserves the original string.

3. **Implement `PhpType::parse(input: &str) -> PhpType`.**
   Calls `mago_type_syntax::parse_str()` and converts the resulting
   AST into `PhpType`. Unknown or unsupported AST variants become
   `PhpType::Raw(input.to_string())`.

4. **Implement `PhpType::to_string() -> String`.**
   Renders the type back to a display string. This is needed for
   hover, completion detail, signature help, and anywhere we
   currently show type strings to the user.

5. **Write thorough unit tests.**
   Round-trip tests: parse a type string, convert to `PhpType`,
   render back to string. Cover every variant including edge cases:
   - `array{name: string, age?: int, ...}`
   - `Closure(int, string): bool`
   - `($x is string ? int : float)`
   - `Collection<int, User>|null`
   - `non-empty-array<string, mixed>`
   - `callable(list<int>, ?Closure(): void=, int...): ((A&B)|null)`
   - `key-of<MyArray>`
   - `T[][]`

### Phase 2: Dual representation on core types

**Goal:** Add `PhpType` fields alongside existing `String` fields on
`MethodInfo`, `PropertyInfo`, `FunctionInfo`, `ParameterInfo`. Both
representations are kept in sync. Existing code continues to read
strings; new code can read structured types.

1. **Add optional `PhpType` fields to info structs.**
   For example, on `MethodInfo`:

   ```
   pub return_type: Option<String>,        // existing
   pub return_type_parsed: Option<PhpType>, // new — always in sync
   ```

   On `PropertyInfo`:

   ```
   pub type_hint: Option<String>,
   pub type_hint_parsed: Option<PhpType>,
   ```

   And similarly for `native_return_type`, `ParameterInfo::type_hint`,
   etc.

2. **Populate `_parsed` fields at extraction time.**
   In `src/parser/classes.rs` and `src/parser/functions.rs`, wherever
   we set a `type_hint` or `return_type` string, also call
   `PhpType::parse()` and store the result. This is the single point
   of truth — the string and the parsed type are always created
   together.

3. **Update `signature_eq` to compare `_parsed` fields.**
   Structured comparison is more correct than string comparison
   (e.g. `int|string` and `string|int` are equivalent types but
   different strings).

4. **Run the full test suite.** Nothing should change behaviourally —
   the `_parsed` fields are written but not yet read by any
   production code path.

### Phase 3: Migrate consumers to structured types

**Goal:** Module by module, switch from reading `type_hint: String`
to reading `type_hint_parsed: PhpType`. This is the bulk of the work.

Migrate in this order (least coupled → most coupled):

1. **`src/docblock/generics.rs`** — Replace `extract_generic_value_type`,
   `extract_generic_key_type`, `extract_iterable_element_type` with
   pattern matches on `PhpType::Reference { generic_args, .. }` and
   `PhpType::Array { value, .. }`.

2. **`src/docblock/shapes.rs`** — Replace `parse_array_shape`,
   `extract_array_shape_value_type`, `parse_object_shape` with
   pattern matches on `PhpType::Shape { entries, .. }`.

3. **`src/docblock/callable_types.rs`** — Replace
   `extract_callable_return_type`, `extract_callable_param_types`
   with pattern matches on `PhpType::Callable { .. }`.

4. **`src/docblock/conditional.rs`** — Replace
   `extract_conditional_return_type` with pattern match on
   `PhpType::Conditional { .. }`. Rewrite `ConditionalReturnType`
   to hold `PhpType` instead of strings.

5. **`src/completion/types/resolution.rs`** — Rewrite
   `type_hint_to_classes` to accept `PhpType` instead of `&str`.
   Rewrite `apply_generic_args` to substitute `PhpType` nodes
   instead of string-replace. This is the most impactful single
   change — it's the bridge between types and class resolution.

6. **`src/inheritance.rs`** — Rewrite `apply_substitution` and its
   variants to operate on `PhpType` trees instead of string
   find-and-replace. Template parameters become `PhpType::Variable`
   nodes; substitution is a recursive tree transform.

7. **`src/completion/types/narrowing.rs`** — Rewrite `is_subtype_of`
   and type comparison functions to operate on `PhpType`. Instanceof
   narrowing produces a new `PhpType::Reference` instead of a string.

8. **`src/completion/types/conditional.rs`** — Rewrite conditional
   return type resolution to evaluate `PhpType::Conditional` trees.

9. **`src/hover/`** — Update hover rendering to use
   `PhpType::to_string()` instead of raw type strings.

10. **`src/signature_help.rs`** — Update parameter type display.

11. **`src/inlay_hints.rs`** — Update inlay hint type display.

After each module is migrated, run the full test suite.

### Phase 4: Migrate the Laravel provider

**Goal:** Rewrite the Laravel virtual member providers to produce and
consume `PhpType` instead of strings.

1. **`src/virtual_members/laravel/relationships.rs`** — Relationship
   type inference. Instead of parsing `HasMany<Post, $this>` as a
   string and extracting generic args with `find('<')`, receive a
   `PhpType::Reference { fqn: "HasMany", generic_args: [Reference("Post"), Variable("$this")] }`
   and pattern-match on it.

2. **`src/virtual_members/laravel/builder.rs`** — Builder return type
   mapping. Instead of string-replacing `static` → `Builder<Model>`,
   do a tree transform on `PhpType`.

3. **`src/virtual_members/laravel/casts.rs`** — Cast type resolution.
   `cast_type_to_php_type` returns a `PhpType` instead of a string.

4. **`src/virtual_members/laravel/scopes.rs`** — Scope method
   synthesis. Parameter and return types are `PhpType`.

5. **`src/virtual_members/laravel/accessors.rs`** — Accessor type
   extraction. Returns `PhpType`.

6. **`src/virtual_members/laravel/factory.rs`** — Factory method
   synthesis. Return types are `PhpType`.

7. **`src/virtual_members/phpdoc.rs`** — PHPDoc virtual member
   provider. `@method` and `@property` tag types are parsed into
   `PhpType` via `mago-docblock` + `mago-type-syntax`.

### Phase 5: Remove string type fields

**Goal:** Remove the old `String`-based type fields and the
`docblock/type_strings.rs` helper module.

1. **Remove `return_type: Option<String>` and friends.**
   Rename `return_type_parsed` → `return_type`. All consumers now
   use `PhpType`.

2. **Delete `src/docblock/type_strings.rs`** (584 lines).

3. **Delete `src/docblock/generics.rs`** (228 lines) — replaced by
   `PhpType` pattern matching.

4. **Delete `src/docblock/shapes.rs`** (372 lines).

5. **Delete `src/docblock/callable_types.rs`** (290 lines).

6. **Delete `src/docblock/conditional.rs`** (214 lines).

7. **Update `PhpType::Raw(String)` fallback.**
   Grep for any remaining `PhpType::Raw` usage. Each instance is a
   type expression we haven't properly handled yet. Either add
   support or document it as a known limitation.

8. **Final test suite run.** Everything green = migration complete.

---

## Testing strategy

Each migration step must leave the test suite green. The primary
verification tools are:

- **`tests/` fixture runner** — end-to-end tests that exercise
  completion, hover, go-to-definition, diagnostics, code actions,
  and signature help against PHP fixture files. These are the
  ultimate correctness check.

- **Unit tests** — each new module (`php_type.rs`, `names.rs`,
  `composer_mago.rs`, docblock adapter) gets its own unit tests
  covering round-trip correctness and edge cases.

- **`example.php`** — the project's showcase file that exercises
  every type intelligence feature. Open it in an editor after each
  phase and verify that completion, hover, and go-to-definition
  work as expected.

For M4 specifically, the dual-representation approach (Phase 2)
means we can add structured types without removing string types.
This gives us a safety net: if a structured-type consumer produces
wrong results, we can compare against the string-type consumer to
debug.

---

## Version alignment

All mago crates should be pinned to the same version. Current
dependencies use 1.14. The new crates (`mago-composer`,
`mago-docblock`, `mago-type-syntax`, `mago-names`) should use the
same version. If upgrading is needed to get bug fixes or features,
upgrade all mago crates together in a single commit.

---

## What this enables

Once the migration is complete, several roadmap items become
significantly easier:

| Roadmap item | How the migration helps |
| --- | --- |
| T7 (`key-of<T>` / `value-of<T>`) | `PhpType::KeyOf` and `PhpType::ValueOf` are first-class variants — just add resolution logic. |
| T2 (`@phpstan-type` aliases) | Type aliases map a name to a `PhpType`. Substitution is a tree transform. |
| T3 (`@phpstan-import-type`) | Same — cross-file alias resolution becomes FQN lookup + `PhpType` storage. |
| C2 (`#[ArrayShape]` return shapes) | Build a `PhpType::Shape` directly from attribute data. |
| L1 (Facade completion) | Preserve full `PhpType` from the concrete class instead of flattening to `@method static` strings. |
| L4 (Custom Eloquent builders) | Resolve `HasBuilder<X>` as `PhpType::Reference { generic_args: [X] }` and extract X structurally. |
| BL1 (Blade support) | Blade variable types (`$loop` as `object{index: int, ...}`) are `PhpType::Shape` values, not hand-built strings. |
| Sprint 6 (Type intelligence depth) | Every item in Sprint 6 involves type manipulation. Structured types make all of them easier and more correct. |

The migration is a one-time cost that pays dividends across every
future sprint.