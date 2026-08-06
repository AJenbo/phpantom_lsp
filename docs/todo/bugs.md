# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B20. A class constructed from a string literal picks up a `bool` generic argument

**Impact: Medium · Effort: Medium**

`new \Acme\Decimal\Decimal('0.00')` resolves to `Decimal<bool>`, and the
value is then rejected against a plain `Decimal` parameter:

```
Argument 1 ($amount) expects Acme\Decimal\Decimal, got Decimal<bool>
```

The generic argument is nonsense: the class is constructed from a string
literal and nothing in the call binds a `bool`. Because the reported type
is wrong, the diagnostic is a false positive, and it accounts for the
argument mismatches the analyzer reports on large Laravel projects.

Two contributing factors are worth separating when it is fixed:

1. Whatever binds `bool` as the template argument of a class constructed
   from a string literal. Start at `classify_template_binding` /
   `remap_inherited_ctor_subs` in
   `type_engine/variable/rhs_resolution/instantiation.rs`, and check what
   the class's own `@template` bound resolves to when the constructor
   argument is a literal.
2. `is_type_compatible`'s unloadable-short-name escape hatch only
   inspects `TypeKind::Named`, so a `Generic` whose base name cannot be
   loaded (here `Decimal` written without its namespace) skips the hatch
   and is compared anyway. Widening the hatch does not fix the wrong
   type, but it stops a name the project cannot even load from producing
   a mismatch.

**Reproduce:** point `analyze` at a project that constructs a generic
class from a string literal and passes it to a parameter typed with the
bare class.

#### B21. Completion's backward scan is blind to comments

**Impact: Low · Effort: Medium**

`scan_back_to_opener` in `src/completion/source/helpers.rs` walks
backwards from the cursor over brackets, parentheses, braces, and string
literals, but does not skip `//`, `#`, or `/* … */` comments. A stray
bracket or quote in a comment between the call and the cursor unbalances
the walk, and the completion drops out:

```php
route('users.show', [   // TODO: check (parameters)
    '|' => 1,
]);
```

It fails closed (no suggestions rather than wrong ones), which is why it
is low impact. The obstacle is that a backwards scan cannot tell a `//`
inside a string from the start of a comment without a forward pass, so
the fix likely means scanning the enclosing statement forward once and
masking comments before the backwards walk, the way the Blade
preprocessor masks non-PHP text.
