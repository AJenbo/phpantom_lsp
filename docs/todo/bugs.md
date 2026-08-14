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

No outstanding items.

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
