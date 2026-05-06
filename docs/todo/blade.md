# PHPantom — Blade

This document is the implementation plan for Laravel Blade template
support in PHPantom. For Eloquent model support see `laravel.md`.
For general architecture see `ARCHITECTURE.md`.

---

## Philosophy

- **No application booting.** Consistent with `laravel.md`. We
  never run PHP or boot a Laravel application.
- **No call-site scanning.** We do not scan controllers, mailers, or
  other PHP files for `view()` calls to infer template variable types.
  Variable types come from explicit `@var` PHPDoc in `@php` blocks
  (compatible with Bladestan's `@bladestan-signature`), `@props`
  directives, or component class constructors.
- **Discovery is just directory walks.** Scanning `resources/views/`
  and `app/View/Components/` (plus `app/Livewire/`) at init time is
  the full extent of external Blade file discovery. Paths are converted
  to view names and component names via string transforms.
- **PSR-4 is for class source lookup, not discovery.** Once we know an
  FQN (e.g. `App\View\Components\Alert`), we use the existing
  `find_or_load_class` pipeline to read its source. We do not use
  PSR-4 to discover component names.
- **Graceful degradation.** Unknown directives become comments. Failed
  component resolution produces comments. The user always gets partial
  completions rather than a broken file. The preprocessor must never
  produce invalid PHP.

---

## Overview

Blade templates (`.blade.php`) mix HTML, Blade directives, component
tags (`<x-alert>`, `<livewire:counter>`), and embedded PHP. The
mago-syntax parser only understands pure PHP. The strategy:

1. Preprocess `.blade.php` files into valid PHP.
2. Feed the virtual PHP through the existing pipeline (parser,
   resolver, completion, definition).
3. Map LSP response positions back to the original Blade file via a
   source map.

---

## Phase 1: Blade-to-PHP Preprocessor

The core preprocessor is implemented in `src/blade/`. It transforms
Blade templates into virtual PHP line-by-line, with a source map for
coordinate translation. The LSP pipeline (`with_file_content`,
`update_ast`, `did_close`) transparently handles Blade files.

Known issues in the current implementation:

### Code actions are not Blade-aware

Code actions like "Import class" insert a `use` statement at the top
of the file rather than inside a `@php` / `<?php` block. All code
actions that produce text edits need their ranges translated, and
actions that generate new code need to be aware of Blade structure.

---

## Phase 2: Component Support

### 9. Template and component file discovery

At `initialized` time (alongside PSR-4 and classmap loading), scan
the filesystem to build three maps.

New file: `src/blade/discovery.rs`

#### 9a. View name map

Recursively scan `resources/views/` for `*.blade.php` files. Build
a map of dot-notation view names to file paths:

- `resources/views/users/index.blade.php` → `"users.index"`
- `resources/views/components/alert.blade.php` → `"components.alert"`

Store as:

```rust
/// View dot-name -> file path.
pub(crate) blade_views: Arc<Mutex<HashMap<String, PathBuf>>>,
```

#### 9b. Class-based component map

Recursively scan `app/View/Components/` for `*.php` files. Convert
file paths to kebab-case component names and FQNs:

- `app/View/Components/Alert.php` → name `"alert"`,
  FQN `"App\\View\\Components\\Alert"`
- `app/View/Components/Forms/Input.php` → name `"forms.input"`,
  FQN `"App\\View\\Components\\Forms\\Input"`

Index components (where directory name matches file name) should be
registered both ways:

- `app/View/Components/Card/Card.php` → name `"card"` (index) and
  `"card.card"` (explicit)

Store as:

```rust
/// Component kebab-name -> FQN.
pub(crate) blade_components: Arc<Mutex<HashMap<String, String>>>,
```

#### 9c. Livewire component map

Recursively scan `app/Livewire/` for `*.php` files. Convert file
paths to dot-notation component names and FQNs:

- `app/Livewire/Counter.php` → name `"counter"`,
  FQN `"App\\Livewire\\Counter"`
- `app/Livewire/Admin/Users.php` → name `"admin.users"`,
  FQN `"App\\Livewire\\Admin\\Users"`

Store as:

```rust
/// Livewire component name -> FQN.
pub(crate) livewire_components: Arc<Mutex<HashMap<String, String>>>,
```

#### 9d. Workspace root dependency

All three scans depend on `workspace_root`. Run them in `initialized`
after the existing Composer parsing, gated on
`workspace_root.is_some()`.

### 10. `<x-component>` tag parsing in preprocessor

New file: `src/blade/components.rs`

The preprocessor detects `<x-name ...>` and `</x-name>` tags and
converts them to PHP.

#### 10a. Opening tags

Parse `<x-component-name attr="val" :attr="$expr" ...>` or
`<x-component-name ... />` (self-closing).

1. Extract the component name (everything between `<x-` and the first
   whitespace or `>`/`/>`).
2. Look up the name in `blade_components`. If found, resolve the FQN.
3. Extract attributes:
   - `attr="literal"` → named arg with string value
   - `:attr="$expr"` → named arg with PHP expression value
   - `::attr="expr"` → ignored (Alpine.js passthrough)
   - Bare `attr` → named arg with `true`
   - `:$var` (short syntax) → named arg `var: $var`
4. Convert attribute names from kebab-case to camelCase for the
   constructor call.
5. Emit `$component = new \FQN(camelAttr: value, ...);`

If the component is not found in `blade_components`, check if it's an
anonymous component (exists in `blade_views` under `components.`
prefix). For anonymous components, emit a comment but still expose
`$attributes` and `$slot`.

For `<x-dynamic-component :component="$name" ...>`, emit
`echo $name;` so the expression gets parsed, but do not try to
resolve a target component.

#### 10b. Closing tags

`</x-name>` becomes a comment: `/* /x-name */`

#### 10c. Named slots

`<x-slot:title>` → `$title = new \Illuminate\Support\HtmlString('');`
`</x-slot>` → comment

#### 10d. Implicit component variables

When inside a component tag region (between opening and closing tags),
inject:

```php
/** @var \Illuminate\View\ComponentAttributeBag $attributes */
$attributes = new \Illuminate\View\ComponentAttributeBag([]);
/** @var \Illuminate\Support\HtmlString $slot */
$slot = new \Illuminate\Support\HtmlString('');
```

### 11. `<livewire:component>` tag parsing

Parse `<livewire:name :attr="$expr" ...>` or
`<livewire:name ... />`.

1. Extract the component name (everything between `<livewire:` and
   the first whitespace or `>`/`/>`).
2. Look up in `livewire_components`. If found, resolve the FQN.
3. Extract attributes (same rules as `<x-...>`).
4. Emit `$component = new \FQN();` followed by property assignments
   for each attribute: `$component->attrName = $expr;`.

Livewire attribute names use camelCase on the class, so apply the
same kebab-to-camelCase conversion.

### 12. `@props` and `@aware`

#### 12a. `@props`

`@props(['type' => 'info', 'message'])` becomes:

```php
$type = 'info';
$message = null;
/** @var \Illuminate\View\ComponentAttributeBag $attributes */
$attributes = new \Illuminate\View\ComponentAttributeBag([]);
/** @var \Illuminate\Support\HtmlString $slot */
$slot = new \Illuminate\Support\HtmlString('');
```

The preprocessor parses the array literal in the `@props()`
argument to extract variable names and default values. Variables
listed without a key-value pair (just `'message'`) get a `null`
default.

#### 12b. `@aware`

`@aware(['color' => 'gray'])` → `$color = 'gray';`

Same parsing as `@props` but without the `$attributes`/`$slot`
injection.

### 13. Component and view name completion

#### 13a. `<x-` completion

When the user types `<x-` in a Blade file, offer completions from:

- `blade_components` map (class-based components, kebab-case names)
- Anonymous component templates: entries in `blade_views` whose key
  starts with `"components."`, with the prefix stripped and dots
  preserved (e.g. `"components.forms.input"` → `"forms.input"`)

Detection: check if the characters before the cursor match
`<x-` (possibly with a partial name typed). This is a Blade-level
context check done before the normal PHP completion pipeline.

Items should use `CompletionItemKind::Module` or `::Class` depending
on whether they're anonymous or class-backed.

#### 13b. `<livewire:` completion

Same pattern. When the user types `<livewire:`, offer completions
from the `livewire_components` map.

#### 13c. `@include('` and `@extends('` view name completion

When the cursor is inside the string argument to `@include`,
`@includeIf`, `@includeWhen`, `@includeUnless`, `@includeFirst`,
`@extends`, `@each`, or a `view()` function call, offer completions
from the `blade_views` map (dot-notation view names).

Detection: look for `@include('`, `@extends('`, or `view('` before
the cursor and check that the cursor is inside the quotes. The
trigger characters `'` and `"` are already registered.

#### 13d. Component attribute completion

When the cursor is inside a `<x-component ` tag (after the component
name, before `>` or `/>`), resolve the component class and offer its
constructor parameter names as kebab-case attribute completions.

Offer both plain and `:` prefixed variants:
- `message` (string literal)
- `:message` (PHP expression)

For Livewire components, offer the class's public property names as
attribute completions.

### 14. Tests

Create `tests/blade_components.rs`:

- `<x-alert>` resolves to `App\View\Components\Alert`
- `<x-forms.input>` resolves to `App\View\Components\Forms\Input`
- `<x-card>` resolves to index component
  `App\View\Components\Card\Card`
- `<livewire:counter>` resolves to `App\Livewire\Counter`
- Anonymous component detection
- `<x-dynamic-component>` does not crash
- Attribute parsing: string, expression, Alpine passthrough, bare,
  short syntax

Extend `tests/completion_blade.rs`:

- `<x-` triggers component name completions
- `<livewire:` triggers Livewire component name completions
- `@include('` triggers view name completions
- `<x-alert ` triggers attribute completions
- `$component->` after component instantiation
- `$attributes->` in component templates

---

## Phase 3: Cross-File View Intelligence

### 15. Go-to-definition for view names and components

#### 15a. View name go-to-definition

Inside `@include('users.index')`, `@extends('layouts.app')`, or
`view('welcome')`:

1. Extract the view name string at the cursor position.
2. Look up in `blade_views`.
3. Return a `Location` pointing to the resolved file.

#### 15b. Component tag go-to-definition

On `<x-alert>`:

1. Extract the component name.
2. Look up in `blade_components` to get the FQN.
3. Use `find_or_load_class` + `class_index` / `classmap` to find the
   source file.
4. Return a `Location` pointing to the class definition.

On `<livewire:counter>`:

1. Same pattern using `livewire_components`.

### 16. Signature merging for `@extends`

When template A contains `@extends('layouts.app')`:

1. Resolve `layouts.app` via `blade_views` to a file path.
2. Read or preprocess that file.
3. Extract `@var` declarations from its `@php` blocks.
4. Merge those declarations into template A's virtual PHP prologue,
   following the Bladestan covariance model:
   - Variables only in child: use child type.
   - Variables only in parent: use parent type.
   - Variables in both: child may narrow but not widen.
   - Walk the chain recursively if the parent also `@extends`.

This gives child templates access to the parent's declared
variables without the user redeclaring them.

### 17. Component class to template variable typing

For class-based components, when editing the component's Blade
template:

1. Determine which component class backs this template. Convention:
   `resources/views/components/alert.blade.php` is backed by
   `App\View\Components\Alert`.
2. Load the class via `find_or_load_class`.
3. Read public properties and constructor parameter types.
4. Inject those as `@var` declarations in the virtual PHP prologue
   (unless the template already has explicit `@var` or `@props`).

### 18. Tests

Create `tests/definition_blade.rs`:

- Go-to-definition on `@include('users.index')` → view file
- Go-to-definition on `@extends('layouts.app')` → layout file
- Go-to-definition on `<x-alert>` → component class
- Go-to-definition on `<livewire:counter>` → Livewire class

Extend `tests/completion_blade.rs`:

- Variables from parent layout available in child via `@extends`
- Component class constructor types available in template

---

## Phase 4: Blade Directive Completion

### 19. Directive name completion

When the user types `@` in a Blade file (outside `{{ }}`, `@php`
blocks, and string literals), offer completions for all known Blade
directives with snippet templates.

Each completion inserts a snippet with tab stops:

```
@if ($1)
    $0
@endif
```

```
@foreach ($1 as $2)
    $0
@endforeach
```

```
@include('$1')
```

```
@props([$1])
```

```
@inject('$1', '$2')
```

```
@php
$0
@endphp
```

Detection: The `@` trigger character is already registered. In
`handle_completion`, check `is_blade_file` and that the cursor is in
an HTML/directive context (not inside `{{ }}`, not inside a `@php`
block, not inside a string literal).

### 20. Tests

Extend `tests/completion_blade.rs`:

- `@` triggers directive name completions
- `@if` partial triggers filtered directive completions
- No directive completion inside `{{ }}` or `@php` blocks

---

## Implementation Sequence

Steps 1-2 are complete. The remaining steps build on the existing
preprocessor and LSP pipeline.

### Step 3: Remaining Phase 1 items

Inject `$loop` inside `@foreach` blocks. Handle `@session`/`@error`/
`@context` implicit variables. Implement stub directives and verbatim
regions. Add `languageId` check.

**Deliverable:** `$loop->first`, `$value` inside `@session` blocks,
and `$message` inside `@error` blocks all produce completions.

### Step 4: Discovery (item 9)

Implement `src/blade/discovery.rs`. Scan `resources/views/`,
`app/View/Components/`, `app/Livewire/` at init time. Add the three
new maps to `Backend`.

**Deliverable:** Maps are populated and logged at startup.

### Step 5: Component tag parsing (items 10-12)

Implement `src/blade/components.rs`. Parse `<x-...>` and
`<livewire:...>` tags. Handle `@props`, `@aware`, named slots.

**Deliverable:** `$component->` after `<x-alert>` produces
completions from the Alert class. `$attributes->` works in component
templates.

### Step 6: Name completions (item 13)

Implement `<x-`, `<livewire:`, `@include('`, and component attribute
completions.

**Deliverable:** Typing `<x-` shows available components. Typing
`@include('` shows available views. Typing attributes inside
`<x-alert ` shows constructor parameter names.

### Step 7: Directive completion (item 19)

Implement `@` directive name completion with snippets.

**Deliverable:** Typing `@` in a Blade file shows all known
directives with snippet templates.

### Step 8: Cross-file intelligence (items 15-17)

Implement go-to-definition for view names and component tags.
Implement `@extends` signature merging. Implement component class to
template variable typing.

**Deliverable:** Ctrl-click on `@include('users.index')` jumps to
the file. Parent layout variables are available in child templates.

---

## Editor Integration Notes

### File extension detection

The server activates Blade preprocessing when:
- The URI ends with `.blade.php`, OR
- The `languageId` in `did_open` is `"blade"`.

### Zed extension

The Zed extension (`zed-extension/extension.toml`) currently
registers `languages = ["PHP"]`. To support Blade files, it will
need an additional language registration. This may require Zed to
have a Blade language definition (grammar, file associations), or
the extension can register `.blade.php` as a PHP variant. This is
an editor-side concern and may need a separate Zed extension or an
update to the existing one.

### Other editors

- **VS Code:** Extensions like Laravel Blade Snippets set
  `languageId` to `"blade"`. PHPantom's VS Code integration would
  need to register for both `"php"` and `"blade"` language IDs.
- **Neovim:** `lspconfig` can be configured to send `.blade.php`
  files to PHPantom with the correct `languageId`.