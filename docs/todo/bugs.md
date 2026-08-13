# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

Each entry below carries an **Impact · Effort** rating using the same
scale defined in [`docs/todo.md`](../todo.md); that table is also where
each bug's row lives in the current sprint/backlog.

### B96. A docblock `@param` type narrower than its native nullable type hint is not flagged

**Impact: Low · Effort: Medium**

```php
/**
 * @param string $name
 */
function greet(?string $name): void {}

/**
 * @param list<int> $items
 */
function takesItems(?array $items): void {}
```

The native type hint (`?string`, `?array`) is nullable, but the docblock says
a narrower type (`string`, `list<int>`) that does not admit `null` — a caller
following the native signature can pass `null` and violate the docblock's own
contract. PHPantom has no diagnostic for a docblock parameter/return type
that is narrower than the native type hint it annotates, for either a scalar
or an array type. Only Qodana flags either case in
`php-typing-conformance`'s corpus.

**Fix:** not investigated; would need a new check (there is no existing
"docblock narrower than native hint" diagnostic to extend) comparing each
documented parameter/return type against its native type hint for
compatibility, most likely reusing the existing type-compatibility check
rather than a new one.

### B97. `CONSTANT[T]` reads as the whole table when `T` comes from a parameter's default value

**Impact: Low-Medium · Effort: Medium**

```php
const ID_TABLE = ['immutable' => 1, 'mutable' => 'two'];

/**
 * @template T of key-of<ID_TABLE>
 * @param T $type
 * @return ID_TABLE[T]
 */
function lookUp(string $type = 'immutable'): int|string {
    return ID_TABLE[$type];
}

takesInt(lookUp('immutable')); // passes: correctly reads as int
takesInt(lookUp());            // reported: got 1|'two' — should also read as int
```

Per-key resolution of `CONSTANT[T]` (a template bound to `key-of<CONSTANT>`)
now works correctly when the call site passes the key as an explicit literal
argument, but falls back to the whole table's value union specifically when
the caller omits the argument and the template binds from the parameter's
*default* value instead. The default value (`'immutable'`) is known at
the declaration site the same way an explicit argument is known at the call
site, so `lookUp()` should resolve identically to `lookUp('immutable')`.

**Fix:** wherever the explicit-argument case resolves `T` to the literal
passed at the call site, apply the same resolution when the argument is
omitted and a literal default value is available.

### B98. Array-index access into a relation collection loses its element type across a multi-hop relation chain

**Impact: Low-Medium · Effort: Medium**

```php
// $product->subcategories: HasMany<Subcategory, $this>
foreach ($product->subcategories as $subcategory) {
    // $subcategory->category: BelongsTo<Category, $this>
    // Category::$translations: HasMany<CategoryTranslation, $this>
    if (!$subcategory->category->translations[0]) {
        continue;
    }
    echo $subcategory->category->translations[0]->name; // unresolved_member_access
}
```

A single-hop relation collection index (`$model->children[0]->name`) resolves
fine: `build_property_type()` in
`src/virtual_members/laravel/relationships.rs` types the property as
`Collection<Related>`, and `extract_value_type()` in `src/php_type/mod.rs`
correctly reads the generic's element type on indexing. But when the
collection property is reached through a chain of two or more relation hops
(`$subcategory->category->translations[0]`, where both `category` and
`translations` are themselves relation-backed virtual properties), indexing
the final collection reports `Cannot verify property 'name' — type of
'...' could not be resolved` instead of resolving to the related model.
Reproduced in `projects/luxplus-shared/src/core/Exports/ProductExport.php`
(7 occurrences across lines 51-68, all on the same chained pattern; the
`foreach` loop variable itself, `$subcategory`, resolves correctly, so the
loss happens specifically at the second relation hop plus array index, not at
the `foreach` element type).

**Fix:** not investigated. Likely the resolver used for
`$obj->relationA->relationB[N]` (property access chained off a relation
property, then indexed) does not re-run the same relation virtual-property
synthesis on the intermediate `relationA` result before resolving
`relationB`, unlike the single-hop case. Bisect against a minimal
`$a->one->many[0]` repro to confirm where the type is dropped.

