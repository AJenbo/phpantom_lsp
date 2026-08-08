# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B65. A `@var` whose type is a closure signature binds the wrong variable

**Impact: Low-Medium · Effort: Low**

A closure type writes `$`-prefixed names for its own parameters, and the
`@var` scan takes the first `$` it finds after the tag as the annotated
variable:

```php
/** @var \Closure(\App\Models\User $user): string $callback */
```

is read as declaring `$user` of type `\Closure(\App\Models\User`, so
`$callback` stays untyped and a bogus `$user` enters the scope. The same
shape appears in `@param` and in a Blade template's signature docblock,
where it also decides which names the contract declares.

**Where to look:** `parse_var_docblock_pairs` in
`type_engine/variable/forward_walk/assignment.rs` scans for the first
`$` after `@var`. The annotated variable is the `$name` at paren depth 0
and angle depth 0, so the scan has to track both while walking the type
rather than stopping at the first `$`.

#### B66. `#[AsCommand]` outranks the signature that actually names the command

**Impact: Low-Medium · Effort: Low**

`command_from_class` reads the command name from `#[AsCommand]` first
and only falls back to `$signature` / `#[Signature]` / `$name`. At
runtime the order is the reverse: `Command::__construct()` uses the
signature whenever one is set and never reaches Symfony's
`getDefaultName()`, and when there is no signature it passes `$this->name`
to the parent, which again wins over the attribute. So a class that
carries both is indexed under a name Artisan does not answer to:

```php
#[AsCommand(name: 'x:from-as-command')]
class Sync extends Command
{
    protected $signature = 'x:from-property {user}';   // Artisan registers this one
}
```

The command is then reported as unknown at every call site that spells
it the way Artisan does, and go-to-definition on the spelling we did
index lands on the attribute rather than the signature.

**Where to look:** the three-branch chain in `command_from_class`
(`src/virtual_members/laravel/commands.rs`). The branches need to run in
the runtime order (signature, then `$name`, then `#[AsCommand]`), which
also removes the case where `CommandEntry::name` comes from the
attribute while `CommandEntry::signature.name` comes from the signature.
