# PHPantom — Bug Fixes

#### B17. `&&` short-circuit narrowing does not eliminate `null` for null-initialized variables

| | |
|---|---|
| **Impact** | Low |
| **Effort** | Low |

When a variable is initialized as `null` and later guarded by
`$var !== null &&` in the same `if` condition, PHPantom still
resolves the variable as `null` on the right side of `&&`,
producing a `scalar_member_access` diagnostic.

**Reproducer:**

```php
$lastPaidEnd = null;

foreach ($periods as $period) {
    if ($lastPaidEnd !== null && $lastPaidEnd->diffInDays($periodStart) > 0) {
        // PHPantom reports: Cannot access method 'diffInDays' on type 'null'
    }
    $lastPaidEnd = $period->ending->startOfDay();
}
```

**Expected:** The `!== null` check on the left side of `&&` should
narrow away `null` for the right side, resolving `$lastPaidEnd` to
`Carbon` (or whatever the reassignment type is).

**Root cause:** This is the remaining gap from the earlier B11 fix
(null-init + guard clause). The partial fix handled early-return
guard clauses (`if ($x === null) return;`) but does not handle
`&&` short-circuit narrowing where the null check and the member
access are in the same compound condition.

**Where to fix:**
- `src/completion/types/narrowing.rs` — extend the `&&` narrowing
  logic to propagate `!== null` / `!is_null()` guards to later
  operands in the same compound condition.

**Discovered in:** analyze-triage iteration 8 (CustomerService.php:302).

---

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