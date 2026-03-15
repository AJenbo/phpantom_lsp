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

This phase delivers the core value: completion and go-to-definition
for PHP expressions inside Blade templates.

### 1. New module `src/blade/`

Create the following files:

- `src/blade/mod.rs` — module declarations, `is_blade_file()` helper,
  public `preprocess()` entry point.
- `src/blade/preprocessor.rs` — the main Blade-to-PHP transformation
  engine.
- `src/blade/directives.rs` — directive pattern matching and their PHP
  translations.
- `src/blade/source_map.rs` — `BladeSourceMap`, `ColumnAdjustment`,
  and position translation functions.

Register `pub mod blade;` in `src/lib.rs`.

### 2. Preprocessor design

The preprocessor operates line-by-line and produces exactly one output
line per input line. This is the single most important invariant: it
makes line mapping a trivial identity function so only column
adjustments are needed.

The virtual PHP document starts with an implicit `<?php` prologue
(occupying a synthetic first line if the Blade file doesn't start with
one). Each Blade line is then transformed:

- **Blade directives** become single-line PHP statements.
- **Echo expressions** (`{{ }}`, `{!! !!}`) are replaced inline.
- **HTML-only lines** become single-line PHP comments (`// HTML`).
- **Component tags** are handled in Phase 2.

#### 2a. Echo expressions

| Blade | Virtual PHP |
|---|---|
| `{{ $expr }}` | `echo e($expr);` |
| `{!! $expr !!}` | `echo ($expr);` |
| `{{-- comment --}}` | `/* comment */` |

When echo expressions appear mid-line (surrounded by HTML), the
surrounding HTML becomes a comment on the same line and the echo
expression is emitted as PHP on that line. Multiple echo expressions
on the same line are joined with `;` on the same output line.

The `@{{ expr }}` escape syntax (used for JS frameworks) strips the
leading `@` and emits a comment.

#### 2b. Control directives

| Blade | Virtual PHP |
|---|---|
| `@if ($cond)` | `if ($cond):` |
| `@elseif ($cond)` | `elseif ($cond):` |
| `@else` | `else:` |
| `@endif` | `endif;` |
| `@foreach ($arr as $v)` | `foreach ($arr as $v):` |
| `@endforeach` | `endforeach;` |
| `@for (...)` | `for (...):` |
| `@endfor` | `endfor;` |
| `@while (...)` | `while (...):` |
| `@endwhile` | `endwhile;` |
| `@forelse ($arr as $v)` | `foreach ($arr as $v):` |
| `@empty` | `endforeach; if (false):` |
| `@endforelse` | `endif;` |
| `@switch($v)` | `switch($v):` |
| `@case(...)` | `case (...):` |
| `@break` | `break;` |
| `@default` | `default:` |
| `@endswitch` | `endswitch;` |
| `@unless (...)` | `if (!( ... )):` |
| `@endunless` | `endif;` |
| `@isset($v)` | `if (isset($v)):` |
| `@endisset` | `endif;` |
| `@empty($v)` (directive) | `if (empty($v)):` |
| `@endempty` | `endif;` |

#### 2c. Directives that expose implicit variables

| Blade | Virtual PHP |
|---|---|
| `@session('key')` | `if (true): $value = '';` |
| `@endsession` | `endif;` |
| `@context('key')` | `if (true): $value = '';` |
| `@endcontext` | `endif;` |
| `@error('field')` | `if (true): $message = '';` |
| `@enderror` | `endif;` |

#### 2d. Stub directives

These are parsed but produce no semantic PHP effect:

- `@auth`/`@endauth`, `@guest`/`@endguest`, `@env(...)`/`@endenv`,
  `@production`/`@endproduction`, `@once`/`@endonce` →
  `if (true):` / `endif;`
- `@csrf`, `@method(...)`, `@push`/`@endpush`, `@prepend`/`@endprepend`,
  `@stack(...)`, `@yield(...)`, `@section(...)`/`@endsection`/`@show`,
  `@extends(...)`, `@include(...)` and variants, `@includeIf(...)`,
  `@includeWhen(...)`, `@includeUnless(...)`, `@includeFirst(...)`,
  `@each(...)` → `/* @directive */`

#### 2e. PHP blocks

| Blade | Virtual PHP |
|---|---|
| `@php ... @endphp` | Raw PHP content (unwrap the delimiters) |
| `@use('App\Models\Flight')` | `use App\Models\Flight;` |
| `@use('App\Models\Flight', 'F')` | `use App\Models\Flight as F;` |
| `@use('App\Models\{A, B}')` | `use App\Models\{A, B};` |

#### 2f. Service injection

`@inject('metrics', 'App\Services\MetricsService')` →
`$metrics = new \App\Services\MetricsService();`

This gives the variable the correct type for completion.

#### 2g. Inline attribute directives

`@class([...])`, `@style([...])`, `@checked(...)`, `@selected(...)`,
`@disabled(...)`, `@readonly(...)`, `@required(...)` → `echo (...);`
so their PHP expressions get parsed.

#### 2h. Verbatim regions

`@verbatim ... @endverbatim` content becomes PHP comments (it
contains JS template syntax that would confuse the parser).

#### 2i. Unknown directives

Any `@directive(...)` not recognized becomes `/* @directive(...) */`.

### 3. Source map

Because the preprocessor maintains one output line per input line, the
`BladeSourceMap` only stores per-line column adjustments:

```rust
pub struct BladeSourceMap {
    /// Per-line column adjustment regions.
    /// Lines with no adjustments have an empty vec (identity mapping).
    adjustments: Vec<Vec<ColumnAdjustment>>,
}

pub struct ColumnAdjustment {
    pub blade_col_start: u32,
    pub blade_col_end: u32,
    pub php_col_start: u32,
    pub php_col_end: u32,
}
```

Functions on `BladeSourceMap`:

- `blade_to_php(line, col) -> (line, col)` — translate a position
  in the original Blade file to the virtual PHP.
- `php_to_blade(line, col) -> (line, col)` — translate a position
  in the virtual PHP back to the original Blade file.

### 4. New fields on `Backend`

```rust
/// Virtual PHP content generated from Blade files.
/// Maps file URI -> preprocessed PHP source.
pub(crate) blade_virtual_content: Arc<Mutex<HashMap<String, String>>>,

/// Source maps from virtual PHP back to original Blade positions.
/// Maps file URI -> BladeSourceMap.
pub(crate) blade_source_maps: Arc<Mutex<HashMap<String, BladeSourceMap>>>,
```

Add these to `Backend::defaults()` with empty `HashMap`s.

### 5. Wire into the LSP pipeline

#### 5a. `is_blade_file()` helper

A public function in `src/blade/mod.rs`:

```rust
pub fn is_blade_file(uri: &str) -> bool {
    uri.ends_with(".blade.php")
}
```

The server should also check `languageId == "blade"` in `did_open`
when available (some editors send this for Blade files).

#### 5b. `did_open` / `did_change` in `server.rs`

After storing the file content in `open_files`, check
`is_blade_file(&uri)`. If true:

1. Run `blade::preprocess(&content)` which returns
   `(virtual_php: String, source_map: BladeSourceMap)`.
2. Store the virtual PHP in `blade_virtual_content`.
3. Store the source map in `blade_source_maps`.
4. Call `self.update_ast(&uri, &virtual_php)` (not the original
   content).

If the file is not Blade, the existing path is unchanged.

#### 5c. `completion` in `handler.rs`

At the top of `handle_completion`, after reading `content` from
`open_files`, check if this is a Blade file. If so:

1. Read the virtual PHP from `blade_virtual_content` (this is what
   gets parsed and resolved against).
2. Read the source map from `blade_source_maps`.
3. Translate the cursor `position` from Blade coordinates to virtual
   PHP coordinates using `source_map.blade_to_php()`.
4. Run the normal completion pipeline against the virtual PHP content
   with the translated position.
5. Translate result positions in `CompletionItem` text edits back to
   Blade coordinates using `source_map.php_to_blade()`.

#### 5d. `goto_definition` in `server.rs`

Same pattern: translate the incoming position to virtual PHP
coordinates before resolution, translate the result location back
if it points into the same Blade file.

#### 5e. `did_close` in `server.rs`

Clean up `blade_virtual_content` and `blade_source_maps` entries
for the closed file.

### 6. Implicit variables

The preprocessor injects these declarations at the top of the virtual
PHP (after the `<?php` prologue):

```php
/** @var \Illuminate\Support\ViewErrorBag $errors */
$errors = new \Illuminate\Support\ViewErrorBag();
/** @var \Illuminate\View\Factory $__env */
$__env = new \Illuminate\View\Factory();
```

Inside `@foreach` blocks, inject a `$loop` variable declaration
immediately after the `foreach` opening:

```php
/** @var object{index: int, iteration: int, remaining: int, count: int, first: bool, last: bool, even: bool, odd: bool, depth: int, parent: ?object} $loop */
$loop = (object)[];
```

### 7. Explicit type declarations via `@var` PHPDoc

Users declare variable types in their Blade templates by writing:

```blade
@php
/**
 * @bladestan-signature
 * @var string $name
 * @var \App\Models\User $user
 */
@endphp
```

The `@php ... @endphp` block becomes raw PHP. The `@var` tags are
standard PHPDoc that mago-syntax already parses. The
`@bladestan-signature` marker is just a comment we ignore. This works
out of the box with the preprocessor and is compatible with the
Bladestan ecosystem. No special handling needed.

### 8. Tests

Create `tests/blade_preprocessor.rs` with unit tests:

- Echo expressions: `{{ $var }}`, `{!! $html !!}`, `{{-- comment --}}`
- Each control directive (if/else/endif, foreach, for, while, switch,
  forelse, unless, isset, empty)
- Directives with implicit vars (@error, @session, @context)
- Stub directives (@auth, @guest, @csrf, etc.)
- PHP blocks (@php/@endphp, @use)
- Service injection (@inject)
- Inline attribute directives (@class, @checked, etc.)
- Verbatim regions
- Unknown directives
- Mixed lines (HTML with embedded `{{ }}`)
- Line count preservation (critical invariant)

Create `tests/completion_blade.rs` with integration tests:

- `$this->` inside a `@php` block
- `$var->` where `$var` is declared via `@var` PHPDoc
- `$user->` inside a `@foreach` with typed collection
- `$loop->` inside a `@foreach`
- `$errors->` (implicit variable)
- `$value` inside `@session` block
- `$message` inside `@error` block

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

The items below are ordered for incremental delivery. Each step
produces a testable, shippable improvement.

### Step 1: Preprocessor skeleton (items 1-3)

Create `src/blade/mod.rs`, `preprocessor.rs`, `directives.rs`,
`source_map.rs`. Implement the `preprocess()` function that handles
echo expressions and all control/stub directives. Write the
`BladeSourceMap` with `blade_to_php()` and `php_to_blade()`.

**Deliverable:** A pure function
`preprocess(blade_source: &str) -> (String, BladeSourceMap)` with
comprehensive unit tests proving line count preservation and correct
PHP output.

### Step 2: Wire into LSP (items 4-5)

Add `blade_virtual_content` and `blade_source_maps` to `Backend`.
Modify `did_open`, `did_change`, `did_close`, `completion`, and
`goto_definition` in `server.rs` to route Blade files through the
preprocessor and translate positions.

**Deliverable:** Completion works for `$var->` inside `{{ }}` and
`@php` blocks in `.blade.php` files.

### Step 3: Implicit variables (item 6)

Inject `$errors`, `$__env`, and `$loop` declarations. Handle
`@session`/`@error`/`@context` implicit `$value`/`$message`.

**Deliverable:** `$errors->`, `$loop->first`, and `$value` inside
`@session` blocks all produce completions.

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