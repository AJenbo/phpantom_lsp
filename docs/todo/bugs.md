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


#### B3. A property write overrides the declared type even when it goes through `__set`

**Impact: Low-Medium · Effort: Medium**

`$obj->prop = expr` records the assigned type under a property-path key
(`assignment.rs`, the `Expression::Access(Access::Property(_) | ...)`
branch) so that a later read of the same path resolves through the
assignment "rather than the declaring class's declared property hints".
That is right for a real declared property, and it is what makes
`stdClass` chains resolve. It is wrong when the write dispatches to
`__set`, which is free to transform, reroute, or drop the value instead
of storing it as given. `__get` then decides what a read returns, and the
recorded write type has no authority over it.

```php
/** @template TData of array */
class DataBag {
    /** @param TData $data */
    public function __construct(private array $data) {}
    /** @return TData[K] */
    public function __get(string $property) { return $this->data[$property]; }
    /** @param TData[K] $value */
    public function __set(string $property, $value) { /* may store anything */ }
}

/** @extends DataBag<array{a: int, b: string}> */
class FooBag extends DataBag {}

$foo = new FooBag(["a" => 5, "b" => "hello"]);
$foo->a = 9;
$a = $foo->a;   // PHPantom: 9      Psalm: int (via __get's TData[K])
```

The write should not be recorded when the property is undeclared on the
subject's class *and* that class declares `__set`; the read should keep
resolving through `__get`. Psalm and PHPStan both treat magic property
writes as opaque for this reason.

This predates literal preservation in expression resolution: the recorded
type used to coincide with the declared type (`int` either way), so the
override was invisible. It surfaced when the write started recording `9`.

**Where to look:** the property-assignment branch of
`type_engine/variable/forward_walk/assignment.rs`, and the magic-member
resolution in `virtual_members/`.

**Test:** the `inheritTemplateParamViaConstructor` case in
`tests/psalm_assertions/template_class_template_extends.php` carries two
`// SKIP` markers that clear when this is fixed.

#### B4. `analyze` reports Blade type diagnostics six lines too early

**Impact: Medium · Effort: Low**

Diagnostics that a Blade template reports through the type-mismatch
passes land six lines above the code that produced them, so the CLI
points at unrelated markup:

```
resources/views/pages/invoice.blade.php
  545   Argument 1 ($amount) expects Acme\Decimal\Decimal, got Decimal<bool>
```

The call is on line 551. The offset is exactly `blade::PROLOGUE_LINES`,
and it is a double translation. `Backend::offset_range_to_lsp_range`
(`diagnostics/mod.rs`) already maps a Blade file's virtual-PHP range back
to Blade coordinates via `map.php_to_blade`, and then `analyse/run.rs`
maps the same range a second time:

```rust
let line = if let Some(ref map) = source_map {
    map.php_to_blade(d.range.start).line + 1
} else {
    d.range.start.line + 1
};
```

The Laravel string-key diagnostics (`invalid_laravel_view`,
`invalid_laravel_route`, …) are unaffected because they build their range
with the *free* `offset_range_to_lsp_range(content, …)`, which does no
Blade translation, so `run.rs` supplies the only one they need. Every
pass that goes through the `&self` method — the three `type_mismatch_*`
collectors, `unknown_member`, `unknown_variable` — is shifted.

The LSP path does not double-translate, so this is `analyze`-only.

The fix is to pick one owner for the translation. Having every collector
emit Blade coordinates (what the `&self` method already does) and
dropping the `php_to_blade` call in `run.rs` is the smaller change, but
it needs an audit that no collector still emits virtual-PHP coordinates.

**Where to look:** `src/analyse/run.rs` (the `source_map` branch),
`Backend::offset_range_to_lsp_range` and the free
`offset_range_to_lsp_range` in `src/diagnostics/mod.rs`.

**Reproduce:** point `analyze` at any project whose Blade templates
trigger a `type_mismatch_argument`, and compare the reported line with
the source.

#### B5. "Promote constructor param" leaves an orphaned docblock behind

**Impact: Low · Effort: Low-Medium**

The "Promote property to constructor parameter" code action deletes only
the property declaration's own line, not a `/** @var ... */` docblock
that precedes it:

```php
class Foo {
    /** @var int */
    private int $bar;   // deleted

    public function __construct(private int $bar) {}   // promoted here
}
```

Applying the action leaves the now-orphaned `/** @var int */` sitting
above the constructor. `property_delete_start`
(`code_actions/promote_constructor_param.rs`) is computed with
`find_line_start`, which only walks back to the previous newline on the
same line — it cannot reach a comment on an earlier line, by design (it
is also used for the non-docblock leading-whitespace case). There is no
current test covering a property with a preceding docblock.

**Where to look:** `build_promotion_candidate` in
`code_actions/promote_constructor_param.rs` — extend
`property_delete_start` to also swallow an immediately-preceding
docblock (mirroring how other code actions in this module, e.g.
`update_docblock.rs`, locate an existing docblock above a declaration).

#### B6. "Add `final`" may insert before a class's opening brace instead of its constructor (needs confirmation)

**Impact: Low-Medium · Effort: Medium**

Found by manual trace while cleaning up comments in
`code_actions/phpstan/new_static.rs`; not yet confirmed with a failing
test, so verify before fixing.

`build_constructor_info` locates where to insert `final ` before an
unmodified constructor. For a constructor with no explicit visibility
keyword, where the enclosing class's opening brace sits alone on its
own line (Allman style):

```php
class Foo
{
    function __construct() {}
}
```

Tracing the offsets by hand: `before_kw.trim_end().rfind('\n')` (line
~516) skips backward past the blank run of whitespace between the
brace line and the `function` keyword's line, landing right *before*
the `{` rather than at the start of `    function …`. The next check
(`line_trimmed.is_empty()`, line ~520-523), meant to detect "no
modifiers on this line, fall back further", instead sees the lone `{`
character as if it were modifier text on the same line as `function`
and skips the fallback branch. The final `decl_start` then points at
the `{` character itself, so inserting `final ` would produce
`class Foo\nfinal {\n    function __construct() {}\n}` — `final`
attached to the brace line, not the constructor — which is invalid
PHP.

**Where to look:** `build_constructor_info` in
`code_actions/phpstan/new_static.rs`. The line/modifier detection at
~514-531 needs to distinguish "text on the function's own line" from
"trailing content from an earlier line that `trim_end()` bridged past
a blank line" — e.g. by checking whether `decl_start_offset` and
`func_kw_offset` are on the same source line (count newlines between
them) rather than only checking whether the slice between them is
non-empty after trimming.

**To confirm:** add a test with a modifier-less constructor whose
class brace is on its own line (as above) and assert the insertion
point lands on the `function` line, not the brace line.

#### B7. Eloquent string-completion's backward paren scan has no bound

**Impact: Low · Effort: Low**

Found while cleaning up comments in `completion/eloquent_string.rs`.
`detect_string_call_context` calls `find_matching_open_paren` on
`content[..quote_pos]` (everything in the file before the cursor's
opening quote) with no length cap. The function scans backward byte by
byte tracking bracket depth; if the preceding source has an unbalanced
`)`/`]` before the real matching `(` — e.g. the cursor's string
literal is itself inside a malformed/mid-edit expression — depth never
returns to 0 and the scan runs all the way to the start of the file.

This mirrors a case already fixed elsewhere in the codebase: the
`function`-keyword backward scan in
`code_actions/phpstan/add_throws.rs`'s `find_enclosing_docblock` had
the same unbounded-backward-scan shape and was capped at 2000 bytes.
`find_matching_open_paren` has no equivalent cap, and it is on the
completion hot path (triggered on every keystroke inside a string
literal argument), so a large file with an unbalanced bracket before
the cursor pays for an O(file size) scan on every trigger.

**Where to look:** `find_matching_open_paren` in
`completion/eloquent_string.rs` — give the backward scan a byte-offset
floor (matching the pattern and rationale already used in
`add_throws.rs`) so pathological/mid-edit source can't force an
unbounded scan.

#### B8. `extends` keyword completion offered inside `enum` declaration headers

**Impact: Low · Effort: Low**

Found while cleaning up comments in `completion/context/keyword_completion.rs`.
`build_keyword_context` sets
`in_extends_declaration_header: decl_kind.is_some()`, which is `true`
whenever `decl_kind` is `Class`, `Interface`, *or* `Enum`. PHP enums
cannot use `extends` (only `implements`), so typing `enum Foo ext|`
offers an `extends` completion that will always produce invalid PHP.
`in_implements_declaration_header` a few lines below already gets this
right, restricting itself to `Class | Enum`.

**Where to look:** `build_keyword_context` in
`completion/context/keyword_completion.rs` — change
`in_extends_declaration_header` to
`matches!(decl_kind, Some(DeclarationHeaderKind::Class | DeclarationHeaderKind::Interface))`,
matching the pattern already used for `in_implements_declaration_header`.
No existing test exercises this path; add one for `enum Foo ext|` (no
completion) and `interface Foo ext|` (completion offered, extends
existing coverage of the Class case).
