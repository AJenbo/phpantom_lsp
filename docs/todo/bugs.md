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

Most entries below come from the 2026-08-13 sample-project sweep (345
diagnostics across ten projects, ~330 of them false positives). Site
counts refer to that sweep; the git-ignored triage log has the full
per-project inventory. Entries filed later say where they came from.

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Narrowing

The five entries below were all found the same way, on 2026-08-14: with
the nullable escape hatch in `is_type_compatible` retired, a `?T`
argument is judged like the `T|null` it stands for, and every null the
walker fails to narrow away now surfaces as a diagnostic. Each one is a
guard the source really writes and we really lose. Site counts are from
a sweep of the ten sample projects on the day the hatch went.

### B153. Static properties get no flow tracking at all

**Impact: Medium-High · Effort: Medium**

Nine sites, the largest single group. Neither an assignment nor a check
is remembered for `self::$x` / `static::$x` / `Foo::$x`, so the lazy-init
idiom reports a null that cannot reach the read:

```php
private static ?Repo $repo = null;

public static function repo(): Repo
{
    if (self::$repo === null) {
        self::$repo = new Repo();
    }

    return self::$repo;      // reported: ?Repo
}
```

`self::$x = 'a'; return self::$x;` is enough on its own — the assignment
alone is not recorded either. The root is that
`narrowing::expr_to_subject_key` (`type_engine/types/narrowing/resolve.rs`)
returns `None` for `Access::StaticProperty`, so a static property is
never a narrowing subject; the read side (`rhs_resolution/property_access.rs`)
also resolves straight from the declaration without consulting the scope.
Both halves have to land together, along with invalidation, for the key
to mean anything.

### B154. An assignment inside a `try` block is lost after the block

**Impact: Medium · Effort: Low-Medium**

```php
if (!$h) {
    try {
        $h = new Holder();
    } catch (RuntimeException) {
        throw new LogicException('x');
    }
}

return $h;                   // reported: ?Holder
```

Without the `try` the same code is narrowed correctly, so what is lost is
the assignment's effect on the scope the `try` statement leaves behind,
not the guard.

### B155. A `&&` chain inside a `match` arm does not narrow its own operands

**Impact: Medium · Effort: Low-Medium**

```php
return match ($kind) {
    1       => $this->a && $this->b && $this->same($this->a),  // reported: ?Holder
    default => true,
};
```

The same chain written as a `return` statement narrows. Match arms record
a scope snapshot per arm, but the arm body's own `&&` operands are not
narrowed against each other inside it.

### B156. `$x === Enum::Case` does not remove null from `$x`

**Impact: Medium · Effort: Low**

```php
return $land === Land::Be && $this->takes($land);   // reported: ?Land
```

Identity against an enum case proves the subject is that case, so
everything else in its union — `null` included — is ruled out for the
rest of the chain.

### B157. A `do`/`while` condition does not narrow its own operands

**Impact: Low-Medium · Effort: Low-Medium**

```php
do {
    $expr = $this->parseOptionalExpression();
} while ($expr && $this->addChildToList($list, $expr));   // reported: ?ASTNode
```

The `&&` narrowing that an `if` condition performs is not applied in a
loop condition.

## Symbol resolution

### B158. A namespaced constant is only found under its bare name

**Impact: Medium-High · Effort: Medium**

`extract_defines_from_statements` (`parser/functions.rs`) registers a
namespace-level `const` under the name as written, dropping the
namespace it sits in, so `Demo\Scaffolding\GRADES` is stored as
`GRADES`. Every reference that names the namespace therefore finds
nothing, while the bare reference finds it:

```php
namespace App;

use App\Config;
use const App\Config\GRADES;

in_array($g, GRADES, true);          // resolved
in_array($g, Config\GRADES, true);   // not resolved
in_array($g, \App\Config\GRADES, true); // not resolved
```

The value is what the narrowing, hover and completion paths read, so a
qualified reference silently loses whatever the constant proves. It is
visible in `examples/php/completion.php`, where the `in_array($grade,
Scaffolding\GRADES, true)` gate cannot narrow the `?string` away and the
return is reported as a mismatch — the demo is right and the resolution
is not.

Storing the fully-qualified name is only half of it: a bare reference
has to keep resolving, which in PHP means trying the current namespace
first and the global one after, and a qualified one has to go through
the file's `use` table the way a class name does. The function index
made the same choice deliberately (see the comment about short-name
collisions in `parser/ast_update.rs`), so follow it rather than adding a
short-name fallback entry.

## Array types

No outstanding items.
