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

### B196. The chain resolution cache key omits file identity

**Impact: Low-Medium · Complexity: Medium**

The chain cache in `type_engine/resolver/mod.rs` keys a variable-free
subject chain by its bare text (`expr.to_subject_text()`), with no file
identity in the key, while some cache activations span multiple files
(e.g. the reference-counts pending-item loop, and the request-level
guard used by find-references/rename while walking other files). Two
files that each `use` a different class under the same alias and spell
the same method-chain text can have the second file's resolution
poisoned by the first file's cached entry:

```php
// file A: use A\Pen;           file B: use B\Pen;
Pen::make()->write();           Pen::make()->write();  // may resolve against A\Pen
```

### B218. `new ReflectionProperty(Foo::class, 'bar')` forgets what it reflects

**Impact: Low · Complexity: Medium**

A reflection value built by `ReflectionClass::getProperty('bar')` carries
the class and the property name, so reading it types as the property
declares. Constructing the same value directly does not:

```php
$viaClass = (new \ReflectionClass(Configuration::class))->getProperty('shell');
$viaClass->getValue($config);   // ?Shell

$direct = new \ReflectionProperty(Configuration::class, 'shell');
$direct->getValue($config);     // mixed
```

The two spellings are interchangeable in real code, so the second should
resolve like the first. The binding cannot come from the constructor's
docblock: `class-string<T>|T $class` would bind the class through the
existing machinery, but the `$property` name is a string literal, and a
literal only binds to a `@template` whose bound is a type operator
(`key-of<…>` and friends). Either the two `new`-expression resolution
paths need the same rule the two call paths got, or literal binding has
to be widened to a `@template TName of string`, which is what PHPStan
does for literal string types and would want measuring against the whole
corpus first.

### B226. A function-`static` variable's type is not tracked across its own reads

**Impact: Low-Medium · Complexity: Medium-High**

`type_engine/variable/forward_walk/` has no handling at all for a
`static $var;` declaration (there is no `StaticVariable` case anywhere
under it); the walker treats the name as an ordinary, unassigned local
until it sees an assignment to it in the same top-to-bottom pass. That
loses the one thing a `static` local actually means: its value can carry
over from an *earlier call* that assigned it in a branch the current
call never reaches.

```php
function info(?Configuration $config = null) {
    static $lastConfig;
    if ($config !== null) {
        $lastConfig = $config;
        return null;
    }
    $config = $lastConfig ?: new Configuration();
    // $shell::VERSION below needs $config resolved to Configuration for
    // Sudo::fetchProperty($config, 'shell') to type as ?Shell (the
    // pass-through accessor inference already handles that part).
    $shell = Sudo::fetchProperty($config, 'shell');
    if ($shell) {
        $shellInfo = ['PsySH version' => $shell::VERSION];
    }
}
```

On the call that falls through to the second half, `$lastConfig` is read
without ever having been assigned within *this* walk, so `$config`
resolves too conservatively for the accessor pass-through (see the
`ReflectionProperty`/`Sudo::fetchProperty` inference above) to carry
`Configuration::$shell`'s declared type through to `$shell`, and
`$shell::VERSION` cannot be resolved. Found via
`php-typing-conformance`'s LSP navigation probe against psysh
(`Psy\Shell::VERSION`, `src/functions.php:383`): find-references reports
20 of 21 known references, missing exactly this one; Intelephense and
Phpactor miss the same reference, but DEVSENSE resolves it, which is
worth chasing. The same gap also degrades hover and inferred types
wherever code narrows on a `static` local this way, not just
find-references. A correct fix needs to seed a
`static $var`'s type from the union of every assignment reachable
anywhere in the enclosing function body (not only the ones preceding the
read in this pass), since the assignment that matters can sit in a
branch this call never takes.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.

## Miscellaneous

No outstanding items.
