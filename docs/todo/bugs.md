# PHPantom — Bug Fixes

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

---

#### B21. Builder `__call` return type drops chain type for dynamic `where{Column}` calls

| | |
|---|---|
| **Impact** | Medium |
| **Effort** | Medium |

Eloquent's `Builder::__call()` intercepts calls like `whereColumnName()`
and forwards them to `where('column_name', ...)`. PHPantom correctly
suppresses the "unknown member" diagnostic because `Builder` has `__call`,
but the return type of the magic call is lost. This breaks every
subsequent link in the chain.

**Reproducer:**

```php
// SubcategoryView has scopeWhereLanguage but NOT scopeWhereSubcategoryId.
// whereSubcategoryId() is a dynamic where{Column} via Builder::__call.
$view = SubcategoryView::whereLanguage($lang)
    ->whereSubcategoryId($id)   // accepted (Builder has __call), but return type lost
    ->first();                  // diagnostic: "subject type could not be resolved"
```

**Expected:** When `__call` is invoked on an Eloquent `Builder<TModel>`,
the return type should be `Builder<TModel>` (i.e. `$this`), preserving
the generic parameter so the chain continues resolving.

**Root cause:** `has_magic_method_for_access` in `unknown_members.rs`
correctly detects `__call` and suppresses the diagnostic, but the
call-resolution pipeline in `call_resolution.rs` does not attempt to
derive a return type from `__call`. For Eloquent builders specifically,
any unrecognised instance method call should return `$this` (the
builder), since nearly all `Builder` methods are fluent.

**Where to fix:**
- `src/completion/call_resolution.rs` — when resolving a method call
  that is not found on the class but the class has `__call`, check
  whether the class is an Eloquent `Builder` (or extends one). If so,
  return `$this` / the builder type as the call's return type instead
  of giving up.
- Alternatively, add a fallback in `resolve_method_return_types_with_args`
  that checks for `__call` and uses its declared return type (or `$this`
  for known builder classes).

**Impact in shared codebase:** ~5 diagnostics (direct chain breaks after
dynamic `where{Column}` calls, plus downstream cascading failures).

**Discovered in:** analyze-triage iteration 10.