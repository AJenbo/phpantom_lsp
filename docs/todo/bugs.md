# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B1. `analyze` flags framework Artisan commands as unknown

**Impact: Medium · Effort: Low**

`phpantom_lsp analyze` reports `Unknown command: 'queue:work'` for a
command the framework itself ships. Reproduce by adding
`Artisan::call('queue:work');` to `examples/laravel/app/Demo.php` and
running a **release** build of
`analyze --project-root examples/laravel` (a debug build runs a reduced
collector list that excludes the Laravel string-key checks, so the
false positive is invisible there).

The cause is which files populate the command index. The LSP's
`initialized` handler calls `build_laravel_command_index`, which scans
the whole FQN → URI index — vendor packages included. The headless
pipeline in `analyse/run.rs` never calls it, and instead gets entries
only from `refresh_laravel_command_index`, which `update_ast` fires per
parsed file. `analyze` parses user files, so the index ends up holding
the project's own commands and nothing else. Since it is then non-empty,
the "index is empty, discovery must have failed" guard in
`collect_invalid_laravel_string_key_diagnostics` does not fire, and every
vendor command name is reported as unknown.

`build_laravel_macro_index` is missing from the same block, with the same
shape of consequence: only macros registered in parsed user files are
recovered, so a macro registered by a vendor package's service provider
can produce a false-positive unknown member.

**Where to change:** call `build_laravel_command_index` and
`build_laravel_macro_index` from the Laravel block in `analyse/run.rs`,
next to `build_laravel_date_class`, `build_provider_resources`, and
`build_laravel_morph_map_index`.


#### B2. A property assigned inside a guarded `if` keeps its declared type after the block

**Impact: Medium · Effort: Medium**

The lazy-initialisation idiom leaves a property at its declared type
once the `if` closes, so returning it from a method with a narrower
return type is reported as a mismatch:

```php
protected ?AbstractType $instance = null;

public function getType(): ConcreteType
{
    if (!$this->instance instanceof ConcreteType) {
        $this->instance = $this->context->makeConcrete();  // : ConcreteType
    }

    return $this->instance;   // false positive: ?AbstractType
}
```

Both paths out of the `if` give `ConcreteType`: the implicit else is the
negation of the condition, and the then-branch assigns one. The two need
to be merged the way the forward walker already merges branch outcomes
for local variables. Property keys are seeded into the walker's scope for
`instanceof` narrowing (`seed_property_keys_into_scope`), so the missing
piece is recording a property *assignment* into that scope and joining
the branches at the end of the block, not new machinery.

Surfaced by removing the supertype-where-subtype escape hatch from the
argument/return compatibility layer, which is what made the stale type
visible. Reproducible in an open-source project: PDepend's
`ASTClassReference::getType()` and `ASTTraitReference::getType()` are the
two remaining diagnostics `analyze` reports there, and PHPStan at level
max reports neither.

**Where to look:** `type_engine/variable/forward_walk/` (branch merging
and `seed_property_keys_into_scope`) and
`type_engine/resolver/property_narrowing.rs`.
