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

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

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

### B176. Find References on a global constant always includes its declaration

**Impact: Low · Effort: Medium**

`find_constant_references` ignores its `include_declaration` parameter:
every match is a `SymbolKind::ConstantReference` span, and unlike
`SymbolKind::FunctionCall` (which carries `is_definition`),
`ConstantReference` does not distinguish a constant's declaration site
(`const BAR = 1;`) from a use of it (`echo BAR;`) -- both are extracted
with the same span kind (`symbol_map/extraction/statements.rs`). So a
"Find References" request with `includeDeclaration: false` still
returns the declaration line for a global constant, unlike functions,
methods, and properties.

Fixing this properly means giving `ConstantReference` an `is_definition`
flag the way `FunctionCall` has one, set at the declaration site in
`extract_from_statement`'s `Statement::Constant` arm and cleared
everywhere else `ConstantReference` is emitted (`Expression::ConstantAccess`,
the `use const` import case in `class_like.rs`), then having
`find_constant_references` skip definition spans when
`include_declaration` is `false`. That touches the span extraction,
the reference index (`reference_index.rs` pattern-matches
`ConstantReference` by name only) and every other match on the enum
variant, so it is not a one-line change.
