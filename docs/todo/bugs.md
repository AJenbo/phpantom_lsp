# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B18. Override completion offers `final` parent methods

**Impact: Medium · Effort: Medium**

`collect_overridable_methods` in
`completion/context/override_completion.rs` filters private, magic, and
virtual methods, but nothing filters `final`. A `final public function
onLock()` on a parent is therefore offered as an override candidate, and
accepting the completion inserts a declaration PHP rejects outright
("Cannot override final method Base::onLock()").

Both override entry points are affected: the `function get|` path and
the class-body-root path, where it is more visible because the inserted
snippet is a complete declaration rather than just a name.

**Where to look:** `MethodInfo` has no `is_final` field — `is_final`
exists only on `ClassInfo`. The fix is to record the method modifier
during parsing (`method.modifiers` is already read for `is_abstract` and
`is_static` at `parser/classes.rs:1298`) and skip final methods in
`MethodCollector::push_from_class`. Adding the field touches every
`MethodInfo` construction site (~30 in `src/`, plus test fixtures), so
it is a mechanical but wide change; keep it a task of its own.

Note that a `final` parent method should still be offered by *member
access* completion (`$obj->onL|`) — only override candidates need the
filter.

#### B19. Override completion drops `readonly` from a redeclared property

**Impact: Medium · Effort: Medium**

At the class-body root, a parent's `public readonly string $onName` is
offered with the inserted declaration `public string $onName;`. PHP
rejects that: "Cannot redeclare readonly property A::$x as non-readonly
B::$x". Redeclaring it *as* readonly is legal (verified on PHP 8.5), so
the fix is to carry the modifier through, not to filter the property
out.

**Where to look:** `PropertyInfo` has no `is_readonly` field. The
modifier is currently recovered by a local re-parse in
`code_actions/generate_getter_setter.rs` (`has_readonly`), which shows
where to read it from during parsing. Once `PropertyInfo` carries it,
`build_property_override_completions` should emit `readonly ` after the
visibility keyword whenever `include_declaration` is set. Same blast
radius caveat as [B18](#b18-override-completion-offers-final-parent-methods):
adding the field touches every `PropertyInfo` construction site.

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

#### B22. Hover collapses a branch-merged union to its first member

**Impact: Medium · Effort: Medium**

When a variable is assigned different types in different branches of an
`if`, the forward walker merges the branches correctly (completion after
the `if` offers members of every branch's type), but hover reports only
the first branch's type:

```php
function test(): void {
    if (rand(0, 1)) { $x = 'a'; } else { $x = 42; }
    take($x); // hover on $x says 'a'; it is 'a'|42
}
```

The same holds for objects: `$x = new Foo(); if (rand(0, 1)) { $x = new
Bar(); }` hovers as `Foo` while completion on `$x->` correctly offers
both `Foo` and `Bar` members. Hover is not incapable of showing several
types (a `Foo|Bar` parameter renders as two sections), so the union
survives the walk and is dropped somewhere between the merged scope and
the hover renderer.

Found while writing fixtures for `never`-returning calls: the whole
class of "variable keeps its pre-branch type" assertions cannot be
expressed as a hover fixture until this is fixed, which is why
`tests/fixtures/type/never_return_type.fixture` asserts through
completion instead.

**Reproduce:** hover a variable after an `if`/`else` that assigns a
different type in each branch.

#### B23. A top-level function body resolves source-level class names without a namespace

**Impact: Medium · Effort: Medium**

Source-level class references (`new Foo`, `Foo::bar()`, `Foo::CONST`)
are resolved with `resolve_source_class_name`, which needs the enclosing
namespace so that a same-namespace class outranks a global class of the
same short name. Every call site reads it from
`current_class.file_namespace`, but when the cursor is inside a plain
function (or top-level code) there is no enclosing class and the callers
fall back to `ClassInfo::default()`, whose `file_namespace` is `None`:

```php
namespace App;

function test(): void {
    Aborter::fail(); // resolves to \Aborter, not App\Aborter
}
```

Inside a method of a class in the same file the resolution is correct,
so the gap is specific to function and top-level scopes. The fix is to
give the synthesised placeholder class the namespace that contains the
cursor (`Backend::namespace_at_offset` already computes it for the
multi-namespace case) rather than leaving it empty, so every consumer of
`current_class.file_namespace` gets the right answer.

**Reproduce:** in a namespaced file that also declares a global class of
the same short name, reference the class unqualified from a plain
function body.
