# PHPantom — Bug Fixes

#### B18. Assignment inside `if` condition does not resolve variable in body

| | |
|---|---|
| **Impact** | Medium |
| **Effort** | Medium |

When a variable is assigned inside an `if` condition
(`if ($x = expr())`), PHPantom does not resolve `$x` inside the
`if` body at all. The variable shows as completely untyped.

**Reproducer:**

```php
if ($admin = AdminUser::first()) {
    $admin->assignRole($role);
    // PHPantom reports: Cannot resolve type of '$admin'
}
```

**Expected:** `$admin` should resolve to `AdminUser` (narrowed from
`AdminUser|null` by the truthiness check).

Splitting into two statements works fine:

```php
$admin = AdminUser::first();
if ($admin) {
    $admin->assignRole($role);  // resolves correctly
}
```

**Where to fix:**
- `src/completion/variable/resolution.rs` — `walk_if_statement` and
  `check_expression_for_assignment` need to handle assignment
  expressions used as `if` conditions, extracting the assignment
  and treating the variable as defined from that point forward.

**Discovered in:** analyze-triage iteration 9 (DatabaseSeeder.php:59).

---

#### B19. Nullable return type `TValue|null` drops `|null`

| | |
|---|---|
| **Impact** | Low |
| **Effort** | Low |

When a method returns `TValue|null` (e.g. Eloquent `first()`), the
resolved type drops the `|null` component. `AdminUser::first()`
shows as `AdminUser` instead of `?AdminUser`.

**Reproducer:**

```php
// Builder::first() has @return TValue|null
$admin = AdminUser::first();
// Hover shows: AdminUser
// Expected:    ?AdminUser
```

**Where to fix:**
- Likely in the template substitution or return type resolution
  pipeline. When `TValue` is substituted with `AdminUser` in a
  `TValue|null` union, the `|null` member may be discarded because
  it does not resolve to a class.

**Discovered in:** analyze-triage iteration 9 (DatabaseSeeder.php:57).

---

#### B20. Loop-body assignments not visible to null narrowing for null-initialized variables

| | |
|---|---|
| **Impact** | Low |
| **Effort** | Medium |

When a variable is initialized as `null` and reassigned inside a
`foreach` or `while` loop body, the assignment type does not flow
into the variable's resolved type for code inside the same loop
iteration. The variable stays typed as `null` even though it was
assigned a class instance on a previous iteration.

**Reproducer:**

```php
$lastPaidEnd = null;

foreach ($periods as $period) {
    if ($lastPaidEnd !== null && $lastPaidEnd->diffInDays($periodStart) > 0) {
        // Hover on $lastPaidEnd shows `null` — the Carbon assignment
        // from the previous iteration is lost.
    }
    $lastPaidEnd = $period->ending->startOfDay();
}
```

**Expected:** `$lastPaidEnd` should resolve to `null|Carbon` (the
union of the initial `null` and the loop-body assignment). The
`!== null` narrowing (B17, now fixed) would then strip `null`,
leaving `Carbon` on the right side of `&&`.

**Actual:** The loop-body assignment (`$lastPaidEnd = $period->ending->startOfDay()`)
appears after the cursor position within the same iteration, so
the variable resolution walker never picks it up. The resolved
type is just `null`.

**Root cause:** `walk_statements_for_assignments` walks statements
sequentially and only accumulates assignments that appear before
the cursor offset. Inside a loop body, an assignment that appears
textually after the cursor can still be live on subsequent
iterations. The walker would need a second pass (or a pre-scan of
the loop body) to collect all assignments within the loop and
union them into the variable's type for any position inside the
loop.

**Where to fix:**
- `src/completion/variable/resolution.rs` — in `walk_foreach_statement`
  and `walk_while_statement`, pre-scan the loop body for assignments
  to the target variable and merge their types into the result set
  before walking the body normally. This ensures that even assignments
  after the cursor contribute to the union type.

**Discovered in:** B17 QA (sandbox.php cases 1, 2, 8).