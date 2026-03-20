# PHPantom — Bug Fixes

Known bugs and incorrect behaviour. These are distinct from feature
requests — they represent cases where existing functionality produces
wrong results. Bugs should generally be fixed before new features at
the same impact tier.

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## B14. Redundant file re-parsing in unknown-member diagnostics

- **Impact:** Medium · **Effort:** Medium-High

The subject deduplication cache (per-pass `SubjectCache`) eliminated
the worst case where identical subjects were resolved hundreds of
times. However, each *unique* subject that goes through variable
resolution still calls `with_parsed_program`, which re-parses the
entire file from scratch. For unresolved subjects, the secondary
helpers (`resolve_scalar_subject_type`,
`resolve_unresolvable_class_subject`) add further re-parses. A
single unique untyped variable subject can trigger up to 6 full
re-parses of the file.

In files with many distinct variable subjects (e.g. different
`$var1->`, `$var2->`, `$var3->` accesses), the parsing cost still
adds up even with the subject cache.

### Fix — parse caching within a diagnostic pass

The file content is immutable during a single diagnostic pass.
Caching the parsed `Program` AST once and threading it through the
resolution calls would eliminate all redundant parsing, reducing
even the per-unique-subject cost. This is a larger refactor because
`with_parsed_program` is used across many modules and the `Program`
type borrows from a `bumpalo::Bump` arena that must stay alive.

---

## B15. Array element from method return chain not resolved in diagnostics

- **Impact:** Low · **Effort:** Low

When a method returns an array type (e.g. `@return Item[]`) and the
caller indexes into it inline (`$c->items()[0]->getLabel()`), the
diagnostics pipeline fails to resolve the element type. The subject
`$c->items()[0]` is a `CallExpr` followed by array access, but
`resolve_target_classes` does not thread the array element type
through the method-return → index → member chain.

The same pattern works when the array is assigned to a variable
first (`$items = $c->items(); $items[0]->getLabel()`), because the
variable resolution path handles `@var` annotations on the
intermediate variable. Only the inline form is broken.

### Reproduction

```php
class Item {
    public function getLabel(): string { return ''; }
}
class Collection {
    /** @return Item[] */
    public function items(): array { return []; }
}
function test(): void {
    $c = new Collection();
    // ✗ subject type not resolved — false "cannot verify" warning
    $c->items()[0]->getLabel()->nonexistent();
}
```

### Fix

When `resolve_target_classes_expr` encounters an `ArrayAccess` whose
base is a `CallExpr`, resolve the call's return type, strip one
array dimension, and use the element type as the resolved class.
This is the same logic that already exists for `ArrayAccess` on
bare variables — it just needs to be extended to call-expression
bases.